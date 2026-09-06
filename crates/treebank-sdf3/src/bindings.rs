//! Bindings next to the rules that create them, lowered to data.
//!
//! SDF3 has no binding attributes; Spoofax puts name binding in a separate
//! language (NaBL2, then Statix). The design note (§5) wants them beside
//! the syntax, so three attributes are a treebank extension on productions:
//! `scope(kind)`, `binds(field -> enclosing | module as kind)`, and
//! `refers(position | field)`. This module lowers them to two outputs:
//!
//! - `bindings.json`: the exact data -- which node types delimit scopes,
//!   which (node, field) pairs bind names and into which scope, which
//!   node is a reference -- plus the `_binding` and `_scope` facet
//!   memberships treebank's `roles.json` would carry.
//! - `queries/locals.scm`: the same in treebank's locals vocabulary, for
//!   any consumer of tree-sitter queries, with a finding wherever the
//!   query dialect cannot say what the data says.
//!
//! `tools/bindings_check.py` holds `bindings.json` to CPython's `symtable`.

use std::collections::{BTreeMap, BTreeSet};

use anyhow::{bail, Result};
use serde_json::{json, Value};

use crate::ast::*;
use crate::lower::{Finding, Kind, Names};

pub struct Emitted {
    pub json: Value,
    pub locals: String,
    pub findings: Vec<Finding>,
}

/// None when the module has no binding attributes at all.
pub fn emit(module: &Module, names: &Names) -> Result<Option<Emitted>> {
    let mut findings = Vec::new();
    let mut scopes: Vec<Value> = Vec::new();
    let mut definitions: Vec<Value> = Vec::new();
    let mut references: Vec<Value> = Vec::new();
    let mut facet_binding: BTreeSet<String> = BTreeSet::new();
    let mut facet_scope: BTreeSet<String> = BTreeSet::new();
    let mut scm_scopes: Vec<String> = Vec::new();
    let mut scm_defs: Vec<String> = Vec::new();
    let mut scm_refs: Vec<String> = Vec::new();
    let mut any = false;
    let mut whole_noted = false;

    for p in module.productions(false) {
        let node = p.reference().and_then(|r| names.node.get(&r).cloned());
        for a in &p.attrs {
            match a {
                Attr::Scope(kind) => {
                    any = true;
                    let Some(node) = &node else {
                        bail!(
                            "{}: `scope` on a production with no node of its own",
                            p.display()
                        );
                    };
                    let kind = kind.clone().unwrap_or_else(|| "block".into());
                    scopes.push(json!({"node": node, "kind": kind, "from": p.display()}));
                    facet_scope.insert(node.clone());
                    scm_scopes.push(format!("({node})"));
                    findings.push(Finding {
                        kind: Kind::Extension,
                        what: format!(
                            "{}: `scope({kind})` (not SDF3): `{node}` delimits a lexical scope",
                            p.display()
                        ),
                    });
                }
                Attr::Binds(b) => {
                    any = true;
                    let Some(node) = &node else {
                        bail!(
                            "{}: `binds` on a production with no node of its own",
                            p.display()
                        );
                    };
                    let Some(sym) = labelled(p, &b.label) else {
                        bail!(
                            "{}: `binds({} -> ..)` names no placeholder label of the production",
                            p.display(),
                            b.label
                        );
                    };
                    let Some(token) = name_token(sym, names) else {
                        findings.push(Finding {
                            kind: Kind::Unsupported,
                            what: format!(
                                "{}: `binds({} -> ..)`: the field holds {sym:?}, not a name token; put the binding on the production whose field is the name",
                                p.display(),
                                b.label
                            ),
                        });
                        continue;
                    };
                    let kind = b.kind.clone().unwrap_or_else(|| "var".into());
                    let target = match &b.target {
                        BindTarget::Enclosing => "enclosing".to_string(),
                        BindTarget::Kind(k) => k.clone(),
                    };
                    let effect = match b.effect {
                        BindEffect::Whole => "whole",
                        BindEffect::After => "after",
                    };
                    definitions.push(json!({
                        "node": node, "field": b.label, "name": token,
                        "scope": target, "kind": kind, "effect": effect, "from": p.display()
                    }));
                    facet_binding.insert(node.clone());
                    let is_scope = p.attrs.iter().any(|a| matches!(a, Attr::Scope(_)));
                    let mut pat =
                        format!("({node} {}: ({token}) @local.definition.{kind}", b.label);
                    if b.effect == BindEffect::Whole && !is_scope && !whole_noted {
                        whole_noted = true;
                        findings.push(Finding {
                            kind: Kind::Deviation,
                            what: "a whole-scope binding (`effect: whole`) is visible before its definition; tree-sitter's locals engine resolves a reference to the nearest definition that precedes it, so a use before the definition resolves outward there. `after` bindings match the engine exactly".into(),
                        });
                    }
                    if let BindTarget::Kind(k) = &b.target {
                        findings.push(Finding {
                            kind: Kind::Deviation,
                            what: format!(
                                "{}: `binds({} -> {k})`: the locals query dialect cannot name a scope by kind, so the pattern binds at the nearest scope; bindings.json carries the target",
                                p.display(),
                                b.label
                            ),
                        });
                        scm_defs.push(format!("; bindings.json: scope = {k}. The dialect has no way to say it.\n{pat})"));
                    } else if is_scope {
                        pat.push_str(&format!("\n  (#set! definition.{kind}.scope \"parent\")"));
                        findings.push(Finding {
                            kind: Kind::Deviation,
                            what: format!(
                                "{}: `binds({} -> enclosing)` on a scope node: tree-sitter's locals engine files a definition under the innermost scope containing it, which is this node; the query carries nvim-treesitter's `#set! ..scope \"parent\"`, which tree-sitter's own highlighter ignores",
                                p.display(),
                                b.label
                            ),
                        });
                        scm_defs.push(format!("{pat})"));
                    } else {
                        scm_defs.push(format!("{pat})"));
                    }
                    findings.push(Finding {
                        kind: Kind::Extension,
                        what: format!(
                            "{}: `binds({} -> {target} as {kind} {effect})` (not SDF3): the `{token}` under `{}` of `{node}` is bound in the {target} scope, {}",
                            p.display(),
                            b.label,
                            b.label,
                            if b.effect == BindEffect::After { "from the end of the node onward" } else { "for the whole scope" }
                        ),
                    });
                }
                Attr::Refers(r) => {
                    any = true;
                    let sym = match r.parse::<usize>() {
                        Ok(n) => match p.symbols().get(n.wrapping_sub(1)) {
                            Some(SymRef::Sym(s)) => Some(*s),
                            _ => None,
                        },
                        Err(_) => labelled(p, r),
                    };
                    let Some(sym) = sym else {
                        bail!(
                            "{}: `refers({r})` names no symbol of the production",
                            p.display()
                        );
                    };
                    let Some(token) = name_token(sym, names) else {
                        findings.push(Finding {
                            kind: Kind::Unsupported,
                            what: format!(
                                "{}: `refers({r})` on {sym:?}, not a name token; ignored",
                                p.display()
                            ),
                        });
                        continue;
                    };
                    references.push(json!({"node": token, "from": p.display()}));
                    scm_refs.push(format!("({token}) @local.reference"));
                    findings.push(Finding {
                        kind: Kind::Extension,
                        what: format!(
                            "{}: `refers({r})` (not SDF3): every `{token}` not claimed by a definition is a reference{}",
                            p.display(),
                            if node.is_none() { "; the production is an injection, so the reference is the token itself" } else { "" }
                        ),
                    });
                }
                _ => {}
            }
        }
    }
    if !any {
        return Ok(None);
    }
    findings.push(Finding {
        kind: Kind::Mapped,
        what: format!(
            "facets from the attributes: _scope = [{}], _binding = [{}]",
            facet_scope.iter().cloned().collect::<Vec<_>>().join(", "),
            facet_binding.iter().cloned().collect::<Vec<_>>().join(", ")
        ),
    });

    let json = json!({
        "note": "GENERATED by treebank-sdf3 from the scope/binds/refers attributes of the module. A definition's `scope` is `enclosing` (the nearest scope node that is a proper ancestor of the binding node) or a scope kind (the nearest enclosing scope of that kind: `module`, `function`). Its `effect` is `whole` (visible throughout the scope; several such bindings of one name in one scope are one slot) or `after` (from the end of the binding node onward; each is a new slot). A reference resolves to the slot of its name with the latest start at or before it, in the nearest scope that has one, outward. A scope's kind-directed binding of a name redirects that scope's other bindings of it. tools/bindings_check.py holds this to CPython's symtable; tools/resolve_check.py holds it to what the real toolchain prints.",
        "scopes": scopes,
        "definitions": definitions,
        "references": references,
        "facets": {"_scope": facet_scope, "_binding": facet_binding},
    });

    let mut locals = String::new();
    locals.push_str(&format!(
        "; GENERATED by treebank-sdf3 from the scope/binds/refers attributes of\n; {}.sdf3, in treebank's locals vocabulary. bindings.json is the exact\n; data; this is the query-dialect view of it.\n\n",
        module.name
    ));
    locals.push_str("; --- where names live\n");
    locals.push_str(&format!("[{}] @local.scope\n\n", scm_scopes.join(" ")));
    locals.push_str("; --- what introduces a name\n");
    for d in &scm_defs {
        locals.push_str(d);
        locals.push_str("\n\n");
    }
    locals.push_str("; --- what mentions a name\n");
    for r in &scm_refs {
        locals.push_str(r);
        locals.push('\n');
    }
    Ok(Some(Emitted {
        json,
        locals,
        findings,
    }))
}

/// The symbol under a placeholder label.
fn labelled<'a>(p: &'a Production, label: &str) -> Option<&'a Symbol> {
    let Rhs::Template(parts) = &p.rhs else {
        return None;
    };
    parts.iter().find_map(|part| match part {
        TemplatePart::Placeholder {
            label: Some(l),
            symbol,
        } if l == label => Some(symbol),
        _ => None,
    })
}

/// The lexical sort's token name, through lists and options, or None when
/// the symbol is not a name token.
fn name_token(sym: &Symbol, names: &Names) -> Option<String> {
    match sym {
        Symbol::Sort(s) if names.lexical.contains(s) => names.sort_rule.get(s).cloned(),
        Symbol::Star(i) | Symbol::Plus(i) | Symbol::Opt(i) => name_token(i, names),
        Symbol::SepList { elem, .. } => name_token(elem, names),
        _ => None,
    }
}

#[allow(dead_code)]
fn _unused(_: &BTreeMap<String, String>) {}
