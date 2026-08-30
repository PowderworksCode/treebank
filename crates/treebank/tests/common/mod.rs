//! Shared by the pack test binaries.
//!
//! A directory under tests/ is not itself compiled into a test binary, which
//! is what makes this the place for something several of them need.

use std::path::PathBuf;

/// Whether a skip here is a bug rather than a missing artifact.
///
/// Set by the one CI job that has already built the packs. Everywhere else a
/// skip is correct, and this is what stops that correctness from quietly
/// applying to a job whose whole purpose is to exercise the loader: a suite
/// that reports clean over nothing is the failure this crate refuses to ship
/// in its own sweep, and it should not ship it in its tests either.
pub fn packs_are_required() -> bool {
    std::env::var_os("TREEBANK_REQUIRE_PACK").is_some_and(|v| v != "0" && !v.is_empty())
}

/// Skip, unless a pack was promised. Panics with what to fix when one was.
pub fn skip(reason: &str) {
    assert!(
        !packs_are_required(),
        "TREEBANK_REQUIRE_PACK is set: {reason}"
    );
    eprintln!("skipping: {reason}");
}

/// The packs are build artifacts, so a checkout that has not run
/// tools/wasm-pack/build.sh has nothing to test against. Every caller treats
/// `None` as "skip", never as a failure: a missing artifact is not a broken
/// loader -- unless `TREEBANK_REQUIRE_PACK` says one is there.
pub fn a_pack() -> Option<PathBuf> {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
    for dir in ["dist/wasm", "site/public/packs"] {
        let path = root.join(dir).join("treebank-python.wasm");
        if path.is_file() {
            return Some(path);
        }
    }
    skip("no treebank-python.wasm; run tools/wasm-pack/build.sh python");
    None
}
