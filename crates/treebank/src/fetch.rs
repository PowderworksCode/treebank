//! Fetch a grammar, so using one is a line of code rather than a build step.
//!
//! ```no_run
//! use treebank::Pack;
//!
//! let pack = Pack::fetch("python")?;
//! let tree = pack.parse("def f(x): return x")?;
//! # Ok::<(), anyhow::Error>(())
//! ```
//!
//! Downloads are **verified and cached**, which is the whole reason packs are
//! content-addressed. The manifest names a sha256 for each grammar; the bytes
//! are checked against it before they are cached or parsed with, so a
//! corrupted download or a substituted file is an error rather than a strange
//! parse later on. A cache entry is named by its hash, so it is never stale
//! and never needs invalidating.
//!
//! Fetching reaches the network, which is a surprising thing for a library to
//! do unasked — so it only happens when you call one of these, and only when
//! the cache does not already have the bytes.
//!
//! For a build that must not vary, name the hash. [`Pack::fetch_pinned`]
//! consults no manifest, so it is reproducible and works offline once warm.

use std::fs;
use std::path::{Path, PathBuf};
use std::sync::OnceLock;

use anyhow::{anyhow, bail, Context, Result};
use serde::Deserialize;
use sha2::{Digest, Sha256};

#[cfg(feature = "pack")]
use crate::pack::Pack;

/// Where packs are served from. `TREEBANK_PACKS_URL` overrides it, for a
/// mirror or an air-gapped copy.
const DEFAULT_BASE: &str = "https://treebank.dev/packs";

/// How long the manifest may be reused before it is fetched again. The
/// manifest is the only mutable thing in the system; the packs it names are
/// immutable, so this is the one place a stale read can happen at all.
const MANIFEST_TTL: std::time::Duration = std::time::Duration::from_secs(3600);

#[derive(Debug, Deserialize)]
struct Manifest {
    packs: std::collections::BTreeMap<String, Entry>,
}

#[derive(Debug, Deserialize)]
struct Entry {
    sha256: String,
    key: String,
}

fn base_url() -> String {
    std::env::var("TREEBANK_PACKS_URL")
        .unwrap_or_else(|_| DEFAULT_BASE.to_string())
        .trim_end_matches('/')
        .to_string()
}

/// `$TREEBANK_CACHE`, else `$XDG_CACHE_HOME/treebank`, else
/// `~/.cache/treebank` — the same place the repository's own toolchain caches
/// into, so a checkout and a consumer do not keep two copies.
pub fn cache_dir() -> PathBuf {
    if let Ok(dir) = std::env::var("TREEBANK_CACHE") {
        return PathBuf::from(dir);
    }
    if let Ok(dir) = std::env::var("XDG_CACHE_HOME") {
        return PathBuf::from(dir).join("treebank");
    }
    if let Ok(home) = std::env::var("HOME") {
        return PathBuf::from(home).join(".cache").join("treebank");
    }
    std::env::temp_dir().join("treebank")
}

fn packs_dir() -> PathBuf {
    cache_dir().join("packs")
}

/// A pack is a few megabytes; the cap is here so a wrong URL that answers with
/// something enormous fails rather than fills a disk.
const BODY_LIMIT: u64 = 64 * 1024 * 1024;

/// One agent for the process, holding one TLS setup.
///
/// The crypto provider is installed rather than passed per request because
/// rustls keeps it as process state. Installing it is allowed to fail: that
/// means the program embedding this library already chose a provider, and
/// theirs should win over ours.
///
/// Roots come from the platform, which is what the `native-certs` feature did
/// before and is what keeps a proxy presenting its own certificate authority
/// working.
fn agent() -> &'static ureq::Agent {
    static AGENT: OnceLock<ureq::Agent> = OnceLock::new();
    AGENT.get_or_init(|| {
        let _ = rustls_graviola::default_provider().install_default();
        ureq::Agent::config_builder()
            .tls_config(
                ureq::tls::TlsConfig::builder()
                    .root_certs(ureq::tls::RootCerts::PlatformVerifier)
                    .build(),
            )
            .build()
            .into()
    })
}

fn get(url: &str) -> Result<Vec<u8>> {
    let mut response = agent()
        .get(url)
        .call()
        .with_context(|| format!("fetching {url}"))?;
    response
        .body_mut()
        .with_config()
        .limit(BODY_LIMIT)
        .read_to_vec()
        .with_context(|| format!("reading {url}"))
}

fn sha256_hex(bytes: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    hasher.finalize().iter().map(|b| format!("{b:02x}")).collect()
}

/// Write through a temporary file in the same directory, then rename. Two
/// processes fetching the same grammar at once is the ordinary case for a
/// build, and a half-written pack in the cache would be read as a whole one.
fn cache_atomically(path: &Path, bytes: &[u8]) -> Result<()> {
    let dir = path.parent().ok_or_else(|| anyhow!("no parent for {}", path.display()))?;
    fs::create_dir_all(dir).with_context(|| format!("creating {}", dir.display()))?;
    let tmp = dir.join(format!(".{}.{}", std::process::id(), rand_suffix()));
    fs::write(&tmp, bytes).with_context(|| format!("writing {}", tmp.display()))?;
    fs::rename(&tmp, path).with_context(|| format!("installing {}", path.display()))?;
    Ok(())
}

fn rand_suffix() -> String {
    use std::time::{SystemTime, UNIX_EPOCH};
    let nanos = SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.subsec_nanos()).unwrap_or(0);
    format!("{nanos:08x}")
}

fn manifest() -> Result<Manifest> {
    let path = cache_dir().join("packs-index.json");
    let fresh = fs::metadata(&path)
        .and_then(|m| m.modified())
        .map(|t| t.elapsed().map(|age| age < MANIFEST_TTL).unwrap_or(false))
        .unwrap_or(false);

    if fresh {
        if let Ok(bytes) = fs::read(&path) {
            if let Ok(parsed) = serde_json::from_slice(&bytes) {
                return Ok(parsed);
            }
        }
    }

    let url = format!("{}/index.json", base_url());
    match get(&url) {
        Ok(bytes) => {
            let parsed = serde_json::from_slice(&bytes)
                .with_context(|| format!("parsing the manifest from {url}"))?;
            let _ = cache_atomically(&path, &bytes);
            Ok(parsed)
        }
        // A stale manifest beats no manifest: the packs it names are immutable
        // and still exist, so an offline build gets the last known grammar
        // rather than failing.
        Err(network) => {
            let bytes = fs::read(&path).map_err(|_| network)?;
            serde_json::from_slice(&bytes).context("parsing the cached manifest")
        }
    }
}

/// Load a pack for `grammar`, downloading and caching it if necessary.
///
/// Resolves the current version through the manifest, so it follows the
/// grammar as it improves. Use [`fetch_pinned`] where that must not happen.
#[cfg(feature = "pack")]
pub fn fetch(grammar: &str) -> Result<Pack> {
    let (key, sha256) = key_for(grammar)?;
    load_verified(&key, &sha256)
}

/// Load an exact pack by the hash in its filename, e.g. `d82f4fd5c5a9`.
///
/// No manifest is consulted, so this is reproducible and needs no network
/// once the bytes are cached. This is what a build that must not vary should
/// call, and what a bug report's permalink names.
#[cfg(feature = "pack")]
pub fn fetch_pinned(grammar: &str, hash: &str) -> Result<Pack> {
    load_verified(&pinned_key(grammar, hash)?, hash)
}

/// The cache path a key would occupy, whether or not it is there.
pub fn cached_path(key: &str) -> PathBuf {
    packs_dir().join(key)
}

fn bytes_verified(key: &str, expected: &str) -> Result<Vec<u8>> {
    let path = cached_path(key);

    if let Ok(bytes) = fs::read(&path) {
        // A cache entry is named by its hash, so a mismatch means the file was
        // damaged or replaced. Re-fetching is the repair.
        if starts_with_hash(&sha256_hex(&bytes), expected) {
            return Ok(bytes);
        }
        let _ = fs::remove_file(&path);
    }

    let url = format!("{}/{key}", base_url());
    let bytes = get(&url)?;
    let actual = sha256_hex(&bytes);
    if !starts_with_hash(&actual, expected) {
        bail!(
            "{url} does not have the expected contents\n  expected sha256 {expected}\n  \
             got      sha256 {actual}"
        );
    }
    cache_atomically(&path, &bytes)?;
    Ok(bytes)
}

fn key_for(grammar: &str) -> Result<(String, String)> {
    let manifest = manifest()?;
    let entry = manifest.packs.get(grammar).ok_or_else(|| {
        let known: Vec<_> = manifest.packs.keys().cloned().collect();
        anyhow!("no grammar named {grammar}; the manifest has {}", known.join(", "))
    })?;
    Ok((entry.key.clone(), entry.sha256.clone()))
}

fn pinned_key(grammar: &str, hash: &str) -> Result<String> {
    if !hash.chars().all(|c| c.is_ascii_hexdigit()) || hash.len() < 8 {
        bail!("{hash} is not a pack hash");
    }
    Ok(format!("treebank-{grammar}-{}.wasm", &hash[..12.min(hash.len())]))
}

/// The verified bytes of a pack, for a host that has its own runtime.
///
/// Everything [`fetch`] does except the last step. A consumer that drives
/// packs through its own engine -- straitjacket materialises trees into an
/// arena its rule library can walk, which this crate's lazy handles cannot
/// satisfy -- would otherwise reimplement the manifest, the cache, the
/// atomic install and the hash check, which is how two consumers come to
/// disagree about which bytes are the python grammar.
///
/// Available without the `pack` feature, so taking it costs no engine.
#[cfg(feature = "fetch-bytes")]
pub fn fetch_bytes(grammar: &str) -> Result<Vec<u8>> {
    let (key, sha256) = key_for(grammar)?;
    bytes_verified(&key, &sha256)
}

/// The verified bytes of an exact pack, by the hash in its filename.
///
/// [`fetch_pinned`] without the engine, and the one a build that must not
/// vary should call: no manifest is consulted, so it is reproducible and
/// needs no network once the bytes are cached.
#[cfg(feature = "fetch-bytes")]
pub fn fetch_pinned_bytes(grammar: &str, hash: &str) -> Result<Vec<u8>> {
    bytes_verified(&pinned_key(grammar, hash)?, hash)
}

#[cfg(feature = "pack")]
fn load_verified(key: &str, expected: &str) -> Result<Pack> {
    let bytes = bytes_verified(key, expected)?;
    Pack::from_bytes(&bytes).with_context(|| format!("loading {key}"))
}

/// The manifest carries a full sha256 and a filename carries its first twelve
/// characters, so a pinned hash is compared as a prefix of the real one.
fn starts_with_hash(actual: &str, expected: &str) -> bool {
    let expected = expected.to_ascii_lowercase();
    !expected.is_empty() && actual.starts_with(&expected)
}

#[cfg(feature = "pack")]
impl Pack {
    /// Load a grammar, downloading and caching it if necessary. See
    /// [`fetch`].
    pub fn fetch(grammar: &str) -> Result<Pack> {
        fetch(grammar)
    }

    /// Load an exact grammar version by hash. See [`fetch_pinned`].
    pub fn fetch_pinned(grammar: &str, hash: &str) -> Result<Pack> {
        fetch_pinned(grammar, hash)
    }
}
