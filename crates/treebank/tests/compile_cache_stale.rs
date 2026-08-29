//! Recovery from a compiled artifact this runtime cannot read.
//!
//! Its own binary, like tests/compile_cache.rs and for the same reason: it
//! points `TREEBANK_CACHE` at a directory it then inspects, and that variable
//! belongs to the process rather than to the thread.
#![cfg(feature = "pack")]

use treebank::pack::Pack;

mod common;
use common::a_pack;

/// A compiled artifact outlives the runtime that wrote it. The cache key
/// covers the wasm bytes and the host but not the runtime version, on the
/// grounds that the runtime stamps its own version into the artifact and
/// refuses one it did not write -- so a stale entry should fail to load, be
/// deleted, and be rebuilt. That claim is what makes upgrading the runtime
/// safe for someone with a warm cache, and it is the kind of thing that is
/// true until it quietly is not.
///
/// It is load-bearing here in a way it was not under wasmtime: wasmer's
/// `Module::deserialize` does NOT validate, and only
/// `deserialize_from_file` -- what the cache calls -- does. If that ever
/// changes to the unchecked reader, this test is what says so.
///
/// Unreadable bytes stand in for an artifact from another version. Both
/// reach the same reader: it refuses, and the recovery is the thing under
/// test.
#[test]
fn a_foreign_compiled_artifact_is_rebuilt_rather_than_trusted() {
    let Some(path) = a_pack() else { return };
    let dir = std::env::temp_dir().join(format!("treebank-staletest-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);

    // SAFETY: single-threaded, and this file is its own test binary precisely
    // so that no sibling test observes the variable.
    unsafe { std::env::set_var("TREEBANK_CACHE", &dir) };

    Pack::from_path(&path).expect("first load");

    let artifact = std::fs::read_dir(dir.join("compiled"))
        .expect("the cache directory should exist")
        .filter_map(|e| e.ok())
        .map(|e| e.path())
        .find(|p| p.extension().is_some_and(|x| x == "cwasm"))
        .expect("a compiled artifact");
    let good = std::fs::metadata(&artifact).unwrap().len();
    std::fs::write(&artifact, b"not a compiled module").unwrap();

    Pack::from_path(&path).expect("a stale artifact should be rebuilt, not fatal");

    assert_eq!(
        std::fs::metadata(&artifact).unwrap().len(),
        good,
        "the artifact should have been recompiled in place"
    );
    let _ = std::fs::remove_dir_all(&dir);
}
