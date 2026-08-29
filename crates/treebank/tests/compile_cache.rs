//! The compile cache, in a process of its own.
//!
//! This lives apart from tests/pack.rs deliberately. The test points
//! `TREEBANK_CACHE` at an empty directory and then counts what lands in it,
//! and an environment variable belongs to the process rather than to the
//! thread -- so run beside its siblings, every other test that loads a pack
//! compiles into the directory being counted, and the count is of their work
//! as much as its own. Cargo gives each file in tests/ its own binary, which
//! is the isolation this needs.
//!
//! Skipped rather than failed when no pack is present, as tests/pack.rs is.
#![cfg(feature = "pack")]

use treebank::pack::Pack;

mod common;
use common::a_pack;

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
