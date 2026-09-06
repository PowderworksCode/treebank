//! An SDF3 reader and a lowering to tree-sitter: the spike behind
//! `notes/metagrammar.md` §11's recommendation to adopt SDF3 as the
//! meta-grammar's surface.
//!
//! The question it answers is narrow and load-bearing: take a language
//! written in SDF3 as Spoofax documents it, read it with a Rust parser,
//! lower it to a tree-sitter `grammar.json`, generate, and see whether the
//! trees tree-sitter builds are the trees the SDF3 semantics say they
//! should be -- priorities nesting the right way, injections producing no
//! node, brackets producing no node, keywords reserved. Where the lowering
//! cannot be exact it says so in a [`lower::Finding`], because a silent
//! approximation would be the one result this spike must not produce.
//!
//! `spike/mini/` holds the language, the generated grammar, and the
//! expectations; `tests/mini.rs` holds the lowering to the committed output.

pub mod antlr;
pub mod ast;
pub mod bindings;
pub mod lower;
pub mod nfa;
pub mod parse;
pub mod print;
pub mod scanner;
pub mod term;
pub mod vocab;
pub mod winnow;

pub use lower::{
    apply_conflicts, conflicts_suggested, lower, read_conflicts, to_grammar_js, Finding, Kind,
    Lowered,
};
pub use parse::parse_module;

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

/// Read a module and everything it imports, as SDF3 composition does:
/// imported sections come first, additively. `imports sql/core` resolves
/// to `sql/core.sdf3` under the module's root, which is its path with its
/// own name stripped: `postgres/15.sdf3` named `postgres/15` has the root
/// `postgres/..`, so a family's modules refer to each other by the same
/// names from anywhere in the tree. Nothing is overridden -- a sort gains
/// productions from every module that declares any -- except what a
/// module's `hiding` clause subtracts (see [`hide`]); then the sorts left
/// without productions are closed as holes (see [`close_holes`]).
pub fn load_module(path: &Path) -> anyhow::Result<ast::Module> {
    let mut visited = BTreeSet::new();
    visited.insert(canonical(path));
    let mut module = load_into(path, &mut visited)?;
    close_holes(&mut module);
    Ok(module)
}

fn canonical(path: &Path) -> PathBuf {
    path.canonicalize().unwrap_or_else(|_| path.to_path_buf())
}

/// The directory a module's imports resolve against.
fn root_of(path: &Path, name: &str) -> PathBuf {
    let text = path.to_string_lossy();
    let suffix = format!("{name}.sdf3");
    match text.strip_suffix(suffix.as_str()) {
        Some("") => PathBuf::from("."),
        Some(prefix) => PathBuf::from(prefix),
        None => path
            .parent()
            .unwrap_or_else(|| Path::new("."))
            .to_path_buf(),
    }
}

fn load_into(path: &Path, visited: &mut BTreeSet<PathBuf>) -> anyhow::Result<ast::Module> {
    let text = std::fs::read_to_string(path)
        .map_err(|e| anyhow::anyhow!("reading {}: {e}", path.display()))?;
    let mut module = parse_module(&text).map_err(|e| anyhow::anyhow!("{}: {e}", path.display()))?;
    let root = root_of(path, &module.name);
    let mut sections = Vec::new();
    for name in &module.imports {
        let sub = root.join(format!("{name}.sdf3"));
        if !visited.insert(canonical(&sub)) {
            continue;
        }
        let imported = load_into(&sub, visited)?;
        sections.extend(imported.sections);
    }
    if !module.hiding.is_empty() {
        hide(&root, &module.hiding, &mut sections)
            .map_err(|e| anyhow::anyhow!("{}: hiding: {e}", path.display()))?;
    }
    sections.append(&mut module.sections);
    module.sections = sections;
    Ok(module)
}

/// `hiding` (a treebank extension): subtract from what the imports composed.
/// A name with a `/` or without a `.` is a module, and every production
/// that module declares itself (not what it imports) is removed; a
/// `Sort.Cons` reference removes that production. Priorities and
/// vocabulary lines that named a removed production lose that member.
/// A name that removes nothing is an error, so a hiding clause cannot
/// go stale silently.
fn hide(root: &Path, hiding: &[String], sections: &mut [ast::Section]) -> anyhow::Result<()> {
    for name in hiding {
        let is_module = name.contains('/') || !name.contains('.');
        let (prods, refs): (Vec<ast::Production>, BTreeSet<String>) = if is_module {
            let sub = root.join(format!("{name}.sdf3"));
            let text = std::fs::read_to_string(&sub)
                .map_err(|e| anyhow::anyhow!("reading {}: {e}", sub.display()))?;
            let m = parse_module(&text).map_err(|e| anyhow::anyhow!("{}: {e}", sub.display()))?;
            let prods: Vec<ast::Production> = m
                .productions(true)
                .chain(m.productions(false))
                .cloned()
                .collect();
            let refs = prods.iter().filter_map(|p| p.reference()).collect();
            (prods, refs)
        } else {
            (Vec::new(), BTreeSet::from([name.clone()]))
        };
        let removed = prune(sections, |p| {
            prods.contains(p) || p.reference().is_some_and(|r| refs.contains(&r))
        });
        if removed.is_empty() {
            anyhow::bail!("`{name}` hides nothing: no production of the imports matches it");
        }
    }
    Ok(())
}

/// Remove the productions `gone` selects, and every priority member and
/// vocabulary member that named one of them. Returns what was removed.
fn prune(
    sections: &mut [ast::Section],
    gone: impl Fn(&ast::Production) -> bool,
) -> Vec<ast::Production> {
    use ast::Section::*;
    let mut removed = Vec::new();
    for sec in sections.iter_mut() {
        if let LexicalSyntax(ps) | ContextFreeSyntax(ps) = sec {
            let (out, keep): (Vec<_>, Vec<_>) = ps.drain(..).partition(|p| gone(p));
            *ps = keep;
            removed.extend(out);
        }
    }
    let refs: BTreeSet<String> = removed.iter().filter_map(|p| p.reference()).collect();
    for sec in sections.iter_mut() {
        match sec {
            ContextFreePriorities(chains) => {
                for c in chains.iter_mut() {
                    for g in &mut c.groups {
                        g.members.retain(|m| !refs.contains(m));
                        g.prods.retain(|q| !removed.iter().any(|r| r.same_as(q)));
                    }
                    c.groups.retain(|g| !g.members.is_empty() || !g.prods.is_empty());
                }
                chains.retain(|c| !c.groups.is_empty());
            }
            Vocabulary(terms) => {
                for t in terms.iter_mut() {
                    t.members.retain(|m| !refs.contains(m));
                }
                terms.retain(|t| !t.members.is_empty());
            }
            _ => {}
        }
    }
    removed
}

/// Close the holes: a sort that is declared (or was emptied by hiding or
/// by an earlier closure) but has no production in this composition is a
/// dialect point the target does not fill. SDF3 says such a sort matches
/// nothing, and the composition is rewritten to say the same thing
/// directly, so every backend sees an ordinary module: an optional or
/// starred occurrence of the hole is removed from its production, and a
/// production that needs the hole is dropped, with the vocabulary and
/// priority lines that named it. Runs to a fixpoint, since a dropped
/// production can empty another sort. An undeclared sort that nothing
/// defines is not a hole; it stays an error in the lowering.
fn close_holes(module: &mut ast::Module) {
    use ast::{Rhs, Section, Symbol, TemplatePart};

    fn mentions(s: &Symbol, hole: &str) -> bool {
        match s {
            Symbol::Sort(n) => n == hole,
            Symbol::Lit(_) | Symbol::CharClass(_) => false,
            Symbol::Star(i) | Symbol::Plus(i) | Symbol::Opt(i) => mentions(i, hole),
            Symbol::SepList { elem, sep, .. } => mentions(elem, hole) || mentions(sep, hole),
            Symbol::Group(alts) => alts.iter().flatten().any(|s| mentions(s, hole)),
        }
    }
    /// Whether an occurrence of the hole in `s` may simply be removed.
    fn removable(s: &Symbol) -> bool {
        matches!(
            s,
            Symbol::Star(_) | Symbol::Opt(_) | Symbol::SepList { min: 0, .. }
        )
    }

    let mut known: BTreeSet<String> = module
        .declared_sorts()
        .iter()
        .map(|s| s.to_string())
        .collect();
    known.extend(module.productions(false).map(|p| p.sort.clone()));
    loop {
        let defined: BTreeSet<String> = module
            .productions(false)
            .chain(module.productions(true))
            .map(|p| p.sort.clone())
            .collect();
        let lexical: BTreeSet<String> = module
            .sections
            .iter()
            .flat_map(|s| match s {
                Section::LexicalSorts(v) => v.clone(),
                _ => Vec::new(),
            })
            .collect();
        let already: BTreeSet<&str> = module.holes.iter().map(|h| h.sort.as_str()).collect();
        let Some(hole) = known
            .iter()
            .find(|s| {
                !defined.contains(*s) && !lexical.contains(*s) && !already.contains(s.as_str())
            })
            .cloned()
        else {
            break;
        };
        let mut blanked = Vec::new();
        let mut dropped = Vec::new();
        for sec in module.sections.iter_mut() {
            let Section::ContextFreeSyntax(ps) = sec else {
                continue;
            };
            let mut keep = Vec::new();
            for mut p in ps.drain(..) {
                let mut needs = false;
                let mut touched = false;
                match &mut p.rhs {
                    Rhs::Template(parts) => {
                        let mut out: Vec<TemplatePart> = Vec::new();
                        for part in parts.drain(..) {
                            match &part {
                                TemplatePart::Placeholder { symbol, .. }
                                    if mentions(symbol, &hole) =>
                                {
                                    if removable(symbol) {
                                        touched = true;
                                        // Take the layout before it too, so the
                                        // template's whitespace stays single.
                                        if matches!(out.last(), Some(TemplatePart::Layout(_)))
                                            && out.len() > 1
                                        {
                                            out.pop();
                                        }
                                    } else {
                                        needs = true;
                                    }
                                }
                                _ => out.push(part),
                            }
                        }
                        *parts = out;
                    }
                    Rhs::Symbols(syms) => {
                        let mut out = Vec::new();
                        for s in syms.drain(..) {
                            if mentions(&s, &hole) {
                                if removable(&s) {
                                    touched = true;
                                } else {
                                    needs = true;
                                }
                            } else {
                                out.push(s);
                            }
                        }
                        *syms = out;
                    }
                }
                if needs {
                    dropped.push(p.reference().unwrap_or_else(|| p.sort.clone()));
                } else {
                    if touched {
                        blanked.push(p.reference().unwrap_or_else(|| p.sort.clone()));
                    }
                    keep.push(p);
                }
            }
            *ps = keep;
        }
        let gone: BTreeSet<String> = dropped.iter().cloned().collect();
        prune(&mut module.sections, |p| {
            gone.contains(&p.reference().unwrap_or_else(|| p.sort.clone()))
        });
        for sec in module.sections.iter_mut() {
            if let Section::Vocabulary(terms) = sec {
                for t in terms.iter_mut() {
                    t.members.retain(|m| *m != hole);
                }
                terms.retain(|t| !t.members.is_empty());
            }
        }
        module.holes.push(ast::Hole {
            sort: hole,
            blanked,
            dropped,
        });
    }
}

/// Findings as a report, grouped by kind, stable across runs so the file
/// can be committed and diffed.
pub fn report(findings: &[Finding]) -> String {
    let mut out = String::new();
    for kind in [
        Kind::Unsupported,
        Kind::Widening,
        Kind::Deviation,
        Kind::Extension,
        Kind::Absorbed,
        Kind::Mapped,
    ] {
        let items: Vec<&Finding> = findings.iter().filter(|f| f.kind == kind).collect();
        if items.is_empty() {
            continue;
        }
        let title = match kind {
            Kind::Unsupported => "UNSUPPORTED -- the grammar is missing something",
            Kind::Widening => "WIDENING -- tree-sitter accepts more than SDF3 here",
            Kind::Deviation => "DEVIATION -- the tree differs in shape from SDF3's AST",
            Kind::Extension => "EXTENSION -- a treebank addition outside SDF3 was used",
            Kind::Absorbed => {
                "ABSORBED -- nothing emitted, tree-sitter gets the effect another way"
            }
            Kind::Mapped => "MAPPED -- lowered exactly",
        };
        out.push_str(&format!("## {title} ({})\n\n", items.len()));
        let mut lines: Vec<String> = items.iter().map(|f| format!("- {}", f.what)).collect();
        lines.dedup();
        out.push_str(&lines.join("\n"));
        out.push_str("\n\n");
    }
    out
}

/// Everything the module lowers to, in the order the pieces need: the
/// grammar, the bindings (which name nodes), then the vocabulary (which
/// renames and threads supertypes, and takes the facets bindings derived).
pub struct Everything {
    pub lowered: lower::Lowered,
    pub bindings: Option<bindings::Emitted>,
    pub vocab: Option<vocab::Emitted>,
}

pub fn lower_all(module: &ast::Module) -> anyhow::Result<Everything> {
    let mut lowered = lower(module)?;
    let bindings = bindings::emit(module, &lowered.names)?;
    let mut derived: std::collections::BTreeMap<String, std::collections::BTreeSet<String>> =
        Default::default();
    if let Some(b) = &bindings {
        for (facet, members) in b.json["facets"].as_object().into_iter().flatten() {
            let set: std::collections::BTreeSet<String> = members
                .as_array()
                .into_iter()
                .flatten()
                .filter_map(|v| v.as_str().map(String::from))
                .collect();
            derived.insert(facet.clone(), set);
        }
        // A binding of kind `function` is the node that defines a callable.
        for d in b.json["definitions"].as_array().into_iter().flatten() {
            if d["kind"].as_str() == Some("function") {
                if let Some(n) = d["node"].as_str() {
                    derived
                        .entry("_callable".into())
                        .or_default()
                        .insert(n.to_string());
                }
            }
        }
    }
    // LAYOUT productions that became named extras are comments.
    if let Some(extras) = lowered.grammar["extras"].as_array() {
        for e in extras {
            if let Some(n) = e["name"].as_str() {
                derived
                    .entry("_comment".into())
                    .or_default()
                    .insert(n.to_string());
            }
        }
    }
    // Only a module that binds terms gets a manifest: derived facets alone
    // would ledger every node of mini as uncategorised, which says nothing.
    let vocab = if module.vocabulary().next().is_some() {
        vocab::apply(module, &mut lowered.grammar, &mut lowered.names, &derived)?
    } else {
        None
    };
    if let Some(v) = &vocab {
        lowered.findings.extend(v.findings.iter().cloned());
    }
    Ok(Everything {
        lowered,
        bindings,
        vocab,
    })
}
