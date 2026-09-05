//! The committed `spike/pyish/` output is what the reader and lowering
//! produce from `pyish.sdf3`, byte for byte -- grammar, findings, the
//! generated indent-stack scanner, and the ANTLR grammar.

use std::path::Path;

fn spike() -> &'static Path {
    Path::new(concat!(env!("CARGO_MANIFEST_DIR"), "/spike/pyish"))
}

#[test]
fn pyish_reads_and_lowers_to_the_committed_output() {
    let module = treebank_sdf3::load_module(&spike().join("pyish.sdf3")).unwrap();
    assert_eq!(module.name, "pyish");
    let lowered = treebank_sdf3::lower(&module).unwrap();

    let produced = serde_json::to_string_pretty(&lowered.grammar).unwrap() + "\n";
    let committed = std::fs::read_to_string(spike().join("grammar.json")).unwrap();
    assert_eq!(
        produced, committed,
        "spike/pyish/grammar.json is stale; regenerate it"
    );

    let report = treebank_sdf3::report(&lowered.findings);
    let committed = std::fs::read_to_string(spike().join("findings.md")).unwrap();
    assert_eq!(
        report, committed,
        "spike/pyish/findings.md is stale; regenerate it"
    );

    let scanner = lowered
        .scanner
        .as_ref()
        .expect("pyish's layout constraints call for a scanner");
    let committed = std::fs::read_to_string(spike().join("src/scanner.c")).unwrap();
    assert_eq!(
        *scanner, committed,
        "spike/pyish/src/scanner.c is stale; regenerate it"
    );

    let antlr = treebank_sdf3::antlr::emit(&module, &lowered.names, &lowered.levels).unwrap();
    let committed = std::fs::read_to_string(spike().join("Pyish.g4")).unwrap();
    assert_eq!(
        antlr.grammar, committed,
        "spike/pyish/Pyish.g4 is stale; regenerate it"
    );
}

#[test]
fn the_binding_attributes_lower_to_the_committed_data_and_query() {
    let module = treebank_sdf3::load_module(&spike().join("pyish.sdf3")).unwrap();
    let lowered = treebank_sdf3::lower(&module).unwrap();
    let b = treebank_sdf3::bindings::emit(&module, &lowered.names)
        .unwrap()
        .expect("pyish declares bindings");
    let produced = serde_json::to_string_pretty(&b.json).unwrap() + "\n";
    let committed = std::fs::read_to_string(spike().join("bindings.json")).unwrap();
    assert_eq!(produced, committed, "spike/pyish/bindings.json is stale");
    let committed = std::fs::read_to_string(spike().join("queries/locals.scm")).unwrap();
    assert_eq!(b.locals, committed, "spike/pyish/queries/locals.scm is stale");
    let committed = std::fs::read_to_string(spike().join("bindings-findings.md")).unwrap();
    assert_eq!(
        treebank_sdf3::report(&b.findings),
        committed,
        "spike/pyish/bindings-findings.md is stale"
    );
    // The facets treebank's roles.json would carry, derived.
    assert_eq!(b.json["facets"]["_scope"], serde_json::json!(["def", "program"]));
    assert_eq!(
        b.json["facets"]["_binding"],
        serde_json::json!(["assign", "def", "global", "param"])
    );
    // Only the def name gets the parent-scope property; the module-directed
    // binding is a deviation the query cannot express.
    assert_eq!(b.locals.matches("#set!").count(), 1);
    assert!(b.findings.iter().any(|f| f.what.contains("cannot name a scope by kind")));
}

#[test]
fn the_declarative_constraints_become_three_externals_and_a_stack() {
    let module = treebank_sdf3::load_module(&spike().join("pyish.sdf3")).unwrap();
    let lowered = treebank_sdf3::lower(&module).unwrap();
    let g = &lowered.grammar;
    let externals: Vec<&str> = g["externals"]
        .as_array()
        .unwrap()
        .iter()
        .map(|e| e["name"].as_str().unwrap())
        .collect();
    assert_eq!(
        externals,
        ["_newline", "_indent", "_dedent", "_error_sentinel"]
    );
    // `indent 1 4` on Stmt.If: the block is wrapped.
    let if_rule = serde_json::to_string(&g["rules"]["if"]).unwrap();
    assert!(if_rule.contains(r#"{"type":"SYMBOL","name":"_indent"}"#), "{if_rule}");
    assert!(if_rule.contains(r#"{"type":"SYMBOL","name":"_dedent"}"#));
    // ...and it ends with the block or the else clause, never `_newline`.
    assert!(!if_rule.contains("_newline"), "{if_rule}");
    // `align-list` on Block and Program: a simple statement ends with `_newline`.
    let pass = serde_json::to_string(&g["rules"]["pass"]).unwrap();
    assert!(pass.contains("_newline"), "{pass}");
    let scanner = lowered.scanner.unwrap();
    assert!(scanner.contains("cols[MAX_DEPTH]"), "the scanner keeps a column stack");
    assert!(scanner.contains("external_scanner_serialize(void *payload, char *buffer) {\n  Indent *s"), "and serializes it");
}

#[test]
fn a_sort_with_one_constructor_is_named_for_the_constructor() {
    let module = treebank_sdf3::load_module(&spike().join("pyish.sdf3")).unwrap();
    let lowered = treebank_sdf3::lower(&module).unwrap();
    assert!(lowered.grammar["rules"].get("else_clause").is_some());
    assert!(lowered.grammar["rules"].get("else").is_none());
}
