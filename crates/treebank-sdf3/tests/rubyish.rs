//! The committed `spike/rubyish/` output is what the reader and lowering
//! produce from `rubyish.sdf3`, byte for byte -- grammar, findings, and the
//! generated scanner.

use std::path::Path;

fn spike() -> &'static Path {
    Path::new(concat!(env!("CARGO_MANIFEST_DIR"), "/spike/rubyish"))
}

#[test]
fn rubyish_reads_and_lowers_to_the_committed_output() {
    let text = std::fs::read_to_string(spike().join("rubyish.sdf3")).unwrap();
    let module = treebank_sdf3::parse_module(&text).unwrap();
    assert_eq!(module.name, "rubyish");
    let lowered = treebank_sdf3::lower_all(&module).unwrap().lowered;

    let produced = serde_json::to_string_pretty(&lowered.grammar).unwrap() + "\n";
    let committed = std::fs::read_to_string(spike().join("grammar.json")).unwrap();
    assert_eq!(
        produced, committed,
        "spike/rubyish/grammar.json is stale; regenerate it"
    );

    let report = treebank_sdf3::report(&lowered.findings);
    let committed = std::fs::read_to_string(spike().join("findings.md")).unwrap();
    assert_eq!(
        report, committed,
        "spike/rubyish/findings.md is stale; regenerate it"
    );

    let scanner = lowered
        .scanner
        .expect("rubyish's layout constraints call for a scanner");
    let committed = std::fs::read_to_string(spike().join("src/scanner.c")).unwrap();
    assert_eq!(
        scanner, committed,
        "spike/rubyish/src/scanner.c is stale; regenerate it"
    );
}

#[test]
fn the_constrained_spellings_are_split_and_the_rest_are_not() {
    let text = std::fs::read_to_string(spike().join("rubyish.sdf3")).unwrap();
    let module = treebank_sdf3::parse_module(&text).unwrap();
    let g = treebank_sdf3::lower_all(&module).unwrap().lowered.grammar;
    let externals: Vec<&str> = g["externals"]
        .as_array()
        .unwrap()
        .iter()
        .map(|e| e["name"].as_str().unwrap())
        .collect();
    for expected in [
        "_minus",
        "_minus_spaced_tight",
        "_star",
        "_star_spaced_tight",
        "_lbracket_adjacent",
        "_lbracket_spaced",
        "_lparen_adjacent",
        "_lparen_spaced",
        "_slash",
        "regex",
        "_error_sentinel",
    ] {
        assert!(
            externals.contains(&expected),
            "missing external {expected}; have {externals:?}"
        );
    }
    // `+` has no constraint anywhere, so it stays an ordinary string token.
    assert!(!externals.iter().any(|e| e.contains("plus")));
    assert_eq!(externals.last(), Some(&"_error_sentinel"));
}
