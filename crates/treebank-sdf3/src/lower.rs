//! Lower an SDF3 module to a tree-sitter `grammar.json`, and say what was
//! lost on the way.
//!
//! The mapping, and the finding each half of it produced:
//!
//! - A **sort** with more than one production becomes a hidden supertype
//!   rule (`_exp`) listed in `supertypes`; each **constructor** becomes a
//!   named node under it. SDF3's injections (`Exp = ID`) are members of the
//!   supertype with no node of their own, which is exactly what a
//!   tree-sitter supertype member is. A sort with a single constructor and
//!   nothing else collapses to one named rule.
//! - A **template** becomes a `SEQ` of string tokens and symbols; its
//!   layout is dropped here and would feed a printer.
//! - A **priority chain** numbers its groups from the bottom, and each
//!   member production is wrapped in `PREC_LEFT` / `PREC_RIGHT` / `PREC`.
//!   `non-assoc` has no tree-sitter form and lowers to `PREC_LEFT` with a
//!   **widening** finding: tree-sitter will accept `a == b == c`.
//! - A **bracket** production becomes a named node, which SDF3's AST does not
//!   have: tree-sitter refuses a hidden supertype member with more than one
//!   visible child, and `( Exp )` has three. Recorded as a deviation.
//! - **Lexical sorts** become regex tokens. **LAYOUT** becomes `extras`.
//!   A **reject** of a literal, and `template options`' `ID = keyword
//!   {reject}`, become `word` plus `reserved.global`.
//! - **Restrictions** are absorbed: tree-sitter's lexer is longest-match,
//!   which is what a follow restriction on a lexical sort asks for.

use std::collections::{BTreeMap, BTreeSet};

use anyhow::{anyhow, bail, Result};
use serde_json::{json, Map, Value};

use crate::ast::*;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Kind {
    /// The construct lowered exactly.
    Mapped,
    /// Lowered to something that accepts more than SDF3 would.
    Widening,
    /// Nothing to emit: tree-sitter gets the same effect another way.
    Absorbed,
    /// A treebank extension outside SDF3 was used.
    Extension,
    /// Lowered, but the tree has a node SDF3's AST would not, or lacks
    /// one it would have.
    Deviation,
    /// Not lowered; the grammar is missing something.
    Unsupported,
}

#[derive(Debug, Clone)]
pub struct Finding {
    pub kind: Kind,
    pub what: String,
}

pub struct Lowered {
    pub grammar: Value,
    pub findings: Vec<Finding>,
    /// `src/scanner.c`, when the module's layout constraints call for one.
    pub scanner: Option<String>,
    /// The node and rule names this lowering chose, so a second backend can
    /// use the same ones and one corpus can serve both.
    pub names: Names,
    /// Context-free production index -> (priority level, associativity),
    /// as assigned. By index, since one constructor may have several
    /// productions at different levels (`Exp.BinaryExpression`).
    pub levels: BTreeMap<usize, (u32, Option<Attr>)>,
}

#[derive(Debug, Clone, Default)]
pub struct Names {
    /// Sort -> the rule a reference to it becomes (`_exp`, `program`, `id`).
    pub sort_rule: BTreeMap<String, String>,
    /// `Sort.Cons` -> the node name.
    pub node: BTreeMap<String, String>,
    /// Lexical sorts, so a backend can tell a token from a rule.
    pub lexical: BTreeSet<String>,
    /// Rule -> the node type it is aliased to: a sort whose constructor is
    /// another sort's (`HIf.TemplateIf` beside `QIf.TemplateIf`) keeps its
    /// own rule and shows as the shared node type.
    pub alias: BTreeMap<String, String>,
}

struct Ctx<'m> {
    module: &'m Module,
    /// sort -> the rule name a reference to it becomes.
    sort_rule: BTreeMap<String, String>,
    /// sort -> the node type its rule is aliased to at every reference.
    alias: BTreeMap<String, String>,
    lexical: BTreeMap<String, Vec<&'m Production>>,
    findings: Vec<Finding>,
    /// production index -> (level, assoc)
    levels: BTreeMap<usize, (u32, Option<Attr>)>,
    /// Word-shaped template literals, for `reserved`.
    keywords: BTreeSet<String>,
    /// `keyword = case-insensitive`: keywords are pattern tokens aliased to
    /// their spelling, not strings.
    ci_keywords: bool,
    rule_names: BTreeSet<String>,
    /// `Sort.Cons` -> node name, as chosen.
    node_names: BTreeMap<String, String>,
    /// Which literal occurrences the generated scanner owns, and why.
    plan: crate::scanner::Plan,
    /// Context-free productions in declaration order; `plan` keys on the index.
    prods: Vec<&'m Production>,
}

fn choice_or_single(mut bodies: Vec<Value>) -> Value {
    if bodies.len() == 1 {
        bodies.pop().unwrap()
    } else {
        json!({"type": "CHOICE", "members": bodies})
    }
}

pub fn lower(module: &Module) -> Result<Lowered> {
    let mut cx = Ctx {
        module,
        sort_rule: BTreeMap::new(),
        alias: BTreeMap::new(),
        lexical: BTreeMap::new(),
        findings: Vec::new(),
        levels: BTreeMap::new(),
        keywords: BTreeSet::new(),
        rule_names: BTreeSet::new(),
        node_names: BTreeMap::new(),
        plan: crate::scanner::Plan::default(),
        prods: module.productions(false).collect(),
        ci_keywords: module
            .template_options()
            .any(|o| matches!(o, TemplateOption::KeywordCaseInsensitive)),
    };
    for p in module.productions(true) {
        cx.lexical.entry(p.sort.clone()).or_default().push(p);
    }
    if !module.imports.is_empty() {
        cx.findings.push(Finding {
            kind: Kind::Mapped,
            what: format!(
                "imports [{}] merged additively by the loader: an imported sort gains this module's productions, nothing is overridden -- where tree-sitter's `extends` would flatten and override",
                module.imports.join(", ")
            ),
        });
    }
    for h in &module.holes {
        let mut what = format!(
            "sort {} has no production in this composition (a dialect point this target leaves empty)",
            h.sort
        );
        if !h.blanked.is_empty() {
            what.push_str(&format!(
                "; its optional occurrence was removed from [{}]",
                h.blanked.join(", ")
            ));
        }
        if !h.dropped.is_empty() {
            what.push_str(&format!(
                "; [{}] needed it and were dropped",
                h.dropped.join(", ")
            ));
        }
        cx.findings.push(Finding {
            kind: Kind::Mapped,
            what,
        });
    }
    let (plan, mut plan_findings) = crate::scanner::plan(module)?;
    cx.findings.append(&mut plan_findings);
    cx.plan = plan;
    cx.assign_levels();

    let mut rules: Map<String, Value> = Map::new();
    let mut supertypes: Vec<String> = Vec::new();

    // Context-free sorts, start symbol first so it is the root rule.
    let mut cf: BTreeMap<String, Vec<&Production>> = BTreeMap::new();
    let mut order: Vec<String> = Vec::new();
    for p in module.productions(false) {
        if !cf.contains_key(&p.sort) {
            order.push(p.sort.clone());
        }
        cf.entry(p.sort.clone()).or_default().push(p);
    }
    let starts = module.start_symbols();
    let start = starts
        .first()
        .ok_or_else(|| anyhow!("no context-free start-symbols"))?
        .to_string();
    order.retain(|s| *s != start);
    order.insert(0, start.clone());

    // Decide every sort's rule name before emitting any body, since bodies
    // refer to other sorts.
    let mut taken: BTreeSet<String> = BTreeSet::new();
    for sort in &order {
        let prods = &cf[sort];
        let constructors: BTreeSet<Option<&str>> =
            prods.iter().map(|p| p.constructor.as_deref()).collect();
        let one_cons = constructors.len() == 1
            && prods[0].constructor.is_some()
            && !prods.iter().any(|p| p.has(&Attr::Bracket));
        let name = if sort.starts_with('_') {
            // A hidden sort: a hidden rule, its symbols the parent's.
            snake(sort)
        } else if one_cons {
            // SDF3's AST node is the constructor, so `Else.ElseClause`
            // is `else_clause`, not `else`.
            let node = snake(prods[0].constructor.as_deref().unwrap_or(sort));
            if taken.contains(&node) {
                // The constructor is another sort's too: one term, one
                // node type, so this sort's rule is aliased to it.
                cx.alias.insert(sort.clone(), node.clone());
                format!("{}_{}", snake(sort), node)
            } else {
                node
            }
        } else {
            format!("_{}", snake(sort))
        };
        taken.insert(name.clone());
        cx.sort_rule.insert(sort.clone(), name);
    }
    for (sort, prods) in &cx.lexical {
        if sort == "LAYOUT" {
            continue;
        }
        if let Some(external) = cx.plan.lexical_owned.get(sort) {
            cx.sort_rule.insert(sort.clone(), external.clone());
            continue;
        }
        let rejects_only = prods.iter().all(|p| p.has(&Attr::Reject));
        if !rejects_only {
            cx.sort_rule.insert(sort.clone(), snake(sort));
        }
    }

    let mut inline: Vec<String> = Vec::new();
    for sort in &order {
        let prods = cf[sort].clone();
        let rule_name = cx.sort_rule[sort].clone();
        if sort.starts_with('_') {
            // A hidden sort (a treebank extension: SDF3 has no constructor-
            // less production of more than one symbol). Its productions
            // become one hidden rule, not a supertype, and the symbols
            // are the parent's children. A rule made of tokens only is
            // inlined, so a precedence on the parent reaches its tokens:
            // tree-sitter resolves a shift by the precedence of the
            // production the token is in, and `_bin_op_mul` has none.
            let mut bodies = Vec::new();
            for p in &prods {
                let b = cx.production_body(p)?;
                bodies.push(cx.wrap_precedence(p, b));
            }
            let token_only = prods.iter().all(|p| {
                p.symbols().iter().all(|s| match s {
                    SymRef::Lit(_) => true,
                    SymRef::Sym(Symbol::Sort(n)) => cx.lexical.contains_key(n),
                    SymRef::Sym(_) => false,
                })
            });
            if token_only {
                inline.push(rule_name.clone());
            }
            cx.findings.push(Finding {
                kind: Kind::Extension,
                what: format!(
                    "`_`-prefixed sort {sort} is hidden (not SDF3): its {} production(s) became the hidden rule `{rule_name}`, whose symbols are its parent's children{}",
                    prods.len(),
                    if token_only { "; made of tokens only, it is inlined so a precedence on the parent reaches its tokens" } else { "" }
                ),
            });
            cx.insert_rule(&mut rules, rule_name, choice_or_single(bodies))?;
            continue;
        }
        if !rule_name.starts_with('_') {
            // One constructor: one named rule, a choice when the
            // constructor has several productions.
            let mut bodies = Vec::new();
            for p in &prods {
                let b = cx.production_body(p)?;
                bodies.push(cx.wrap_precedence(p, b));
            }
            let cons = prods[0].constructor.as_deref().unwrap_or("?").to_string();
            let node = cx.alias.get(sort).cloned().unwrap_or_else(|| rule_name.clone());
            if prods.len() == 1 {
                cx.findings.push(Finding {
                    kind: Kind::Mapped,
                    what: format!(
                        "sort {sort} has the single constructor {cons}; collapsed to the named rule `{rule_name}`"
                    ),
                });
            } else {
                cx.findings.push(Finding {
                    kind: Kind::Mapped,
                    what: format!(
                        "sort {sort}'s {} productions share the constructor {cons}: one named rule `{rule_name}` with a choice of their bodies, since SDF3's AST has one constructor",
                        prods.len()
                    ),
                });
            }
            if node != rule_name {
                cx.findings.push(Finding {
                    kind: Kind::Mapped,
                    what: format!(
                        "sort {sort}'s constructor {cons} is also another sort's: the rule `{rule_name}` is aliased to the node type `{node}` wherever {sort} is referenced, so the two are one term in the tree"
                    ),
                });
            }
            for p in &prods {
                if let Some(r) = p.reference() {
                    cx.node_names.insert(r, node.clone());
                }
            }
            cx.insert_rule(&mut rules, rule_name, choice_or_single(bodies))?;
            continue;
        }
        let mut members: Vec<Value> = Vec::new();
        // Constructors in first-appearance order, each with its productions.
        let mut groups: Vec<(Option<String>, Vec<&Production>)> = Vec::new();
        for p in &prods {
            let key = if p.has(&Attr::Bracket) {
                None
            } else {
                p.constructor.clone()
            };
            match groups.iter_mut().find(|(k, _)| *k == key && key.is_some()) {
                Some((_, g)) => g.push(p),
                None => groups.push((key, vec![p])),
            }
        }
        for (cons, group) in groups {
            let Some(c) = cons else {
                for p in group {
                    if p.has(&Attr::Bracket) {
                        // tree-sitter refuses a hidden supertype member with more
                        // than one visible child ("Supertype symbols must always
                        // have a single visible child"), and `( Exp )` has three.
                        // So the bracket becomes a named node, which SDF3's AST does
                        // not have.
                        let node = format!("{}_bracket", snake(sort));
                        let body = cx.production_body(p)?;
                        cx.insert_rule(&mut rules, node.clone(), body)?;
                        members.push(json!({"type": "SYMBOL", "name": node}));
                        cx.findings.push(Finding {
                            kind: Kind::Deviation,
                            what: format!("bracket production of {sort} became the named node `{node}`; SDF3's AST has no node for brackets, but a hidden supertype member may have only one visible child and `( {sort} )` has three"),
                        });
                        continue;
                    }
                    // Injection: the member is whatever the rhs names.
                    let body = cx.production_body(p)?;
                    if body.get("type").and_then(Value::as_str) != Some("SYMBOL")
                        && body.get("type").and_then(Value::as_str) != Some("ALIAS")
                    {
                        bail!("injection {sort} = ... has a right-hand side that is not a single symbol; unsupported");
                    }
                    members.push(body);
                    cx.findings.push(Finding {
                        kind: Kind::Mapped,
                        what: format!("injection into {sort} became a supertype member with no node of its own"),
                    });
                }
                continue;
            };
            let mut node = snake(&c);
            if cx.rule_names.contains(&node) || cx.sort_rule.values().any(|v| *v == node) {
                node = format!("{}_{}", snake(sort), node);
            }
            let mut bodies = Vec::new();
            for p in &group {
                let body = cx.production_body(p)?;
                bodies.push(cx.wrap_precedence(p, body));
                if let Some(r) = p.reference() {
                    cx.node_names.insert(r, node.clone());
                }
            }
            if group.len() > 1 {
                cx.findings.push(Finding {
                    kind: Kind::Mapped,
                    what: format!(
                        "{sort}.{c} has {} productions: one named rule `{node}` with a choice of their bodies, each at its own precedence, since SDF3's AST has one constructor",
                        group.len()
                    ),
                });
            }
            cx.insert_rule(&mut rules, node.clone(), choice_or_single(bodies))?;
            members.push(json!({"type": "SYMBOL", "name": node}));
        }
        cx.insert_rule(
            &mut rules,
            rule_name.clone(),
            json!({"type": "CHOICE", "members": members}),
        )?;
        supertypes.push(rule_name);
    }

    // Lexical sorts.
    let mut extras: Vec<Value> = Vec::new();
    let mut reserved: BTreeSet<String> = BTreeSet::new();
    let mut word: Option<String> = None;
    let lexical_sorts: Vec<String> = cx.lexical.keys().cloned().collect();
    // A lexical sort only other lexical sorts refer to (`HEX` inside
    // `STRING_LIT`) is inlined into their regexes and is no token.
    let cf_referenced = cf_referenced_sorts(module);
    let lex_referenced: BTreeSet<String> = {
        let b = crate::nfa::Builder::new(module);
        lexical_sorts
            .iter()
            .flat_map(|s| b.referenced(s))
            .collect()
    };
    for sort in &lexical_sorts {
        let prods = cx.lexical[sort].clone();
        if sort == "LAYOUT" {
            let mut comment_n = 0;
            for p in &prods {
                let re = cx.regex_of(p)?;
                let mut chars = Vec::new();
                let is_ws = p.constructor.is_none()
                    && matches!(&p.rhs, Rhs::Symbols(s) if s.iter().all(|sym| crate::scanner::whitespace_alphabet(sym, &mut chars)));
                if is_ws {
                    extras.push(json!({"type": "PATTERN", "value": re}));
                    cx.findings.push(Finding {
                        kind: Kind::Mapped,
                        what: format!("LAYOUT class became an extras pattern /{re}/"),
                    });
                } else {
                    let name = match &p.constructor {
                        Some(c) => snake(c),
                        None => {
                            comment_n += 1;
                            if comment_n == 1 {
                                "comment".to_string()
                            } else {
                                format!("comment_{comment_n}")
                            }
                        }
                    };
                    cx.insert_rule(
                        &mut rules,
                        name.clone(),
                        json!({"type": "PATTERN", "value": re}),
                    )?;
                    extras.push(json!({"type": "SYMBOL", "name": name}));
                    cx.findings.push(Finding {
                        kind: Kind::Mapped,
                        what: format!("LAYOUT production became the named extra `{name}` /{re}/"),
                    });
                }
            }
            continue;
        }
        if let Some(external) = cx.plan.lexical_owned.get(sort).cloned() {
            cx.findings.push(Finding {
                kind: Kind::Mapped,
                what: format!("lexical sort {sort} is scanned by the generated scanner as `{external}`; no token rule emitted"),
            });
            continue;
        }
        if !cf_referenced.contains(sort) && lex_referenced.contains(sort) {
            cx.findings.push(Finding {
                kind: Kind::Mapped,
                what: format!("lexical sort {sort} is referenced by lexical syntax only; inlined into the tokens that use it, no token of its own"),
            });
            continue;
        }
        let (keep, rejects): (Vec<&Production>, Vec<&Production>) =
            prods.iter().copied().partition(|p| !p.has(&Attr::Reject));
        for r in rejects {
            match &r.rhs {
                Rhs::Symbols(s) if s.len() == 1 => {
                    if let Symbol::Lit(w) = &s[0] {
                        reserved.insert(w.clone());
                        cx.findings.push(Finding {
                            kind: Kind::Mapped,
                            what: format!("`{sort} = \"{w}\" {{reject}}` became a reserved word"),
                        });
                        continue;
                    }
                }
                _ => {}
            }
            cx.findings.push(Finding { kind: Kind::Unsupported, what: format!("a reject production on {sort} that is not a single literal has no tree-sitter form") });
        }
        if keep.is_empty() {
            continue;
        }
        let alts: Vec<String> = keep.iter().map(|p| cx.regex_of(p)).collect::<Result<_>>()?;
        let re = if alts.len() == 1 {
            alts[0].clone()
        } else {
            format!("(?:{})", alts.join("|"))
        };
        let name = snake(sort);
        cx.insert_rule(
            &mut rules,
            name.clone(),
            json!({"type": "PATTERN", "value": re}),
        )?;
        cx.findings.push(Finding {
            kind: Kind::Mapped,
            what: format!("lexical sort {sort} became the token `{name}` /{re}/"),
        });
    }

    for opt in module.template_options() {
        match opt {
            TemplateOption::KeywordReject { sort } => {
                let w = cx
                    .sort_rule
                    .get(sort)
                    .cloned()
                    .ok_or_else(|| anyhow!("template options rejects keywords as {sort}, which is not a lexical sort here"))?;
                word = Some(w.clone());
                let re = rules[&w]["value"].as_str().unwrap_or("").to_string();
                let regex = regex_lite_compile(&re);
                for k in &cx.keywords {
                    if regex.as_ref().map(|r| r(k)).unwrap_or(false) {
                        reserved.insert(k.clone());
                    }
                }
                cx.findings.push(Finding {
                    kind: Kind::Mapped,
                    what: format!(
                        "`{sort} = keyword {{reject}}` became `word: {w}` plus reserved.global = [{}]",
                        reserved.iter().cloned().collect::<Vec<_>>().join(", ")
                    ),
                });
            }
            TemplateOption::KeywordPrefer { sort } => {
                let w = cx
                    .sort_rule
                    .get(sort)
                    .cloned()
                    .ok_or_else(|| anyhow!("template options prefers keywords over {sort}, which is not a lexical sort here"))?;
                word = Some(w.clone());
                cx.findings.push(Finding {
                    kind: Kind::Extension,
                    what: format!(
                        "`{sort} = keyword {{prefer}}` (not SDF3's template options): `word: {w}` with no reserved set, so each of the {} word-shaped literals is a keyword only where the parse admits it and a {w} elsewhere, and where both are admitted the keyword wins -- tree-sitter's keyword extraction, which is HCL's own rule",
                        cx.keywords.len()
                    ),
                });
            }
            TemplateOption::KeywordFollow(_) => cx.findings.push(Finding {
                kind: Kind::Absorbed,
                what: "`keyword -/- [class]`: tree-sitter's keyword extraction already refuses to lex a keyword that is a prefix of a longer word".into(),
            }),
            // Reported where the keyword rules are emitted, below.
            TemplateOption::KeywordCaseInsensitive => {}
            TemplateOption::Tokenize(s) => cx.findings.push(Finding {
                kind: Kind::Mapped,
                what: format!("`tokenize: {s:?}`: the reader split template literal runs at these characters, so each is its own token"),
            }),
        }
    }
    for r in module.restrictions(true) {
        cx.findings.push(Finding {
            kind: Kind::Absorbed,
            what: format!(
                "lexical restriction on {}: longest-match tokenisation gives the same effect",
                r.symbols.join(" ")
            ),
        });
    }
    for r in module.restrictions(false) {
        cx.findings.push(Finding {
            kind: Kind::Absorbed,
            what: format!(
                "context-free restriction on {}: extras are skipped greedily",
                r.symbols.join(" ")
            ),
        });
    }
    if word.is_none() {
        let unreserved: Vec<&String> = cx
            .keywords
            .iter()
            .filter(|k| !reserved.contains(*k))
            .collect();
        if !unreserved.is_empty() {
            cx.findings.push(Finding {
                kind: Kind::Widening,
                what: format!(
                    "no `template options` keyword rejection: {} word-shaped literals are unreserved and may lex as identifiers (notes/field_guide.md §5)",
                    unreserved.len()
                ),
            });
        }
    }

    let mut grammar = Map::new();
    grammar.insert(
        "$schema".into(),
        json!("https://tree-sitter.github.io/tree-sitter/assets/schemas/grammar.schema.json"),
    );
    grammar.insert("name".into(), json!(module.symbol_name()));
    if let Some(w) = &word {
        grammar.insert("word".into(), json!(w));
    }
    // A rejected word that no production uses (`ID = "async" {reject}` in
    // an edition module) is a word tree-sitter cannot reserve, since only
    // a token can be reserved and a token is a literal some rule reaches.
    // So the words are reached: from the start rule, behind a pattern
    // that matches nothing. The lexer then knows them as keywords, the
    // reserved set refuses them as identifiers, and no parse can shift
    // them. The effect is SDF3's: the word is a syntax error anywhere.
    // `keyword = case-insensitive`: one pattern token per keyword, matched
    // in any case with lexical precedence over the word token, reserved
    // by symbol. Keyword extraction (`word`) stays: it is what makes the
    // reserved set bite where an identifier is expected.
    if cx.ci_keywords {
        let all: BTreeSet<String> = cx.keywords.union(&reserved).cloned().collect();
        for w in &all {
            rules.insert(
                kw_rule(w),
                json!({"type": "TOKEN", "content": {"type": "PREC", "value": 1, "content": {"type": "PATTERN", "value": ci_regex(w)}}}),
            );
        }
        cx.findings.push(Finding {
            kind: Kind::Extension,
            what: format!(
                "`keyword = case-insensitive` (not SDF3's template options; its productive form is the `'kw'` literal): {} keywords became `token(prec(1, /[sS][eE]../))` rules aliased to their spelling and reserved by symbol; the tree and the printer show the template's spelling",
                all.len()
            ),
        });
    }
    let ci = cx.ci_keywords;
    let reserved_value = move |w: &str| -> Value {
        if ci {
            json!({"type": "SYMBOL", "name": kw_rule(w)})
        } else {
            json!({"type": "STRING", "value": w})
        }
    };
    let orphans: Vec<&String> = reserved
        .iter()
        .filter(|w| !cx.keywords.contains(*w))
        .collect();
    if !orphans.is_empty() {
        let words: Vec<Value> = orphans.iter().map(|w| reserved_value(w)).collect();
        rules.insert(
            "_reserved_word".into(),
            json!({"type": "SEQ", "members": [
                {"type": "PATTERN", "value": "[^\\s\\S]"},
                {"type": "CHOICE", "members": words}
            ]}),
        );
        if let Some(body) = rules.get_mut(&cx.sort_rule[&start]) {
            let orig = body.take();
            *body = json!({"type": "SEQ", "members": [orig, optional(json!({"type": "SYMBOL", "name": "_reserved_word"}))]});
        }
        cx.findings.push(Finding {
            kind: Kind::Mapped,
            what: format!(
                "rejected words used by no production [{}]: reserved, and made tokens by a hidden `_reserved_word` rule the start rule reaches only behind a pattern matching nothing, so each is a syntax error wherever it appears",
                orphans.iter().map(|s| s.as_str()).collect::<Vec<_>>().join(", ")
            ),
        });
    }
    grammar.insert("rules".into(), Value::Object(rules));
    grammar.insert("extras".into(), Value::Array(extras));
    grammar.insert("conflicts".into(), json!([]));
    grammar.insert("precedences".into(), json!([]));
    let externals: Vec<Value> = if cx.plan.is_empty() {
        Vec::new()
    } else {
        cx.plan
            .externals()
            .into_iter()
            .map(|n| json!({"type": "SYMBOL", "name": n}))
            .collect()
    };
    grammar.insert("externals".into(), Value::Array(externals));
    grammar.insert("inline".into(), json!(inline));
    grammar.insert("supertypes".into(), json!(supertypes));
    if !reserved.is_empty() {
        let words: Vec<Value> = reserved.iter().map(|w| reserved_value(w)).collect();
        grammar.insert("reserved".into(), json!({"global": words}));
    }
    let scanner = if cx.plan.is_empty() {
        None
    } else {
        Some(crate::scanner::c_source(&cx.plan, &module.symbol_name()))
    };
    let names = Names {
        sort_rule: cx.sort_rule.clone(),
        node: cx.node_names.clone(),
        lexical: cx.lexical.keys().cloned().collect(),
        alias: cx
            .alias
            .iter()
            .map(|(sort, node)| (cx.sort_rule[sort].clone(), node.clone()))
            .collect(),
    };
    Ok(Lowered {
        grammar: Value::Object(grammar),
        findings: cx.findings,
        scanner,
        names,
        levels: cx.levels.clone(),
    })
}

impl<'m> Ctx<'m> {
    fn insert_rule(
        &mut self,
        rules: &mut Map<String, Value>,
        name: String,
        body: Value,
    ) -> Result<()> {
        if rules.contains_key(&name) {
            bail!("two rules would be named `{name}`");
        }
        self.rule_names.insert(name.clone());
        rules.insert(name, body);
        Ok(())
    }

    fn assign_levels(&mut self) {
        let chains: Vec<&PriorityChain> = self.module.priorities().collect();
        if chains.len() > 1 {
            self.findings.push(Finding {
                kind: Kind::Widening,
                what: format!(
                    "{} independent priority chains; tree-sitter precedence is one global order, so their levels are numbered together and may interact",
                    chains.len()
                ),
            });
        }
        let mut level = 0u32;
        let mut missing: Vec<String> = Vec::new();
        for chain in chains.iter().rev() {
            for group in chain.groups.iter().rev() {
                level += 1;
                for m in &group.members {
                    let mut any = false;
                    for (pi, p) in self.prods.iter().enumerate() {
                        if p.reference().as_deref() == Some(m.as_str()) {
                            self.levels.insert(pi, (level, group.assoc.clone()));
                            any = true;
                        }
                    }
                    if !any {
                        missing.push(m.clone());
                    }
                }
                for q in &group.prods {
                    let mut any = false;
                    for (pi, p) in self.prods.iter().enumerate() {
                        if p.same_as(q) {
                            self.levels.insert(pi, (level, group.assoc.clone()));
                            any = true;
                        }
                    }
                    if !any {
                        missing.push(format!("{} = ...", q.display()));
                    }
                }
            }
        }
        for m in missing {
            self.findings.push(Finding {
                kind: Kind::Unsupported,
                what: format!("priorities name `{m}`, which is no production of the module; ignored"),
            });
        }
        // Per-production associativity outside any chain.
        for (pi, p) in self.prods.clone().iter().enumerate() {
            if p.reference().is_none() {
                continue;
            }
            let attr = p
                .attrs
                .iter()
                .find(|a| matches!(a, Attr::Left | Attr::Right | Attr::NonAssoc))
                .cloned();
            match self.levels.get_mut(&pi) {
                Some(entry) => {
                    if entry.1.is_none() {
                        entry.1 = attr;
                    }
                }
                None => {
                    if attr.is_some() {
                        self.levels.insert(pi, (0, attr));
                    }
                }
            }
        }
    }

    fn wrap_precedence(&mut self, p: &Production, body: Value) -> Value {
        let mut body = body;
        let pi = self.prods.iter().position(|q| std::ptr::eq(*q, p));
        if let (Some(pi), Some(r)) = (pi, p.reference()) {
            if let Some((level, assoc)) = self.levels.get(&pi).cloned() {
                let ty = match assoc {
                    Some(Attr::Left) => "PREC_LEFT",
                    Some(Attr::Right) => "PREC_RIGHT",
                    Some(Attr::NonAssoc) => {
                        self.findings.push(Finding {
                            kind: Kind::Widening,
                            what: format!("{r} is non-assoc; tree-sitter has no non-associativity, lowered to PREC_LEFT so `a == b == c` parses where SDF3 rejects it"),
                        });
                        "PREC_LEFT"
                    }
                    _ => "PREC",
                };
                self.findings.push(Finding {
                    kind: Kind::Mapped,
                    what: format!("{r} at priority level {level} became {ty} at that level"),
                });
                body = json!({"type": ty, "value": level, "content": body});
            }
        }
        // `prefer` and `avoid` settle an ambiguity between complete parses.
        // tree-sitter's counterpart is dynamic precedence, which only acts
        // where a conflict is declared; if the scanner split has made the
        // readings disjoint, the weight is inert, and generate says which.
        for (attr, weight, word) in [(Attr::Prefer, 1, "prefer"), (Attr::Avoid, -1, "avoid")] {
            if p.has(&attr) {
                self.findings.push(Finding {
                    kind: Kind::Mapped,
                    what: format!("{}: `{{{word}}}` became dynamic precedence {weight:+}; it decides only where a conflict is declared", p.display()),
                });
                body = json!({"type": "PREC_DYNAMIC", "value": weight, "content": body});
            }
        }
        body
    }

    fn production_body(&mut self, p: &Production) -> Result<Value> {
        let pi = self.prods.iter().position(|q| std::ptr::eq(*q, p));
        let mut members = Vec::new();
        match &p.rhs {
            Rhs::Template(parts) => {
                let mut pos = 0;
                for part in parts {
                    match part {
                        TemplatePart::Layout(_) => {}
                        TemplatePart::Lit(s) => {
                            pos += 1;
                            members.push(self.literal(pi, pos, s));
                        }
                        TemplatePart::Placeholder { label, symbol } => {
                            pos += 1;
                            if crate::scanner::is_layout_symbol(SymRef::Sym(symbol)) {
                                // `LAYOUT?` in kernel syntax: layout is admitted
                                // here, which is what extras do everywhere.
                                continue;
                            }
                            let mut v = self.symbol_at(pi, pos, symbol)?;
                            if let Some(l) = label {
                                self.findings.push(Finding {
                                    kind: Kind::Extension,
                                    what: format!(
                                        "{}: placeholder label `{l}` became a field (not SDF3)",
                                        p.display()
                                    ),
                                });
                                v = json!({"type": "FIELD", "name": l, "content": v});
                            }
                            self.push_symbol(&mut members, pi, pos, v);
                        }
                    }
                }
            }
            Rhs::Symbols(syms) => {
                for (k, s) in syms.iter().enumerate() {
                    match s {
                        Symbol::Lit(l) => members.push(self.literal(pi, k + 1, l)),
                        other => {
                            if crate::scanner::is_layout_symbol(SymRef::Sym(other)) {
                                continue;
                            }
                            let v = self.symbol_at(pi, k + 1, other)?;
                            self.push_symbol(&mut members, pi, k + 1, v);
                        }
                    }
                }
            }
        }
        if let (Some(pi), Some(ind)) = (pi, &self.plan.indent) {
            if ind.terminated.contains(&pi) {
                members.push(json!({"type": "SYMBOL", "name": "_newline"}));
            }
        }
        Ok(seq(members))
    }

    /// A nonterminal at a symbol position, wrapped in `_indent .. _dedent`
    /// when the indent plan says the occurrence is an indented block.
    fn push_symbol(&self, members: &mut Vec<Value>, pi: Option<usize>, pos: usize, v: Value) {
        let wrapped = pi.is_some_and(|pi| {
            self.plan
                .indent
                .as_ref()
                .is_some_and(|ind| ind.blocks.contains(&(pi, pos)))
        });
        if wrapped {
            members.push(json!({"type": "SYMBOL", "name": "_indent"}));
            members.push(v);
            members.push(json!({"type": "SYMBOL", "name": "_dedent"}));
        } else {
            members.push(v);
        }
    }

    /// A literal at a top-level symbol position: a string token, unless the
    /// scanner plan owns this occurrence, in which case the external variant
    /// aliased back to the spelling so the tree still shows `-`.
    fn literal(&mut self, pi: Option<usize>, pos: usize, s: &str) -> Value {
        self.note_literal(s);
        if let Some(pi) = pi {
            if let Some(external) = self.plan.occurrences.get(&(pi, pos)).cloned() {
                return json!({
                    "type": "ALIAS",
                    "content": {"type": "SYMBOL", "name": external},
                    "named": false,
                    "value": s
                });
            }
        }
        self.keyword_value(s)
    }

    /// A literal as a token: a string, or under `keyword = case-insensitive`
    /// a word-shaped one becomes the `_kw_` pattern token aliased back to the
    /// template's spelling, so the tree shows `SELECT` whatever was typed.
    fn keyword_value(&self, s: &str) -> Value {
        if self.ci_keywords && is_word(s) {
            json!({"type": "ALIAS", "content": {"type": "SYMBOL", "name": kw_rule(s)}, "named": false, "value": s})
        } else {
            json!({"type": "STRING", "value": s})
        }
    }

    fn note_literal(&mut self, s: &str) {
        if is_word(s) {
            self.keywords.insert(s.to_string());
        }
    }

    /// A symbol at a position of a production: `token.immediate` when the
    /// scanner plan says the occurrence must be adjacent to what precedes
    /// it, which tree-sitter's lexer can say of a token and of nothing else.
    fn symbol_at(&mut self, pi: Option<usize>, pos: usize, s: &Symbol) -> Result<Value> {
        let immediate = pi.is_some_and(|pi| self.plan.immediate.contains(&(pi, pos)));
        if immediate {
            if let Symbol::Sort(name) = s {
                if let Some(prods) = self.lexical.get(name).cloned() {
                    let alts: Vec<String> = prods
                        .iter()
                        .filter(|p| !p.has(&Attr::Reject))
                        .map(|p| self.regex_of(p))
                        .collect::<Result<_>>()?;
                    let re = if alts.len() == 1 {
                        alts[0].clone()
                    } else {
                        format!("(?:{})", alts.join("|"))
                    };
                    let token = json!({"type": "IMMEDIATE_TOKEN", "content": {"type": "PATTERN", "value": re}});
                    return Ok(if name.starts_with('_') {
                        token
                    } else {
                        json!({"type": "ALIAS", "content": token, "named": true, "value": snake(name)})
                    });
                }
            }
        }
        self.symbol(s)
    }

    fn symbol(&mut self, s: &Symbol) -> Result<Value> {
        Ok(match s {
            Symbol::Sort(name) if name == "LAYOUT" => json!({"type": "BLANK"}),
            Symbol::Sort(name) => {
                let rule = self
                    .sort_rule
                    .get(name)
                    .cloned()
                    .ok_or_else(|| anyhow!("reference to undefined sort {name}"))?;
                match self.alias.get(name) {
                    Some(node) => {
                        json!({"type": "ALIAS", "content": {"type": "SYMBOL", "name": rule}, "named": true, "value": node})
                    }
                    None => json!({"type": "SYMBOL", "name": rule}),
                }
            }
            Symbol::Lit(l) => {
                self.note_literal(l);
                self.keyword_value(l)
            }
            Symbol::CharClass(c) => json!({"type": "PATTERN", "value": class_regex(c)}),
            Symbol::Star(inner) => json!({"type": "REPEAT", "content": self.symbol(inner)?}),
            Symbol::Plus(inner) => json!({"type": "REPEAT1", "content": self.symbol(inner)?}),
            Symbol::Opt(inner) => optional(self.symbol(inner)?),
            Symbol::SepList { elem, sep, min } => {
                let e = self.symbol(elem)?;
                let sp = self.symbol(sep)?;
                let one_or_more = seq(vec![
                    e.clone(),
                    json!({"type": "REPEAT", "content": seq(vec![sp, e])}),
                ]);
                self.findings.push(Finding {
                    kind: Kind::Mapped,
                    what: format!("a `{{Elem Sep}}{}` list expanded to seq/repeat; the expansion has no name in grammar.json", if *min == 0 { "*" } else { "+" }),
                });
                if *min == 0 {
                    optional(one_or_more)
                } else {
                    one_or_more
                }
            }
            Symbol::Group(alts) => {
                let members: Vec<Value> = alts
                    .iter()
                    .map(|a| {
                        let ms: Vec<Value> =
                            a.iter().map(|s| self.symbol(s)).collect::<Result<_>>()?;
                        Ok(seq(ms))
                    })
                    .collect::<Result<_>>()?;
                json!({"type": "CHOICE", "members": members})
            }
        })
    }

    /// The regex a lexical production denotes. Sort references inline the
    /// referenced sort's own regex, so lexical sorts must not be recursive.
    fn regex_of(&self, p: &Production) -> Result<String> {
        match &p.rhs {
            Rhs::Symbols(syms) => syms
                .iter()
                .map(|s| self.symbol_regex(s))
                .collect::<Result<Vec<_>>>()
                .map(|v| v.concat()),
            Rhs::Template(_) => bail!("lexical syntax for {} uses a template; unsupported", p.sort),
        }
    }

    fn symbol_regex(&self, s: &Symbol) -> Result<String> {
        Ok(match s {
            Symbol::CharClass(c) => class_regex(c),
            Symbol::Lit(l) => regex_escape(l),
            Symbol::Sort(name) => {
                let prods = self
                    .lexical
                    .get(name)
                    .ok_or_else(|| anyhow!("lexical sort {name} referenced but not defined"))?;
                let alts: Vec<String> = prods
                    .iter()
                    .filter(|p| !p.has(&Attr::Reject))
                    .map(|p| self.regex_of(p))
                    .collect::<Result<_>>()?;
                format!("(?:{})", alts.join("|"))
            }
            Symbol::Star(i) => format!("(?:{})*", self.symbol_regex(i)?),
            Symbol::Plus(i) => format!("(?:{})+", self.symbol_regex(i)?),
            Symbol::Opt(i) => format!("(?:{})?", self.symbol_regex(i)?),
            Symbol::Group(alts) => {
                let parts: Vec<String> = alts
                    .iter()
                    .map(|a| {
                        a.iter()
                            .map(|s| self.symbol_regex(s))
                            .collect::<Result<Vec<_>>>()
                            .map(|v| v.concat())
                    })
                    .collect::<Result<_>>()?;
                format!("(?:{})", parts.join("|"))
            }
            Symbol::SepList { .. } => bail!("a separated list in lexical syntax is unsupported"),
        })
    }
}

/// Every sort context-free syntax, the vocabulary or the template options
/// refer to. A lexical sort outside this set is only ever part of another
/// token's text: tree-sitter inlines it, ANTLR makes it a `fragment`.
pub fn cf_referenced_sorts(module: &Module) -> BTreeSet<String> {
    fn walk(s: &Symbol, out: &mut BTreeSet<String>) {
        match s {
            Symbol::Sort(n) => {
                out.insert(n.clone());
            }
            Symbol::Star(i) | Symbol::Plus(i) | Symbol::Opt(i) => walk(i, out),
            Symbol::SepList { elem, sep, .. } => {
                walk(elem, out);
                walk(sep, out);
            }
            Symbol::Group(alts) => alts.iter().flatten().for_each(|s| walk(s, out)),
            _ => {}
        }
    }
    let mut out = BTreeSet::new();
    for p in module.productions(false) {
        for s in p.symbols() {
            if let SymRef::Sym(s) = s {
                walk(s, &mut out);
            }
        }
    }
    for t in module.vocabulary() {
        out.extend(t.members.iter().cloned());
    }
    for o in module.template_options() {
        if let TemplateOption::KeywordReject { sort } | TemplateOption::KeywordPrefer { sort } = o {
            out.insert(sort.clone());
        }
    }
    out
}

fn seq(mut members: Vec<Value>) -> Value {
    if members.len() == 1 {
        members.pop().unwrap()
    } else {
        json!({"type": "SEQ", "members": members})
    }
}

fn optional(v: Value) -> Value {
    json!({"type": "CHOICE", "members": [v, {"type": "BLANK"}]})
}

fn class_regex(c: &CharClass) -> String {
    let mut s = String::from("[");
    if c.negated {
        s.push('^');
    }
    for (a, b) in &c.ranges {
        s.push_str(&class_char(*a));
        if a != b {
            s.push('-');
            s.push_str(&class_char(*b));
        }
    }
    s.push(']');
    s
}

fn class_char(c: char) -> String {
    match c {
        '\n' => "\\n".into(),
        '\r' => "\\r".into(),
        '\t' => "\\t".into(),
        '\\' | ']' | '[' | '^' | '-' | '/' => format!("\\{c}"),
        _ => c.to_string(),
    }
}

fn regex_escape(s: &str) -> String {
    let mut out = String::new();
    for c in s.chars() {
        if "\\.+*?()|[]{}^$/".contains(c) {
            out.push('\\');
        }
        out.push(c);
    }
    out
}

/// The declared conflict set a `carry` needs, pinned beside the module as
/// backend data: tree-sitter's generate is what discovers them (see the
/// `--generate` loop in `examples/lower.rs`), and the file is what makes the
/// lowering reproducible without running it.
pub fn read_conflicts(path: &std::path::Path) -> Result<Option<Vec<Vec<String>>>> {
    if !path.exists() {
        return Ok(None);
    }
    let text = std::fs::read_to_string(path)?;
    let v: Value = serde_json::from_str(&text)?;
    let sets = v["conflicts"]
        .as_array()
        .ok_or_else(|| anyhow!("{}: no `conflicts` array", path.display()))?;
    let mut out = Vec::new();
    for set in sets {
        let names: Vec<String> = set
            .as_array()
            .ok_or_else(|| {
                anyhow!(
                    "{}: a conflict must be an array of rule names",
                    path.display()
                )
            })?
            .iter()
            .filter_map(|n| n.as_str().map(str::to_string))
            .collect();
        out.push(names);
    }
    Ok(Some(out))
}

/// Declare the pinned conflicts in the grammar, and say so: each is a carry.
pub fn apply_conflicts(grammar: &mut Value, conflicts: &[Vec<String>]) -> Vec<Finding> {
    grammar["conflicts"] = json!(conflicts);
    conflicts
        .iter()
        .map(|set| {
            let supertype = set.iter().any(|n| n.starts_with('_'));
            Finding {
                kind: Kind::Mapped,
                what: format!(
                    "declared conflict [{}]: a carry, named by tree-sitter generate and pinned in tree-sitter.conflicts.json{}",
                    set.join(", "),
                    if supertype {
                        "; it names a supertype, the early-commit shape notes/field_guide.md §2 budgets for"
                    } else {
                        ""
                    }
                ),
            }
        })
        .collect()
}

/// tree-sitter's generate names the rules an unresolved conflict is between
/// ("Add a conflict for these rules: `a`, `b`"). Pull every such set out of
/// its stderr.
pub fn conflicts_suggested(stderr: &str) -> Vec<Vec<String>> {
    let mut out = Vec::new();
    for line in stderr.lines() {
        let Some(rest) = line.split("Add a conflict for these rules:").nth(1) else {
            continue;
        };
        let names: Vec<String> = rest
            .split('`')
            .skip(1)
            .step_by(2)
            .map(str::to_string)
            .collect();
        if !names.is_empty() && !out.contains(&names) {
            out.push(names);
        }
    }
    out
}

/// A literal that could be an identifier: a keyword.
fn is_word(s: &str) -> bool {
    s.chars()
        .next()
        .is_some_and(|c| c.is_ascii_alphabetic() || c == '_')
        && s.chars().all(|c| c.is_ascii_alphanumeric() || c == '_')
}

/// The token rule a case-insensitive keyword lowers to.
fn kw_rule(w: &str) -> String {
    format!("_kw_{}", w.to_ascii_lowercase())
}

/// `SELECT` as `[sS][eE][lL][eE][cC][tT]`.
fn ci_regex(w: &str) -> String {
    w.chars()
        .map(|c| {
            if c.is_ascii_alphabetic() {
                format!("[{}{}]", c.to_ascii_lowercase(), c.to_ascii_uppercase())
            } else {
                regex_escape(&c.to_string())
            }
        })
        .collect()
}

pub fn snake(name: &str) -> String {
    let mut out = String::new();
    let chars: Vec<char> = name.chars().collect();
    for (i, c) in chars.iter().enumerate() {
        if c.is_ascii_uppercase() {
            let prev_lower =
                i > 0 && (chars[i - 1].is_ascii_lowercase() || chars[i - 1].is_ascii_digit());
            let next_lower = i + 1 < chars.len() && chars[i + 1].is_ascii_lowercase();
            if i > 0 && (prev_lower || next_lower) && chars[i - 1] != '_' {
                out.push('_');
            }
            out.push(c.to_ascii_lowercase());
        } else {
            out.push(*c);
        }
    }
    out
}

/// Enough of a regex matcher to ask "is this keyword entirely matched by
/// the identifier token's pattern": the patterns lexical sorts produce are
/// character classes with `*`/`+`/`?`, which is what this handles. Anything
/// else is answered conservatively (no match).
type Matcher = Box<dyn Fn(&str) -> bool>;

fn regex_lite_compile(re: &str) -> Option<Matcher> {
    #[derive(Clone)]
    enum Piece {
        Class(Vec<(char, char)>, bool),
        Star(Box<Piece>),
        Plus(Box<Piece>),
        Opt(Box<Piece>),
    }
    fn parse_piece(chars: &[char], i: &mut usize) -> Option<Piece> {
        if *i >= chars.len() {
            return None;
        }
        let base = if chars[*i] == '[' {
            *i += 1;
            let neg = chars.get(*i) == Some(&'^');
            if neg {
                *i += 1;
            }
            let mut ranges = Vec::new();
            while *i < chars.len() && chars[*i] != ']' {
                let mut a = chars[*i];
                if a == '\\' {
                    *i += 1;
                    a = match chars[*i] {
                        'n' => '\n',
                        't' => '\t',
                        'r' => '\r',
                        c => c,
                    };
                }
                *i += 1;
                let mut b = a;
                if chars.get(*i) == Some(&'-') && chars.get(*i + 1) != Some(&']') {
                    *i += 1;
                    b = chars[*i];
                    if b == '\\' {
                        *i += 1;
                        b = chars[*i];
                    }
                    *i += 1;
                }
                ranges.push((a, b));
            }
            *i += 1;
            Piece::Class(ranges, neg)
        } else if chars[*i] == '('
            && chars.get(*i + 1) == Some(&'?')
            && chars.get(*i + 2) == Some(&':')
        {
            *i += 3;
            let inner = parse_piece(chars, i)?;
            if chars.get(*i) != Some(&')') {
                return None;
            }
            *i += 1;
            inner
        } else {
            return None;
        };
        Some(match chars.get(*i) {
            Some('*') => {
                *i += 1;
                Piece::Star(Box::new(base))
            }
            Some('+') => {
                *i += 1;
                Piece::Plus(Box::new(base))
            }
            Some('?') => {
                *i += 1;
                Piece::Opt(Box::new(base))
            }
            _ => base,
        })
    }
    fn matches(p: &Piece, s: &[char], pos: usize) -> Vec<usize> {
        match p {
            Piece::Class(ranges, neg) => {
                if pos < s.len() {
                    let c = s[pos];
                    let inside = ranges.iter().any(|(a, b)| *a <= c && c <= *b);
                    if inside != *neg {
                        return vec![pos + 1];
                    }
                }
                vec![]
            }
            Piece::Opt(inner) => {
                let mut v = vec![pos];
                v.extend(matches(inner, s, pos));
                v
            }
            Piece::Star(inner) | Piece::Plus(inner) => {
                let mut out = if matches!(p, Piece::Star(_)) {
                    vec![pos]
                } else {
                    vec![]
                };
                let mut frontier = vec![pos];
                while let Some(q) = frontier.pop() {
                    for n in matches(inner, s, q) {
                        if n > q && !out.contains(&n) {
                            out.push(n);
                            frontier.push(n);
                        }
                    }
                }
                out
            }
        }
    }
    let chars: Vec<char> = re.chars().collect();
    let mut i = 0;
    let mut pieces = Vec::new();
    while i < chars.len() {
        pieces.push(parse_piece(&chars, &mut i)?);
    }
    Some(Box::new(move |s: &str| {
        let cs: Vec<char> = s.chars().collect();
        let mut positions = vec![0usize];
        for p in &pieces {
            let mut next = Vec::new();
            for pos in positions {
                for n in matches(p, &cs, pos) {
                    if !next.contains(&n) {
                        next.push(n);
                    }
                }
            }
            positions = next;
            if positions.is_empty() {
                return false;
            }
        }
        positions.contains(&cs.len())
    }))
}

/// A readable `grammar.js` rendering of the same grammar, for humans.
pub fn to_grammar_js(grammar: &Value) -> String {
    fn expr(v: &Value, indent: usize) -> String {
        let pad = "  ".repeat(indent);
        let ty = v["type"].as_str().unwrap_or("");
        match ty {
            "STRING" => format!("{:?}", v["value"].as_str().unwrap_or("")),
            "PATTERN" => format!("/{}/", v["value"].as_str().unwrap_or("")),
            "SYMBOL" => format!("$.{}", v["name"].as_str().unwrap_or("")),
            "BLANK" => "blank()".into(),
            "SEQ" | "CHOICE" => {
                let f = if ty == "SEQ" { "seq" } else { "choice" };
                let members = v["members"].as_array().cloned().unwrap_or_default();
                if members.len() == 2 && ty == "CHOICE" && members[1]["type"] == "BLANK" {
                    return format!("optional({})", expr(&members[0], indent));
                }
                let inner: Vec<String> = members
                    .iter()
                    .map(|m| format!("{pad}  {}", expr(m, indent + 1)))
                    .collect();
                format!("{f}(\n{}\n{pad})", inner.join(",\n"))
            }
            "REPEAT" => format!("repeat({})", expr(&v["content"], indent)),
            "REPEAT1" => format!("repeat1({})", expr(&v["content"], indent)),
            "FIELD" => format!(
                "field({:?}, {})",
                v["name"].as_str().unwrap_or(""),
                expr(&v["content"], indent)
            ),
            "PREC" | "PREC_LEFT" | "PREC_RIGHT" | "PREC_DYNAMIC" => {
                let f = match ty {
                    "PREC" => "prec",
                    "PREC_LEFT" => "prec.left",
                    "PREC_RIGHT" => "prec.right",
                    _ => "prec.dynamic",
                };
                format!("{f}({}, {})", v["value"], expr(&v["content"], indent))
            }
            "TOKEN" => format!("token({})", expr(&v["content"], indent)),
            "IMMEDIATE_TOKEN" => format!("token.immediate({})", expr(&v["content"], indent)),
            "ALIAS" => {
                let target = if v["named"].as_bool().unwrap_or(false) {
                    format!("$.{}", v["value"].as_str().unwrap_or(""))
                } else {
                    format!("{:?}", v["value"].as_str().unwrap_or(""))
                };
                format!("alias({}, {target})", expr(&v["content"], indent))
            }
            other => format!("/* {other} */"),
        }
    }
    let mut out = String::new();
    out.push_str("// GENERATED from an SDF3 module by treebank-sdf3; grammar.json is the\n// artifact the parser is generated from, this file is for reading.\n\n");
    out.push_str(&format!(
        "module.exports = grammar({{\n  name: {:?},\n",
        grammar["name"].as_str().unwrap_or("")
    ));
    if let Some(w) = grammar["word"].as_str() {
        out.push_str(&format!("  word: $ => $.{w},\n"));
    }
    if let Some(extras) = grammar["extras"].as_array() {
        let e: Vec<String> = extras.iter().map(|x| expr(x, 2)).collect();
        out.push_str(&format!("  extras: $ => [{}],\n", e.join(", ")));
    }
    if let Some(s) = grammar["supertypes"].as_array() {
        let e: Vec<String> = s
            .iter()
            .map(|x| format!("$.{}", x.as_str().unwrap_or("")))
            .collect();
        out.push_str(&format!("  supertypes: $ => [{}],\n", e.join(", ")));
    }
    if let Some(ext) = grammar["externals"].as_array() {
        if !ext.is_empty() {
            let e: Vec<String> = ext
                .iter()
                .map(|x| format!("$.{}", x["name"].as_str().unwrap_or("")))
                .collect();
            out.push_str(&format!("  externals: $ => [{}],\n", e.join(", ")));
        }
    }
    if let Some(inl) = grammar["inline"].as_array() {
        if !inl.is_empty() {
            let e: Vec<String> = inl
                .iter()
                .map(|x| format!("$.{}", x.as_str().unwrap_or("")))
                .collect();
            out.push_str(&format!("  inline: $ => [{}],\n", e.join(", ")));
        }
    }
    if let Some(conf) = grammar["conflicts"].as_array() {
        if !conf.is_empty() {
            let sets: Vec<String> = conf
                .iter()
                .map(|c| {
                    let names: Vec<String> = c
                        .as_array()
                        .into_iter()
                        .flatten()
                        .map(|n| format!("$.{}", n.as_str().unwrap_or("")))
                        .collect();
                    format!("[{}]", names.join(", "))
                })
                .collect();
            out.push_str(&format!("  conflicts: $ => [{}],\n", sets.join(", ")));
        }
    }
    if let Some(words) = grammar["reserved"]["global"].as_array() {
        let e: Vec<String> = words
            .iter()
            .map(|x| match x["type"].as_str() {
                Some("SYMBOL") => format!("$.{}", x["name"].as_str().unwrap_or("")),
                _ => format!("{:?}", x["value"].as_str().unwrap_or("")),
            })
            .collect();
        out.push_str(&format!(
            "  reserved: {{ global: $ => [{}] }},\n",
            e.join(", ")
        ));
    }
    out.push_str("  rules: {\n");
    if let Some(rules) = grammar["rules"].as_object() {
        for (name, body) in rules {
            out.push_str(&format!("    {name}: $ => {},\n\n", expr(body, 2)));
        }
    }
    out.push_str("  },\n});\n");
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn snake_case_of_sort_and_constructor_names() {
        assert_eq!(snake("Exp"), "exp");
        assert_eq!(snake("ExprStmt"), "expr_stmt");
        assert_eq!(snake("ID"), "id");
        assert_eq!(snake("INT"), "int");
        assert_eq!(snake("Program"), "program");
    }

    #[test]
    fn keyword_matcher_answers_for_identifier_patterns() {
        let m = regex_lite_compile("[a-zA-Z_][a-zA-Z0-9_]*").unwrap();
        assert!(m("let"));
        assert!(m("while"));
        assert!(!m("=="));
        assert!(!m("("));
        let m = regex_lite_compile("[0-9]+").unwrap();
        assert!(m("42"));
        assert!(!m("x"));
    }
}
