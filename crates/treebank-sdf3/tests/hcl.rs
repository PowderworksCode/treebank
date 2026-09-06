//! The committed `spike/hcl/` output is what the reader and lowering
//! produce from `hcl.sdf3`, byte for byte -- grammar, findings, the
//! generated scanner (newline token, template automata, delimiter stack),
//! and the other two backends' grammars.

use std::path::Path;

fn spike() -> &'static Path {
    Path::new(concat!(env!("CARGO_MANIFEST_DIR"), "/spike/hcl"))
}

#[test]
fn hcl_reads_and_lowers_to_the_committed_output() {
    let module = treebank_sdf3::load_module(&spike().join("hcl.sdf3")).unwrap();
    assert_eq!(module.name, "hcl");
    let lowered = treebank_sdf3::lower_all(&module).unwrap().lowered;

    let produced = serde_json::to_string_pretty(&lowered.grammar).unwrap() + "\n";
    let committed = std::fs::read_to_string(spike().join("grammar.json")).unwrap();
    assert_eq!(
        produced, committed,
        "spike/hcl/grammar.json is stale; regenerate it"
    );

    let report = treebank_sdf3::report(&lowered.findings);
    let committed = std::fs::read_to_string(spike().join("findings.md")).unwrap();
    assert_eq!(
        report, committed,
        "spike/hcl/findings.md is stale; regenerate it"
    );

    let scanner = lowered
        .scanner
        .as_ref()
        .expect("hcl's kernel syntax and delimiter call for a scanner");
    let committed = std::fs::read_to_string(spike().join("src/scanner.c")).unwrap();
    assert_eq!(
        *scanner, committed,
        "spike/hcl/src/scanner.c is stale; regenerate it"
    );

    let antlr = treebank_sdf3::antlr::emit(&module, &lowered.names, &lowered.levels).unwrap();
    let committed = std::fs::read_to_string(spike().join("Hcl.g4")).unwrap();
    assert_eq!(
        antlr.grammar, committed,
        "spike/hcl/Hcl.g4 is stale; regenerate it"
    );

    let wn = treebank_sdf3::winnow::emit(&module, &lowered.names, &lowered.levels).unwrap();
    let committed = std::fs::read_to_string(spike().join("winnow/src/main.rs")).unwrap();
    assert_eq!(
        wn.source, committed,
        "spike/hcl/winnow/src/main.rs is stale; regenerate it"
    );
}

#[test]
fn the_scanner_owns_what_kernel_syntax_reaches_and_the_delimiter() {
    let module = treebank_sdf3::load_module(&spike().join("hcl.sdf3")).unwrap();
    let (plan, _) = treebank_sdf3::scanner::plan(&module).unwrap();
    let owned: Vec<&str> = plan.owned.iter().map(|o| o.sort.as_str()).collect();
    for sort in [
        "QUOTE",
        "_QCHUNK",
        "_HCHUNK",
        "ESCAPE_SEQUENCE",
        "_DIR_ENDIF",
        "HEREDOC_START",
        "HEREDOC_END",
        "_NL",
    ] {
        assert!(owned.contains(&sort), "{sort} should be scanner-owned: {owned:?}");
    }
    // The internal lexer keeps what layout may precede.
    for sort in ["IDENTIFIER", "STRING_LIT", "INTEGER"] {
        assert!(!owned.contains(&sort), "{sort} should stay a token rule");
    }
    let roles: Vec<(&str, treebank_sdf3::scanner::OwnedRole)> = plan
        .owned
        .iter()
        .map(|o| (o.sort.as_str(), o.role))
        .collect();
    assert!(roles.contains(&("HEREDOC_START", treebank_sdf3::scanner::OwnedRole::Opener)));
    assert!(roles.contains(&("HEREDOC_END", treebank_sdf3::scanner::OwnedRole::Closer)));
    // `foo.0`: adjacency on a lexical sort is `token.immediate`.
    assert!(!plan.immediate.is_empty());
    let scanner = treebank_sdf3::lower(&module).unwrap().scanner.unwrap();
    assert!(scanner.contains("scan_owned"));
    assert!(scanner.contains("word_on_top"));
}

#[test]
fn one_widening_and_it_is_the_skipped_layout_before_a_kernel_token() {
    let module = treebank_sdf3::load_module(&spike().join("hcl.sdf3")).unwrap();
    let lowered = treebank_sdf3::lower_all(&module).unwrap().lowered;
    let widenings: Vec<&str> = lowered
        .findings
        .iter()
        .filter(|f| f.kind == treebank_sdf3::Kind::Widening)
        .map(|f| f.what.as_str())
        .collect();
    assert_eq!(widenings.len(), 1, "{widenings:?}");
    assert!(widenings[0].contains("_QCHUNK"));
    assert!(lowered
        .findings
        .iter()
        .all(|f| f.kind != treebank_sdf3::Kind::Unsupported));
}
