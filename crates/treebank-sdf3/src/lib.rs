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

pub mod ast;
pub mod lower;
pub mod parse;
pub mod scanner;

pub use lower::{lower, to_grammar_js, Finding, Kind, Lowered};
pub use parse::parse_module;

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
