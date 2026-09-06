//! The committed `spike/jsish/` output is what the reader and lowerings
//! produce from `jsish.sdf3`, byte for byte.

use std::path::Path;

fn spike() -> &'static Path {
    Path::new(concat!(env!("CARGO_MANIFEST_DIR"), "/spike/jsish"))
}

fn same(name: &str, produced: &str) {
    let committed = std::fs::read_to_string(spike().join(name)).unwrap();
    assert_eq!(
        produced, committed,
        "spike/jsish/{name} is stale; regenerate it"
    );
}

#[test]
fn jsish_reads_and_lowers_to_the_committed_output() {
    let module = treebank_sdf3::load_module(&spike().join("jsish.sdf3")).unwrap();
    let lowered = treebank_sdf3::lower_all(&module).unwrap().lowered;
    let mut grammar = lowered.grammar.clone();
    let mut findings = lowered.findings.clone();
    let conflicts = treebank_sdf3::read_conflicts(&spike().join("tree-sitter.conflicts.json"))
        .unwrap()
        .unwrap_or_default();
    findings.extend(treebank_sdf3::apply_conflicts(&mut grammar, &conflicts));
    same(
        "grammar.json",
        &(serde_json::to_string_pretty(&grammar).unwrap() + "\n"),
    );
    same("findings.md", &treebank_sdf3::report(&findings));
    let b = treebank_sdf3::bindings::emit(&module, &lowered.names)
        .unwrap()
        .unwrap();
    same(
        "bindings.json",
        &(serde_json::to_string_pretty(&b.json).unwrap() + "\n"),
    );
    same("queries/locals.scm", &b.locals);
    same("bindings-findings.md", &treebank_sdf3::report(&b.findings));
    let antlr = treebank_sdf3::antlr::emit(&module, &lowered.names, &lowered.levels).unwrap();
    same("Jsish.g4", &antlr.grammar);
}

#[test]
fn var_binds_in_the_function_and_let_in_the_block() {
    let module = treebank_sdf3::load_module(&spike().join("jsish.sdf3")).unwrap();
    let lowered = treebank_sdf3::lower_all(&module).unwrap().lowered;
    let b = treebank_sdf3::bindings::emit(&module, &lowered.names)
        .unwrap()
        .unwrap();
    let defs = b.json["definitions"].as_array().unwrap();
    let scope = |node: &str| {
        defs.iter()
            .find(|d| d["node"] == node)
            .map(|d| d["scope"].as_str().unwrap().to_string())
            .unwrap()
    };
    assert_eq!(scope("var"), "function");
    assert_eq!(scope("let"), "enclosing");
    assert!(b
        .findings
        .iter()
        .any(|f| f.what.contains("cannot name a scope by kind")));
}
