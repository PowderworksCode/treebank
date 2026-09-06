//! The shared vocabulary, from the module: which of its sorts and
//! constructors carry which of treebank's terms, lowered to the two tiers
//! `notes/DESIGN.md` §3 describes and checked by the code that checks the
//! shipped grammars.
//!
//! A `vocabulary` section (a treebank extension) binds terms to members:
//!
//! ```text
//! vocabulary
//!   _statement    = Stmt
//!   _expression   = Exp
//!   _body         = Block
//!   _branch       = Stmt.If
//!   _control_flow = _branch _loop _jump
//!   _clause       = Else
//! ```
//!
//! The lowering decides the tier per term, as §3.1.1 says a grammar must:
//!
//! - A **table-tier** term bound to one whole sort **renames** that sort's
//!   supertype (`_stmt` becomes `_statement`), so the role is the sort's
//!   own derivation.
//! - A table-tier term bound to constructors, single-constructor sorts,
//!   tokens or other terms **threads** a new supertype: the members become
//!   its alternation and every reference to a member becomes a reference
//!   to the term, so `_branch` nests inside `_statement` wherever `if`
//!   stood, and `_body` wraps `block` in every field that held it. The
//!   tree is unchanged, since supertypes are hidden.
//! - A **facet-tier** term goes to `roles.json` as type-level membership.
//!   Three facets are also derived from what the module already says:
//!   `_scope` and `_binding` from the binding attributes, `_callable` from
//!   a binding of kind `function`, `_comment` from LAYOUT.
//! - A table-tier term whose members another table-tier term already
//!   claims, with neither nested in the other, cannot be one derivation.
//!   If the vocabulary marks it `either_tier` it is **demoted** to a facet
//!   with the reason written for it; otherwise it is refused.
//!
//! `roles.json` lists every named node the terms leave uncovered as
//! `uncategorised`, with a reason that says exactly that, so the checker's
//! rule 2 passes and the finding says what the module has not named.

use std::collections::{BTreeMap, BTreeSet};

use anyhow::{bail, Result};
use serde_json::{json, Map, Value};

use crate::ast::*;
use crate::lower::{Finding, Kind, Names};

pub struct Emitted {
    /// The `roles.json` manifest.
    pub roles: Value,
    pub findings: Vec<Finding>,
    /// Term -> the node types it covers, table and facet alike.
    pub coverage: BTreeMap<String, BTreeSet<String>>,
}

/// Apply the module's vocabulary to a lowered grammar in place. `derived`
/// carries the facets the bindings lowering already knows.
pub fn apply(
    module: &Module,
    grammar: &mut Value,
    names: &mut Names,
    derived: &BTreeMap<String, BTreeSet<String>>,
) -> Result<Option<Emitted>> {
    let terms: Vec<&VocabTerm> = module.vocabulary().collect();
    if terms.is_empty() && derived.is_empty() {
        return Ok(None);
    }
    let vocab = treebank::vocabulary();
    let table: BTreeSet<&str> = vocab.table.iter().map(|t| t.name.as_str()).collect();
    let facet: BTreeSet<&str> = vocab.facets.iter().map(|t| t.name.as_str()).collect();
    let either: BTreeSet<&str> = vocab.either_tier.iter().map(String::as_str).collect();
    let mut findings = Vec::new();
    if !terms.is_empty() {
        findings.push(Finding {
            kind: Kind::Extension,
            what: format!(
                "`vocabulary` section (not SDF3): {} terms of treebank's vocabulary {} bound to this module's sorts and constructors",
                terms.len(),
                vocab.version
            ),
        });
    }

    let mut facets: BTreeMap<String, BTreeSet<String>> = derived.clone();
    let mut demoted: BTreeMap<String, String> = BTreeMap::new();
    // node -> the table term that claims it, for conflict detection.
    let mut claimed: BTreeMap<String, String> = BTreeMap::new();
    let mut coverage: BTreeMap<String, BTreeSet<String>> = BTreeMap::new();

    // Terms whose members name other terms come after those terms.
    let mut order: Vec<&VocabTerm> = Vec::new();
    let mut pending: Vec<&VocabTerm> = terms.clone();
    while !pending.is_empty() {
        let before = pending.len();
        let done: BTreeSet<&str> = order.iter().map(|t| t.term.as_str()).collect();
        let (ready, rest): (Vec<&VocabTerm>, Vec<&VocabTerm>) =
            pending.into_iter().partition(|t| {
                t.members
                    .iter()
                    .all(|m| !m.starts_with('_') || done.contains(m.as_str()))
            });
        order.extend(ready);
        pending = rest;
        if pending.len() == before {
            bail!(
                "vocabulary terms refer to each other in a cycle or to an undeclared term: {:?}",
                pending.iter().map(|t| &t.term).collect::<Vec<_>>()
            );
        }
    }

    for t in order {
        let term = t.term.as_str();
        let is_table = table.contains(term);
        let is_facet = facet.contains(term);
        if !is_table && !is_facet {
            findings.push(Finding {
                kind: Kind::Unsupported,
                what: format!("`{term}` is not a term of vocabulary {}; the list is closed, and this binding is ignored", vocab.version),
            });
            continue;
        }
        // Resolve members to rule names.
        let mut members: Vec<String> = Vec::new();
        let mut whole_sort_supertype: Option<String> = None;
        for m in &t.members {
            let resolved = if m.starts_with('_') {
                if demoted.contains_key(m) {
                    findings.push(Finding {
                        kind: Kind::Unsupported,
                        what: format!("`{term}` lists `{m}`, which was demoted to a facet and has no supertype to nest; skipped"),
                    });
                    continue;
                }
                m.clone()
            } else if let Some((sort, cons)) = m.split_once('.') {
                match names.node.get(&format!("{sort}.{cons}")) {
                    Some(n) => n.clone(),
                    None => bail!("vocabulary: `{m}` names no constructor of the module"),
                }
            } else if let Some(rule) = names.sort_rule.get(m) {
                if rule.starts_with('_') && t.members.len() == 1 && is_table {
                    whole_sort_supertype = Some(rule.clone());
                }
                rule.clone()
            } else {
                bail!("vocabulary: `{m}` names no sort or constructor of the module");
            };
            members.push(resolved);
        }
        if members.is_empty() {
            continue;
        }

        if is_facet && !is_table {
            let nodes: BTreeSet<String> = members
                .iter()
                .flat_map(|m| closure_of(grammar, m))
                .map(|n| names.alias.get(&n).cloned().unwrap_or(n))
                .collect();
            facets
                .entry(term.to_string())
                .or_default()
                .extend(nodes.clone());
            coverage.insert(term.to_string(), nodes.clone());
            // A member that is a sort's own alternation (`_upsert` over
            // `upsert_nothing`, `upsert_update`) stays a hidden rule but
            // leaves the supertypes list: the checker holds every
            // supertype to the table tier, and this one is facet-only.
            let mut unlisted = Vec::new();
            if let Some(s) = grammar["supertypes"].as_array_mut() {
                for m in &members {
                    if m.starts_with('_')
                        && !table.contains(m.as_str())
                        && s.iter().any(|v| v.as_str() == Some(m))
                    {
                        s.retain(|v| v.as_str() != Some(m));
                        unlisted.push(m.clone());
                    }
                }
            }
            findings.push(Finding {
                kind: Kind::Mapped,
                what: format!(
                    "`{term}` is a facet: [{}] listed in roles.json{}",
                    nodes.iter().cloned().collect::<Vec<_>>().join(", "),
                    if unlisted.is_empty() { String::new() } else { format!("; [{}] left the supertypes list (hidden rules still), since a supertype must be a table-tier term", unlisted.join(", ")) }
                ),
            });
            continue;
        }

        // Table tier. A term whose members sit inside a whole-sort term
        // nests in it; that is the point. A conflict is a member that
        // another *threaded* term already claims, with neither term
        // nesting the other: the node would have two derivations.
        let member_nodes: BTreeSet<String> = members
            .iter()
            .flat_map(|m| closure_of(grammar, m))
            .collect();
        let nested_terms: BTreeSet<String> = members
            .iter()
            .filter(|m| m.starts_with('_'))
            .cloned()
            .collect();
        let conflict = member_nodes.iter().find_map(|n| {
            claimed
                .get(n)
                .filter(|c| !nested_terms.contains(*c))
                .map(|c| (n.clone(), c.clone()))
        });
        if let Some((node, other)) = conflict {
            if either.contains(term) {
                let reason = format!(
                    "`{node}` is also a member of `{other}`, and neither term nests in the other, so the two cannot share one derivation; every member of `{term}` is a concrete node type, so facet membership selects the same nodes"
                );
                demoted.insert(term.to_string(), reason.clone());
                facets
                    .entry(term.to_string())
                    .or_default()
                    .extend(member_nodes.clone());
                coverage.insert(term.to_string(), member_nodes.clone());
                findings.push(Finding {
                    kind: Kind::Deviation,
                    what: format!("`{term}` demoted to the facet tier: {reason}"),
                });
            } else {
                findings.push(Finding {
                    kind: Kind::Unsupported,
                    what: format!("`{term}` shares `{node}` with `{other}` and is not either_tier in the vocabulary; the table tier cannot hold both, and `{term}` is not threaded"),
                });
            }
            continue;
        }

        if let Some(old) = whole_sort_supertype.clone() {
            rename(grammar, names, &old, term);
            findings.push(Finding {
                kind: Kind::Mapped,
                what: format!(
                    "`{term}` is the sort {}: its supertype `{old}` is named `{term}`",
                    t.members[0]
                ),
            });
        } else {
            // A member that is a sort's own hidden alternation, not a
            // term, is flattened into the threaded term: the checker wants
            // every supertype to be a table-tier term, and `_dir` inside
            // `_modifier` would be one that is not.
            let flatten: BTreeSet<String> = members
                .iter()
                .filter(|m| {
                    m.starts_with('_') && !table.contains(m.as_str()) && !facet.contains(m.as_str())
                })
                .cloned()
                .collect();
            thread(grammar, term, &members, &flatten);
            findings.push(Finding {
                kind: Kind::Mapped,
                what: format!(
                    "`{term}` threaded as a supertype over [{}]; every reference to a member now goes through it, and the tree is unchanged{}",
                    members.join(", "),
                    if flatten.is_empty() { String::new() } else { format!("; [{}] are sorts, not terms, and were flattened into it", flatten.iter().cloned().collect::<Vec<_>>().join(", ")) }
                ),
            });
        }
        if whole_sort_supertype.is_none() {
            // The innermost threaded term owning each node: a term that
            // nests others hands their nodes back to them.
            for n in &member_nodes {
                let owned_by_nested = nested_terms
                    .iter()
                    .any(|t| coverage.get(t).is_some_and(|c| c.contains(n)));
                if !owned_by_nested {
                    claimed.insert(n.clone(), term.to_string());
                }
            }
        }
        coverage.insert(term.to_string(), member_nodes);
    }

    // Coverage: rule 2 of the checker, computed here so the manifest says
    // what is outside and the finding says so too.
    let supertypes: Vec<String> = grammar["supertypes"]
        .as_array()
        .map(|a| {
            a.iter()
                .filter_map(|v| v.as_str().map(String::from))
                .collect()
        })
        .unwrap_or_default();
    let table_covered: BTreeSet<String> = supertypes
        .iter()
        .filter(|s| table.contains(s.as_str()))
        .flat_map(|s| closure_of(grammar, s))
        .collect();
    let facet_covered: BTreeSet<String> = facets.values().flatten().cloned().collect();
    let mut uncategorised = Vec::new();
    let named = named_nodes(grammar);
    for n in &named {
        if !table_covered.contains(n) && !facet_covered.contains(n) {
            // Say where it sits: a token or node only ever referenced
            // from inside covered nodes is a piece of them, which is the
            // shape of most of the shipped grammars' uncategorised entries.
            let parents = referencing_rules(grammar, n);
            let covered_parents: Vec<&String> = parents
                .iter()
                .filter(|p| table_covered.contains(*p) || facet_covered.contains(*p))
                .collect();
            let reason = if !parents.is_empty() && covered_parents.len() == parents.len() {
                format!(
                    "a piece of [{}], which carry the roles; no term names it on its own",
                    covered_parents
                        .iter()
                        .map(|s| s.as_str())
                        .collect::<Vec<_>>()
                        .join(", ")
                )
            } else {
                "no vocabulary term names it: the module's `vocabulary` section leaves it out"
                    .to_string()
            };
            uncategorised.push(json!({"node": n, "reason": reason}));
        }
    }
    if !uncategorised.is_empty() {
        findings.push(Finding {
            kind: Kind::Deviation,
            what: format!(
                "{} named node(s) outside the vocabulary, ledgered as uncategorised: [{}]",
                uncategorised.len(),
                uncategorised
                    .iter()
                    .map(|u| u["node"].as_str().unwrap_or("").to_string())
                    .collect::<Vec<_>>()
                    .join(", ")
            ),
        });
    }
    let facets_json: Map<String, Value> = facets
        .iter()
        .filter(|(_, m)| !m.is_empty())
        .map(|(k, m)| (k.clone(), json!(m)))
        .collect();
    let roles = json!({
        "vocabulary": vocab.version,
        "demoted": demoted,
        "facets": facets_json,
        "uncategorised": uncategorised,
    });
    findings.push(Finding {
        kind: Kind::Mapped,
        what: format!(
            "roles.json: {} of {} table-tier terms are supertypes, {} facet(s), {} named node(s), {} uncategorised (vocabulary {})",
            supertypes.iter().filter(|s| table.contains(s.as_str())).count(),
            table.len(),
            facets_json.len(),
            named.len(),
            uncategorised.len(),
            vocab.version
        ),
    });
    Ok(Some(Emitted {
        roles,
        findings,
        coverage,
    }))
}

/// The concrete node types under a rule: itself if it is a node, or the
/// closure of its members if it is a supertype.
fn closure_of(grammar: &Value, rule: &str) -> BTreeSet<String> {
    let mut out = BTreeSet::new();
    let mut stack = vec![rule.to_string()];
    let mut seen = BTreeSet::new();
    while let Some(r) = stack.pop() {
        if !seen.insert(r.clone()) {
            continue;
        }
        if r.starts_with('_') {
            if let Some(members) = grammar["rules"][&r]["members"].as_array() {
                for m in members {
                    if let Some(n) = m["name"].as_str() {
                        stack.push(n.to_string());
                    } else if m["type"] == "ALIAS" && m["named"] == true {
                        // A member aliased to a node type is that type.
                        if let Some(v) = m["value"].as_str() {
                            out.insert(v.to_string());
                        }
                    }
                }
            }
        } else {
            out.insert(r);
        }
    }
    out
}

/// The visible rules whose bodies reference `node` directly, through any
/// hidden rule that is not a supertype.
fn referencing_rules(grammar: &Value, node: &str) -> Vec<String> {
    fn mentions(v: &Value, node: &str) -> bool {
        match v {
            Value::Object(o) => {
                (o.get("type").and_then(Value::as_str) == Some("SYMBOL")
                    && o.get("name").and_then(Value::as_str) == Some(node))
                    || o.values().any(|x| mentions(x, node))
            }
            Value::Array(a) => a.iter().any(|x| mentions(x, node)),
            _ => false,
        }
    }
    grammar["rules"]
        .as_object()
        .map(|r| {
            r.iter()
                .filter(|(k, v)| !k.starts_with('_') && *k != node && mentions(v, node))
                .map(|(k, _)| k.clone())
                .collect()
        })
        .unwrap_or_default()
}

/// Every visible rule: a named node type of the generated grammar.
/// Every named rule the start rule reaches, plus the externals. A rule
/// nothing reaches -- a lexical sort only ever inlined into another's
/// regex -- is no node of the grammar, and node-types.json will not list
/// it, so the ledger must not either.
fn named_nodes(grammar: &Value) -> Vec<String> {
    let rules = grammar["rules"].as_object().cloned().unwrap_or_default();
    let mut reached: BTreeSet<String> = BTreeSet::new();
    let mut todo: Vec<String> = rules.keys().take(1).cloned().collect();
    // Extras are roots too: a named comment is reached by no rule.
    symbols_in(&grammar["extras"], &mut todo);
    while let Some(r) = todo.pop() {
        if !reached.insert(r.clone()) {
            continue;
        }
        if let Some(body) = rules.get(&r) {
            let mut refs = Vec::new();
            symbols_in(body, &mut refs);
            todo.extend(refs);
        }
    }
    // A rule referenced only through a named alias is no node type of its
    // own; the alias's name is one.
    let mut bare: BTreeSet<String> = BTreeSet::new();
    let mut aliased: BTreeSet<String> = BTreeSet::new();
    let mut alias_names: BTreeSet<String> = BTreeSet::new();
    for body in rules.values().chain(grammar["extras"].as_array().into_iter().flatten()) {
        alias_refs(body, false, &mut bare, &mut aliased, &mut alias_names);
    }
    let mut v: Vec<String> = reached
        .into_iter()
        .filter(|k| !k.starts_with('_'))
        .filter(|k| bare.contains(k) || !aliased.contains(k))
        .chain(alias_names)
        .collect();
    if let Some(ext) = grammar["externals"].as_array() {
        for e in ext {
            if let Some(n) = e["name"].as_str() {
                if !n.starts_with('_') {
                    v.push(n.to_string());
                }
            }
        }
    }
    v.sort();
    v.dedup();
    v
}

/// Symbols referenced bare, symbols referenced as the content of a named
/// alias, and the alias names.
fn alias_refs(
    v: &Value,
    in_alias: bool,
    bare: &mut BTreeSet<String>,
    aliased: &mut BTreeSet<String>,
    alias_names: &mut BTreeSet<String>,
) {
    match v {
        Value::Object(o) => {
            let ty = o.get("type").and_then(Value::as_str);
            if ty == Some("SYMBOL") {
                if let Some(n) = o.get("name").and_then(Value::as_str) {
                    if in_alias {
                        aliased.insert(n.to_string());
                    } else {
                        bare.insert(n.to_string());
                    }
                }
                return;
            }
            let named_alias = ty == Some("ALIAS") && o.get("named") == Some(&Value::Bool(true));
            if named_alias {
                if let Some(n) = o.get("value").and_then(Value::as_str) {
                    alias_names.insert(n.to_string());
                }
            }
            for x in o.values() {
                alias_refs(x, in_alias || named_alias, bare, aliased, alias_names);
            }
        }
        Value::Array(a) => a
            .iter()
            .for_each(|x| alias_refs(x, in_alias, bare, aliased, alias_names)),
        _ => {}
    }
}

fn symbols_in(v: &Value, out: &mut Vec<String>) {
    match v {
        Value::Object(o) => {
            if o.get("type").and_then(Value::as_str) == Some("SYMBOL") {
                if let Some(n) = o.get("name").and_then(Value::as_str) {
                    out.push(n.to_string());
                }
            }
            for x in o.values() {
                symbols_in(x, out);
            }
        }
        Value::Array(a) => a.iter().for_each(|x| symbols_in(x, out)),
        _ => {}
    }
}

fn rename(grammar: &mut Value, names: &mut Names, old: &str, new: &str) {
    let rules = grammar["rules"].as_object().cloned().unwrap_or_default();
    let mut renamed: Map<String, Value> = Map::new();
    for (k, v) in rules {
        let key = if k == old { new.to_string() } else { k };
        let mut v = v;
        replace_symbol(&mut v, old, new);
        renamed.insert(key, v);
    }
    grammar["rules"] = Value::Object(renamed);
    for key in ["supertypes", "conflicts", "inline"] {
        if let Some(arr) = grammar[key].as_array_mut() {
            for v in arr.iter_mut() {
                rename_str(v, old, new);
            }
        }
    }
    for v in names.sort_rule.values_mut() {
        if v == old {
            *v = new.to_string();
        }
    }
}

fn rename_str(v: &mut Value, old: &str, new: &str) {
    match v {
        Value::String(s) if s == old => *s = new.to_string(),
        Value::Array(a) => a.iter_mut().for_each(|x| rename_str(x, old, new)),
        _ => {}
    }
}

fn replace_symbol(v: &mut Value, old: &str, new: &str) {
    match v {
        Value::Object(o) => {
            if o.get("type").and_then(Value::as_str) == Some("SYMBOL")
                && o.get("name").and_then(Value::as_str) == Some(old)
            {
                o.insert("name".into(), json!(new));
                return;
            }
            for x in o.values_mut() {
                replace_symbol(x, old, new);
            }
            // A choice that now names the term twice names it once.
            if o.get("type").and_then(Value::as_str) == Some("CHOICE") {
                if let Some(members) = o.get_mut("members").and_then(Value::as_array_mut) {
                    let mut seen = BTreeSet::new();
                    members.retain(|m| {
                        let key = serde_json::to_string(m).unwrap_or_default();
                        seen.insert(key)
                    });
                }
            }
        }
        Value::Array(a) => a.iter_mut().for_each(|x| replace_symbol(x, old, new)),
        _ => {}
    }
}

/// Insert `term` as a supertype over `members`, after the last member's
/// rule, and route every reference to a member through it.
fn thread(grammar: &mut Value, term: &str, members: &[String], flatten: &BTreeSet<String>) {
    let rules = grammar["rules"].as_object().cloned().unwrap_or_default();
    let mut out: Map<String, Value> = Map::new();
    let last = rules
        .keys()
        .enumerate()
        .filter(|(_, k)| members.contains(k))
        .map(|(i, _)| i)
        .max();
    let mut alternatives: Vec<Value> = Vec::new();
    for m in members {
        if flatten.contains(m) {
            if let Some(inner) = rules.get(m).and_then(|r| r["members"].as_array()) {
                alternatives.extend(inner.iter().cloned());
                continue;
            }
        }
        alternatives.push(json!({"type": "SYMBOL", "name": m}));
    }
    let choice = json!({"type": "CHOICE", "members": alternatives});
    for (i, (k, v)) in rules.into_iter().enumerate() {
        let mut v = v;
        if k != term {
            for m in members {
                replace_symbol(&mut v, m, term);
            }
        }
        if !flatten.contains(&k) {
            out.insert(k, v);
        }
        if Some(i) == last {
            out.insert(term.to_string(), choice.clone());
        }
    }
    if last.is_none() {
        out.insert(term.to_string(), choice);
    }
    grammar["rules"] = Value::Object(out);
    if let Some(s) = grammar["supertypes"].as_array_mut() {
        s.retain(|v| v.as_str().is_none_or(|n| !flatten.contains(n)));
        s.push(json!(term));
    }
}
