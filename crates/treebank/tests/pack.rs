//! The pack loader, against a real pack.
//!
//! Skipped rather than failed when no pack is present: the packs are build
//! artifacts, so a checkout that has not run tools/wasm-pack/build.sh has
//! nothing to test against, and failing there would mean the test suite
//! reported a missing artifact as a broken loader.
#![cfg(feature = "pack")]

use std::path::PathBuf;

use treebank::pack::Pack;

/// Queries arrived at pack_abi 3. A checkout whose packs predate that should
/// skip these rather than fail: the pack is stale, not the code.
fn a_queryable_pack() -> Option<Pack> {
    let pack = Pack::from_path(a_pack()?).ok()?;
    if pack.provenance().pack_abi < 3 {
        eprintln!(
            "pack is pack_abi {}; queries need 3. Rebuild with tools/wasm-pack/build.sh",
            pack.provenance().pack_abi
        );
        return None;
    }
    Some(pack)
}

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

/// Compiling a pack costs a few hundred milliseconds and loading a cached one
/// costs a few, so this is the startup cost of every tool that uses a grammar.
/// Asserts the cache is actually hit, in a clean directory rather than
/// whatever the developer's happens to hold.
///
/// The ratio rather than a duration, because this runs in a debug profile
/// where cranelift is unoptimised and everything is roughly ten times slower.
#[test]
fn a_second_load_uses_the_compiled_cache() {
    let Some(path) = a_pack() else { return };
    let dir = std::env::temp_dir().join(format!("treebank-cachetest-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);

    // SAFETY: single-threaded test; the variable is read on the next line's
    // call and nothing else in this process depends on it.
    unsafe { std::env::set_var("TREEBANK_CACHE", &dir) };

    let cold = std::time::Instant::now();
    Pack::from_path(&path).expect("cold load");
    let cold = cold.elapsed();

    let warm = std::time::Instant::now();
    Pack::from_path(&path).expect("warm load");
    let warm = warm.elapsed();

    let cached: Vec<_> = std::fs::read_dir(dir.join("compiled"))
        .expect("the cache directory should exist")
        .filter_map(|e| e.ok())
        .filter(|e| e.path().extension().is_some_and(|x| x == "cwasm"))
        .collect();
    assert_eq!(cached.len(), 1, "one compiled artifact, got {}", cached.len());

    // Deliberately loose. The point is that the second load does not recompile,
    // not that it hits a particular number on a particular machine.
    assert!(
        warm * 4 < cold,
        "a cached load should be far faster than compiling: cold {cold:?}, warm {warm:?}"
    );
    let _ = std::fs::remove_dir_all(&dir);
}

/// The vocabulary's whole purpose: one query, several languages, whatever each
/// one calls its declarations. Skipped unless more than one pack is present,
/// because a single-language run would prove nothing about portability.
#[test]
fn one_query_runs_against_every_grammar() {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
    let samples: Vec<(&str, &str)> = vec![
        ("python", "def greet(n):\n    return n\n\nclass P:\n    pass\n"),
        ("rust", "fn largest(x: u8) -> u8 { x }\nstruct P { a: u8 }\n"),
        ("typescript", "function greet(n: string) { return n }\nclass P {}\n"),
    ];

    let mut ran = 0;
    for (lang, src) in samples {
        let Some(path) = ["dist/wasm", "site/public/packs"]
            .iter()
            .map(|d| root.join(d).join(format!("treebank-{lang}.wasm")))
            .find(|p| p.is_file())
        else {
            continue;
        };
        let pack = Pack::from_path(&path).expect("load");
        if pack.provenance().pack_abi < 3 {
            continue; // predates queries; nothing to assert here
        }
        let tree = pack.parse(src).expect("parse");

        let found = pack.query(&tree, "(_declaration) @decl").expect("query");
        assert!(
            found.len() >= 2,
            "{lang}: expected both declarations, got {:?}",
            found.iter().map(|c| &c.kind).collect::<Vec<_>>()
        );
        assert!(found.iter().all(|c| c.name == "decl"), "{lang}: capture name");
        // Ranges must point into the source that was parsed.
        assert!(found.iter().all(|c| c.range.end <= src.len()), "{lang}: range");
        // The node types differ per language; that is the point.
        assert!(found.iter().any(|c| c.kind.contains("function")), "{lang}: a function");

        // A facet has to be expanded before it can run at all.
        let callable = pack.query(&tree, "(_callable) @fn").expect("facet query");
        assert!(!callable.is_empty(), "{lang}: (_callable) found nothing");
        ran += 1;
    }

    if ran < 2 {
        eprintln!("only {ran} pack(s) present; build more with tools/wasm-pack/build.sh");
    }
}

#[test]
fn a_broken_query_says_where() {
    let Some(pack) = a_queryable_pack() else { return };
    let tree = pack.parse("x = 1").expect("parse");

    let err = pack.query(&tree, "(nonexistent_node) @x").unwrap_err().to_string();
    assert!(err.contains("node type"), "should name the problem: {err}");
    assert!(err.contains("byte 1"), "should give the position: {err}");

    // An unbalanced query is a syntax error rather than a panic.
    assert!(pack.query(&tree, "(module").is_err());
}

/// A newer loader must still drive an older pack. Queries arrived at pack_abi
/// 3, and every pack published before that has none of the exports -- so
/// binding them unconditionally made this crate refuse every pack currently
/// served from treebank.dev. It did, until this test existed.
#[test]
fn an_older_pack_still_works_without_queries() {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
    let old = root.join("crates/treebank/tests/fixtures/pack-abi-2.wasm");
    if !old.is_file() {
        eprintln!("no abi-2 fixture; skipping");
        return;
    }
    let pack = Pack::from_path(&old).expect("an older pack must still load");
    assert!(pack.provenance().pack_abi < 3, "fixture should predate queries");

    // Everything that is not a query works exactly the same.
    let tree = pack.parse("def f(x):\n    return x\n").expect("parse");
    assert_eq!(tree.root().kind().unwrap(), "module");
    assert!(!tree.root().has_error().unwrap());
    assert!(!pack.roles().facets.is_empty());

    // And a query fails with something a reader can act on.
    let err = pack.query(&tree, "(_declaration) @d").unwrap_err().to_string();
    assert!(err.contains("pack_abi"), "should name the version: {err}");
    assert!(err.contains("expand_query"), "should offer the way round it: {err}");
}
