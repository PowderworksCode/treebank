//! The pack loader, against a real pack.
//!
//! Skipped rather than failed when no pack is present: the packs are build
//! artifacts, so a checkout that has not run tools/wasm-pack/build.sh has
//! nothing to test against, and failing there would mean the test suite
//! reported a missing artifact as a broken loader.
#![cfg(feature = "pack")]

use std::path::PathBuf;

use treebank::pack::Pack;

fn a_pack() -> Option<PathBuf> {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
    for dir in ["dist/wasm", "site/public/packs"] {
        let path = root.join(dir).join("treebank-python.wasm");
        if path.is_file() {
            return Some(path);
        }
    }
    None
}

#[test]
fn parses_and_answers_for_itself() {
    let Some(path) = a_pack() else {
        eprintln!("no treebank-python.wasm; run tools/wasm-pack/build.sh python");
        return;
    };
    let pack = Pack::from_path(&path).expect("load");

    let p = pack.provenance();
    assert_eq!(p.language, "python");
    assert_eq!(pack.language(), "python");
    assert!(!p.vocabulary.is_empty());
    // Provenance is a source hash rather than an upstream version.
    assert!(p.sources.contains_key("grammar.js"));

    let tree = pack.parse("def f(x):\n    return x + 1\n").expect("parse");
    let root = tree.root();
    assert_eq!(root.kind().unwrap(), "module");
    assert!(!root.has_error().unwrap(), "clean source should not carry an error");
    assert!(root.sexp().unwrap().starts_with("(module"));
    assert_eq!(root.byte_range().unwrap().start, 0);

    let kids = root.named_children().unwrap();
    assert_eq!(kids.len(), 1);
    assert_eq!(kids[0].kind().unwrap(), "function_definition");

    // Field names belong to the parent's view of a child.
    let f = &kids[0];
    let names: Vec<_> = (0..f.child_count(false).unwrap())
        .filter_map(|i| f.field_name_for_child(i).unwrap())
        .collect();
    assert!(names.iter().any(|n| n == "name"), "expected a name field, got {names:?}");
}

#[test]
fn reports_errors() {
    let Some(path) = a_pack() else { return };
    let pack = Pack::from_path(&path).expect("load");
    let tree = pack.parse("def f(:\n    return 1\n").expect("parse");
    let root = tree.root();
    assert!(root.has_error().unwrap(), "broken source should carry an error");
}

#[test]
fn expands_a_facet_query_against_the_packs_own_manifest() {
    let Some(path) = a_pack() else { return };
    let pack = Pack::from_path(&path).expect("load");
    let facets = &pack.roles().facets;
    assert!(!facets.is_empty(), "a pack carries its facet manifest");

    let (term, members) = facets.iter().next().unwrap();
    let expanded = pack.expand_query(&format!("({term})")).expect("expand");
    // The whole point: the facet name is replaced by this grammar's members.
    assert!(!expanded.contains(term), "facet should be expanded away: {expanded}");
    assert!(expanded.contains(&members[0]), "expected {} in {expanded}", members[0]);
}
