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
pub mod bindings;
pub mod ast;
pub mod lower;
pub mod parse;
pub mod print;
pub mod scanner;
pub mod term;
pub mod vocab;

pub use lower::{
    apply_conflicts, conflicts_suggested, lower, read_conflicts, to_grammar_js, Finding, Kind,
    Lowered,
};
pub use parse::parse_module;

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

/// Read a module and everything it imports, as SDF3 composition does:
/// imported sections come first, additively. `imports cish` resolves to
/// `cish.sdf3` beside the importing file. Nothing is overridden -- a sort
/// gains productions from every module that declares any.
pub fn load_module(path: &Path) -> anyhow::Result<ast::Module> {
    let mut visited = BTreeSet::new();
    visited.insert(path.to_path_buf());
    load_into(path, &mut visited)
}

fn load_into(path: &Path, visited: &mut BTreeSet<PathBuf>) -> anyhow::Result<ast::Module> {
    let text = std::fs::read_to_string(path)
        .map_err(|e| anyhow::anyhow!("reading {}: {e}", path.display()))?;
    let mut module = parse_module(&text).map_err(|e| anyhow::anyhow!("{}: {e}", path.display()))?;
    let dir = path.parent().unwrap_or_else(|| Path::new("."));
    let mut sections = Vec::new();
    for name in &module.imports {
        let sub = dir.join(format!("{name}.sdf3"));
        if !visited.insert(sub.clone()) {
            continue;
        }
        let imported = load_into(&sub, visited)?;
        sections.extend(imported.sections);
    }
    sections.append(&mut module.sections);
    module.sections = sections;
    Ok(module)
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
                    derived.entry("_callable".into()).or_default().insert(n.to_string());
                }
            }
        }
    }
    // LAYOUT productions that became named extras are comments.
    if let Some(extras) = lowered.grammar["extras"].as_array() {
        for e in extras {
            if let Some(n) = e["name"].as_str() {
                derived.entry("_comment".into()).or_default().insert(n.to_string());
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

