//! The committed `spike/cppish/` output is what the loader, lowering and
//! pinned conflict set produce from `cppish.sdf3` and its import.

use std::path::Path;

fn spike() -> &'static Path {
    Path::new(concat!(env!("CARGO_MANIFEST_DIR"), "/spike/cppish"))
}

#[test]
fn cppish_imports_cish_and_lowers_to_the_committed_output() {
    let module = treebank_sdf3::load_module(&spike().join("cppish.sdf3")).unwrap();
    assert_eq!(module.name, "cppish");
    assert_eq!(module.imports, vec!["cish"]);
    // cish's productions arrived through the import; cppish added one.
    assert!(module.productions(false).count() > 10);
    assert_eq!(
        module
            .productions(false)
            .filter(|p| p.constructor.as_deref() == Some("TemplateId"))
            .count(),
        1
    );

    let mut lowered = treebank_sdf3::lower(&module).unwrap();
    let conflicts = treebank_sdf3::read_conflicts(&spike().join("tree-sitter.conflicts.json"))
        .unwrap()
        .expect("the carry needs a pinned conflict set");
    let conflict_findings = treebank_sdf3::apply_conflicts(&mut lowered.grammar, &conflicts);
    lowered.findings.extend(conflict_findings);

    let produced = serde_json::to_string_pretty(&lowered.grammar).unwrap() + "\n";
    let committed = std::fs::read_to_string(spike().join("grammar.json")).unwrap();
    assert_eq!(
        produced, committed,
        "spike/cppish/grammar.json is stale; regenerate it"
    );

    let report = treebank_sdf3::report(&lowered.findings);
    let committed = std::fs::read_to_string(spike().join("findings.md")).unwrap();
    assert_eq!(
        report, committed,
        "spike/cppish/findings.md is stale; regenerate it"
    );
    assert!(
        lowered.scanner.is_none(),
        "cppish has no layout constraints"
    );
}

#[test]
fn prefer_becomes_a_dynamic_weight_and_the_conflict_is_declared() {
    let module = treebank_sdf3::load_module(&spike().join("cppish.sdf3")).unwrap();
    let g = treebank_sdf3::lower_all(&module).unwrap().lowered.grammar;
    let t = &g["rules"]["template_id"];
    assert_eq!(t["type"], "PREC_DYNAMIC");
    assert_eq!(t["value"], 1);
    let conflicts = treebank_sdf3::read_conflicts(&spike().join("tree-sitter.conflicts.json"))
        .unwrap()
        .unwrap();
    assert!(!conflicts.is_empty());
}
