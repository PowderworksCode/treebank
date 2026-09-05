//! The committed `spike/mini/grammar.json` is what the reader and lowering
//! produce from `spike/mini/mini.sdf3`, byte for byte. Regenerate with
//! `cargo run -p treebank-sdf3 --example lower -- crates/treebank-sdf3/spike/mini/mini.sdf3`.

use std::path::Path;

fn spike() -> &'static Path {
    Path::new(concat!(env!("CARGO_MANIFEST_DIR"), "/spike/mini"))
}

#[test]
fn mini_reads_and_lowers_to_the_committed_grammar() {
    let text = std::fs::read_to_string(spike().join("mini.sdf3")).unwrap();
    let module = treebank_sdf3::parse_module(&text).unwrap();
    assert_eq!(module.name, "mini");
    assert_eq!(
        module.productions(false).count(),
        21,
        "context-free productions"
    );
    assert_eq!(module.productions(true).count(), 4, "lexical productions");

    let lowered = treebank_sdf3::lower(&module).unwrap();
    let produced = serde_json::to_string_pretty(&lowered.grammar).unwrap() + "\n";
    let committed = std::fs::read_to_string(spike().join("grammar.json")).unwrap();
    assert_eq!(
        produced, committed,
        "spike/mini/grammar.json is stale; regenerate it"
    );

    let report = treebank_sdf3::report(&lowered.findings);
    let committed = std::fs::read_to_string(spike().join("findings.md")).unwrap();
    assert_eq!(
        report, committed,
        "spike/mini/findings.md is stale; regenerate it"
    );
}

#[test]
fn the_lowering_says_what_it_cannot_keep() {
    use treebank_sdf3::Kind;
    let text = std::fs::read_to_string(spike().join("mini.sdf3")).unwrap();
    let module = treebank_sdf3::parse_module(&text).unwrap();
    let lowered = treebank_sdf3::lower(&module).unwrap();
    let count = |k: Kind| lowered.findings.iter().filter(|f| f.kind == k).count();
    assert_eq!(count(Kind::Unsupported), 0);
    // The two non-assoc operators, and nothing else, widen.
    assert_eq!(count(Kind::Widening), 2);
    // The one bracket production changes the tree's shape.
    assert_eq!(count(Kind::Deviation), 1);
    // Placeholder labels are the extension, one finding per label.
    assert_eq!(count(Kind::Extension), 30);
}

#[test]
fn sorts_become_supertypes_and_keywords_become_reserved() {
    let text = std::fs::read_to_string(spike().join("mini.sdf3")).unwrap();
    let module = treebank_sdf3::parse_module(&text).unwrap();
    let g = treebank_sdf3::lower(&module).unwrap().grammar;
    assert_eq!(g["supertypes"], serde_json::json!(["_stmt", "_exp"]));
    assert_eq!(g["word"], "id");
    let reserved: Vec<&str> = g["reserved"]["global"]
        .as_array()
        .unwrap()
        .iter()
        .map(|w| w["value"].as_str().unwrap())
        .collect();
    assert_eq!(reserved, ["else", "fun", "if", "let", "return", "while"]);
    // The start symbol is the first rule.
    let first = g["rules"].as_object().unwrap().keys().next().unwrap();
    assert_eq!(first, "program");
}
