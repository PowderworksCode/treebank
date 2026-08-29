//! Shared by the pack test binaries.
//!
//! A directory under tests/ is not itself compiled into a test binary, which
//! is what makes this the place for something several of them need.

use std::path::PathBuf;

/// The packs are build artifacts, so a checkout that has not run
/// tools/wasm-pack/build.sh has nothing to test against. Every caller treats
/// `None` as "skip", never as a failure: a missing artifact is not a broken
/// loader.
pub fn a_pack() -> Option<PathBuf> {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
    for dir in ["dist/wasm", "site/public/packs"] {
        let path = root.join(dir).join("treebank-python.wasm");
        if path.is_file() {
            return Some(path);
        }
    }
    None
}
