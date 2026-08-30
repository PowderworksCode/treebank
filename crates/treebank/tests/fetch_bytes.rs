//! Acquisition without an engine, and without the network.
//!
//! Nothing here reaches out. A pinned fetch consults no manifest, so a cache
//! entry whose hash matches is the whole answer -- which is exactly the
//! property that makes a pinned build reproducible and offline once warm, and
//! the one worth asserting.
//!
//! `TREEBANK_CACHE` belongs to the process rather than to the thread, so it
//! is set once here and every test shares the directory. What keeps them
//! apart is the cache layout itself: an entry is named by its own digest, so
//! tests with different content cannot land on the same path.
#![cfg(feature = "fetch-bytes")]

use std::fs;
use std::path::PathBuf;
use std::sync::OnceLock;

use treebank::fetch::{cached_path, fetch_pinned_bytes};

fn shared_cache() -> &'static PathBuf {
    static DIR: OnceLock<PathBuf> = OnceLock::new();
    DIR.get_or_init(|| {
        let dir = std::env::temp_dir().join(format!("treebank-fetchtest-{}", std::process::id()));
        // SAFETY: set exactly once, before any test reads it, and never
        // changed afterwards.
        unsafe { std::env::set_var("TREEBANK_CACHE", &dir) };
        dir
    })
}

fn digest(bytes: &[u8]) -> String {
    use sha2::{Digest, Sha256};
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    hasher
        .finalize()
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect()
}

/// Seed the cache with `content` under the name `hash` claims, and return
/// both the path and that hash.
fn seed(content: &[u8], named_for: &[u8]) -> (PathBuf, String) {
    let hash = digest(named_for);
    let path = cached_path(&format!("treebank-python-{}.wasm", &hash[..12]));
    fs::create_dir_all(path.parent().expect("a parent")).expect("cache dir");
    fs::write(&path, content).expect("seed the cache");
    (path, hash)
}

#[test]
fn a_pinned_fetch_is_served_from_the_cache() {
    let _ = shared_cache();
    let pack = b"a warm pinned fetch needs no network";
    let (_, hash) = seed(pack, pack);

    let bytes = fetch_pinned_bytes("python", &hash).expect("served from the cache");
    assert_eq!(bytes, pack);
}

#[test]
fn a_cache_entry_that_does_not_match_its_name_is_discarded() {
    let _ = shared_cache();
    // Named for one digest, holding something else: what a truncated download
    // or a substituted file looks like on disk.
    let claimed = b"the bytes this entry claims to hold";
    let (path, hash) = seed(b"substituted", claimed);

    // Offline this then fails at the download, which is the point -- what is
    // under test is that the bytes on disk were never handed back.
    let _ = fetch_pinned_bytes("python", &hash);
    assert!(
        !path.is_file(),
        "a mismatching entry should be removed, not trusted"
    );
}

#[test]
fn a_hash_that_is_not_a_hash_is_refused_before_anything_is_read() {
    let _ = shared_cache();

    let error = fetch_pinned_bytes("python", "../../etc/passwd").expect_err("refused");
    assert!(error.to_string().contains("not a pack hash"), "{error}");

    let error = fetch_pinned_bytes("python", "abc").expect_err("too short");
    assert!(error.to_string().contains("not a pack hash"), "{error}");
}
