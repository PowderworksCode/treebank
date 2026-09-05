//! The committed `spike/rustish/` output is what the reader and lowerings
//! produce from `rustish.sdf3`, byte for byte.

use std::path::Path;

fn spike() -> &'static Path {
    Path::new(concat!(env!("CARGO_MANIFEST_DIR"), "/spike/rustish"))
}

fn same(name: &str, produced: &str) {
    let committed = std::fs::read_to_string(spike().join(name)).unwrap();
    assert_eq!(produced, committed, "spike/rustish/{name} is stale; regenerate it");
}

#[test]
fn rustish_reads_and_lowers_to_the_committed_output() {
    let module = treebank_sdf3::load_module(&spike().join("rustish.sdf3")).unwrap();
    let lowered = treebank_sdf3::lower_all(&module).unwrap().lowered;
    let mut grammar = lowered.grammar.clone();
    let mut findings = lowered.findings.clone();
    let conflicts = treebank_sdf3::read_conflicts(&spike().join("tree-sitter.conflicts.json"))
        .unwrap()
        .unwrap_or_default();
    findings.extend(treebank_sdf3::apply_conflicts(&mut grammar, &conflicts));
    same("grammar.json", &(serde_json::to_string_pretty(&grammar).unwrap() + "\n"));
    same("findings.md", &treebank_sdf3::report(&findings));
    let b = treebank_sdf3::bindings::emit(&module, &lowered.names).unwrap().unwrap();
    same("bindings.json", &(serde_json::to_string_pretty(&b.json).unwrap() + "\n"));
    same("queries/locals.scm", &b.locals);
    same("bindings-findings.md", &treebank_sdf3::report(&b.findings));
    let antlr = treebank_sdf3::antlr::emit(&module, &lowered.names, &lowered.levels).unwrap();
    same("Rustish.g4", &antlr.grammar);
}

#[test]
fn a_let_binds_after_its_node_and_a_fn_item_binds_the_whole_scope() {
    let module = treebank_sdf3::load_module(&spike().join("rustish.sdf3")).unwrap();
    let lowered = treebank_sdf3::lower_all(&module).unwrap().lowered;
    let b = treebank_sdf3::bindings::emit(&module, &lowered.names).unwrap().unwrap();
    let defs = b.json["definitions"].as_array().unwrap();
    let effect = |node: &str| {
        defs.iter()
            .find(|d| d["node"] == node)
            .map(|d| d["effect"].as_str().unwrap().to_string())
            .unwrap()
    };
    assert_eq!(effect("let"), "after");
    assert_eq!(effect("fn"), "whole");
    assert_eq!(effect("param"), "whole");
    assert_eq!(b.json["facets"]["_scope"], serde_json::json!(["block", "fn", "program"]));
}
