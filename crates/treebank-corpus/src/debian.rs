//! The artifact corpus: Debian source packages, shared by every language
//! that has no package registry of its own.
//!
//! C brought this path in and bash is the second user of it, which is what
//! moved it out of `c.rs`. The two need exactly the same three things — a
//! popularity signal, a source tarball for the same name, and a way to ask
//! what language a package is actually written in — and differ only in the
//! last one's answer. So everything here is common and the caller supplies
//! one predicate.
//!
//! What it biases toward, stated once so both callers inherit it:
//!
//! - Ranking is popcon (`popularity-contest`) install counts, aggregated per
//!   *source* package by Debian itself. It counts machines, not dependency
//!   edges, which makes it the same kind of metric as crates.io downloads
//!   rather than Java's dependent-repos proxy.
//! - The corpus is therefore **the code a distribution ships**: system
//!   libraries, daemons, autotools trees, packaging glue, decades-old code
//!   that still runs everything. It is emphatically not "trending on
//!   GitHub", and gap numbers from it will differ from a corpus that is.
//! - Debian is also the only source that supplies a *popularity signal and a
//!   source tarball for the same name*, which is why it beats vcpkg/Conan
//!   (no download counts) and a GitHub star scrape (no version identity).
//! - `sid`, not stable: treebank exists because grammars fall behind, so the
//!   corpus should be the newest code the distro carries.
//!
//! The tarball fetched is upstream's own release archive (`.orig.tar.*`), so
//! the corpus is upstream source; the `.debian.tar.xz` carrying the distro's
//! patches is deliberately not fetched.

use std::collections::{BTreeMap, HashMap};
use std::io::{BufRead, BufReader, Read};
use std::path::Path;
use std::sync::{LazyLock, Mutex};

use anyhow::{bail, Context, Result};
use serde::{Deserialize, Serialize};

use treebank_lang::LangName;
use crate::rank::RankedCrate;

const POPCON: &str = "https://popcon.debian.org/source/by_inst";
const MIRROR: &str = "https://deb.debian.org/debian";
pub const SUITE: &str = "sid";
const SOURCES: &str = "https://deb.debian.org/debian/dists/sid/main/source/Sources.gz";
/// How long a cached Sources index may be reused. Shorter than the daily
/// cron's period so an unattended run always sees the day's versions;
/// `TREEBANK_REFRESH_SOURCES=1` forces a refresh, as `TREEBANK_REFRESH_DUMP`
/// does for the rust dump in `bootstrap.sh`.
const SOURCES_MAX_AGE_HOURS: u64 = 12;

/// Debian pool coordinates for one source package, resolved at rank time.
/// `resolve()` gets no `db` path, so `rank()` leaves this index behind at a
/// fixed location for it to read — the same shape of arrangement as
/// `tools/*-oracle` being found relative to the repo root.
#[derive(Serialize, Deserialize, Clone)]
pub struct Pool {
    pub version: String,
    pub directory: String,
    pub file: String,
}

fn index_path(lang: LangName) -> String {
    format!("corpus/{lang}/db/index.json")
}

static POOLS: LazyLock<Mutex<HashMap<LangName, std::sync::Arc<HashMap<String, Pool>>>>> =
    LazyLock::new(|| Mutex::new(HashMap::new()));

fn pool_index(lang: LangName) -> std::sync::Arc<HashMap<String, Pool>> {
    if let Some(hit) = POOLS.lock().unwrap().get(&lang) {
        return hit.clone();
    }
    let loaded: HashMap<String, Pool> = std::fs::read_to_string(index_path(lang))
        .ok()
        .and_then(|s| serde_json::from_str(&s).ok())
        .unwrap_or_default();
    let arc = std::sync::Arc::new(loaded);
    POOLS.lock().unwrap().insert(lang, arc.clone());
    arc
}

/// Straight out of the pool, using the coordinates `rank()` recorded.
pub fn resolve(lang: LangName, pkg: &RankedCrate) -> Result<(String, String)> {
    let index = pool_index(lang);
    let pool = index.get(&pkg.name).with_context(|| {
        format!(
            "{}: not in {} — re-run `treebank rank --lang {lang}`",
            pkg.name,
            index_path(lang)
        )
    })?;
    Ok((
        pool.version.clone(),
        format!("{MIRROR}/{}/{}", pool.directory, pool.file),
    ))
}

/// One package's language census, as measured at a specific version.
///
/// The census is kept whole rather than reduced to the two counts one caller
/// happens to need, because the next caller needs a different pair. Flattened
/// into the same JSON object as `version`, which is also what makes the
/// cache written by the C-only version of this file still load: its
/// `{"version":…,"ansic":…,"cpp":…}` entries land in `langs` unchanged.
#[derive(Serialize, Deserialize, Clone)]
pub struct Sloc {
    pub version: String,
    #[serde(flatten)]
    pub langs: BTreeMap<String, i64>,
}

impl Sloc {
    /// SLOC in one sloccount language (`ansic`, `cpp`, `sh`, `perl`, …).
    pub fn lines(&self, lang: &str) -> i64 {
        self.langs.get(lang).copied().unwrap_or(0)
    }
}

/// popcon's per-source install counts, filtered by `keep` to the packages
/// that carry the language being ranked, newest-version pool coordinates
/// recorded for `resolve()`.
///
/// `what` is the language's own word for what `keep` selects; it only ever
/// reaches progress messages.
pub fn rank(
    lang: LangName,
    db: &Path,
    k: usize,
    what: &str,
    keep: &dyn Fn(&Sloc) -> bool,
) -> Result<Vec<RankedCrate>> {
    std::fs::create_dir_all(db)?;

    // 1. popcon: rank, source name, installs.
    eprintln!("rank: GET {POPCON}");
    let popcon = ureq::get(POPCON)
        .call()
        .with_context(|| format!("GET {POPCON}"))?
        .into_string()?;
    let mut ranked: Vec<(String, u64)> = Vec::new();
    for line in popcon.lines() {
        if line.starts_with('#') || line.starts_with("---") {
            continue;
        }
        let f: Vec<&str> = line.split_whitespace().collect();
        // "<rank> <name> <inst> <vote> <old> <recent> <no-files>"; the file
        // also carries a "Not in sid" pseudo-entry and a Total line.
        if f.len() < 3 || f[0].parse::<u64>().is_err() || f[1] == "Total" {
            continue;
        }
        let Ok(inst) = f[2].parse::<u64>() else { continue };
        ranked.push((f[1].to_string(), inst));
    }
    eprintln!("rank: popcon lists {} source packages", ranked.len());

    // 2. the Sources index, for versions and pool paths.
    let pool = load_sources(db)?;
    eprintln!("rank: {SUITE} index has {} sources with an orig tarball", pool.len());

    // 3. Walk popcon top-down, keeping the ones that carry the language.
    //
    // Batched rather than one-at-a-time: each batch resolves its cache misses
    // concurrently, then the batch is consumed IN POPCON ORDER, so the result
    // is identical to a sequential walk and does not depend on which lookup
    // finished first. Batches also mean we stop early — reaching k costs no
    // lookups beyond the batch that reached it.
    const BATCH: usize = 64;
    let mut sloc = load_sloc_cache(db);
    let cached_at_start = sloc.len();
    let mut out = Vec::new();
    let mut index: HashMap<String, Pool> = HashMap::new();
    let (mut not_in_sid, mut wrong_lang) = (0usize, 0usize);
    let (mut queried, mut reused, mut failed) = (0usize, 0usize, 0usize);
    let candidates: Vec<&(String, u64)> = ranked.iter().collect();
    'outer: for batch in candidates.chunks(BATCH) {
        // Anything absent from the index cannot be fetched at all; skip it
        // before spending a lookup on it.
        let present: Vec<&(String, u64)> = batch
            .iter()
            .copied()
            .filter(|(name, _)| pool.contains_key(name))
            .collect();
        // A cached entry measured at a different version is refetched: that is
        // the only case where the language mix can have moved.
        let want: Vec<(String, String)> = present
            .iter()
            .filter_map(|(name, _)| {
                let version = &pool.get(name)?.version;
                match sloc.get(name) {
                    Some(hit) if hit.version == *version => {
                        reused += 1;
                        None
                    }
                    _ => Some((name.clone(), version.clone())),
                }
            })
            .collect();
        queried += want.len();
        for (name, result) in fetch_sloc(&want) {
            match result {
                Ok(entry) => {
                    sloc.insert(name, entry);
                }
                Err(e) => {
                    failed += 1;
                    eprintln!("rank: skipping {name}: {e:#}");
                }
            }
        }
        // Consumed in popcon order over the WHOLE batch, so the skip counts
        // describe exactly the prefix of the list the walk actually reached
        // rather than the batch it happened to be reading when it hit k.
        for (name, inst) in batch.iter().copied() {
            let Some(p) = pool.get(name) else {
                not_in_sid += 1;
                continue;
            };
            match sloc.get(name) {
                Some(entry) if keep(entry) => {}
                Some(_) => {
                    wrong_lang += 1;
                    continue;
                }
                None => continue, // lookup failed; already reported
            }
            out.push(RankedCrate {
                rank: out.len() + 1,
                name: name.clone(),
                version: p.version.clone(),
                downloads: *inst,
            });
            index.insert(name.clone(), p.clone());
            if out.len() >= k {
                break 'outer;
            }
        }
        eprintln!("rank: {} of {k} {what} sources kept ({} looked up so far)", out.len(), queried);
    }
    // The cache is written even on a short or failed run: lookups already paid
    // for should not be paid for twice.
    std::fs::write(db.join("sloc.json"), serde_json::to_string(&sloc)?)?;
    eprintln!(
        "rank: sloc cache {cached_at_start} -> {} entries \
         ({queried} looked up, {reused} reused from cache, {failed} failed)",
        sloc.len()
    );
    if out.is_empty() {
        bail!("debian rank list came out empty");
    }
    // Written where `resolve()` will look for it rather than under `db`, so
    // that the two cannot disagree when `--db` is given a different path.
    let index_out = std::path::PathBuf::from(index_path(lang));
    std::fs::create_dir_all(index_out.parent().unwrap())?;
    std::fs::write(&index_out, serde_json::to_string_pretty(&index)?)?;
    eprintln!(
        "rank: kept {} {what} sources; skipped {wrong_lang} without enough {what} \
         and {not_in_sid} not in {SUITE}",
        out.len()
    );
    Ok(out)
}

/// Newest stanza per source package in `dists/<suite>/main/source/Sources`,
/// cached under `db/`. 16 MB gzipped, one download.
fn load_sources(db: &Path) -> Result<HashMap<String, Pool>> {
    let cached = db.join("Sources.gz");
    // The index carries every package's VERSION, so a permanently cached copy
    // freezes the corpus: `resolve()` would keep returning the same tarballs,
    // the sweep cache would skip them all, and the "new version of a top-K
    // package" event this whole loop is built around could never fire.
    // Refreshed on any run older than the daily cron's period.
    let stale = match std::fs::metadata(&cached).and_then(|m| m.modified()) {
        Ok(modified) => modified
            .elapsed()
            .map(|age| age > std::time::Duration::from_secs(SOURCES_MAX_AGE_HOURS * 3600))
            .unwrap_or(true),
        Err(_) => true,
    };
    let forced = std::env::var_os("TREEBANK_REFRESH_SOURCES").is_some();
    if !cached.exists() || stale || forced {
        if cached.exists() {
            eprintln!("rank: {SUITE} index is stale (>{SOURCES_MAX_AGE_HOURS}h) — refreshing");
        }
        eprintln!("rank: GET {SOURCES}");
        let mut body = Vec::new();
        ureq::get(SOURCES)
            .call()
            .with_context(|| format!("GET {SOURCES}"))?
            .into_reader()
            .read_to_end(&mut body)?;
        std::fs::write(&cached, &body)?;
    }
    let gz = flate2::read::GzDecoder::new(std::fs::File::open(&cached)?);
    let mut pool: HashMap<String, Pool> = HashMap::new();
    let (mut pkg, mut version, mut directory) = (None, None, None);
    let mut origs: Vec<String> = Vec::new();
    let finish = |pkg: &mut Option<String>,
                  version: &mut Option<String>,
                  directory: &mut Option<String>,
                  origs: &mut Vec<String>,
                  pool: &mut HashMap<String, Pool>| {
        if let (Some(n), Some(v), Some(d)) = (pkg.take(), version.take(), directory.take()) {
            // Shortest name wins for multi-tarball sources: that is the main
            // one, the rest are `.orig-<component>.tar.*` add-ons.
            origs.sort_by_key(|f| (f.len(), f.clone()));
            if let Some(file) = origs.first().cloned() {
                let newer = pool
                    .get(&n)
                    .is_none_or(|old: &Pool| deb_version_lt(&old.version, &v));
                if newer {
                    pool.insert(n, Pool { version: v, directory: d, file });
                }
            }
        }
        origs.clear();
    };
    for line in BufReader::new(gz).lines() {
        let line = line?;
        if let Some(v) = line.strip_prefix("Package: ") {
            pkg = Some(v.trim().to_string());
        } else if let Some(v) = line.strip_prefix("Version: ") {
            version = Some(v.trim().to_string());
        } else if let Some(v) = line.strip_prefix("Directory: ") {
            directory = Some(v.trim().to_string());
        } else if line.starts_with(' ') && line.contains(".orig.tar.") {
            if let Some(f) = line.split_whitespace().last() {
                if f.ends_with(".tar.xz") || f.ends_with(".tar.gz") || f.ends_with(".tar.bz2") {
                    origs.push(f.to_string());
                }
            }
        } else if line.is_empty() {
            finish(&mut pkg, &mut version, &mut directory, &mut origs, &mut pool);
        }
    }
    finish(&mut pkg, &mut version, &mut directory, &mut origs, &mut pool);
    if pool.is_empty() {
        bail!("parsed no sources out of {}", cached.display());
    }
    Ok(pool)
}

/// Numeric-chunk comparison. Not dpkg's algorithm (no epoch/tilde
/// subtleties); it only has to pick the newest of a package's own stanzas.
fn deb_version_lt(a: &str, b: &str) -> bool {
    fn chunks(s: &str) -> Vec<(u64, String)> {
        let mut out = Vec::new();
        let mut num = String::new();
        let mut txt = String::new();
        for ch in s.chars() {
            if ch.is_ascii_digit() {
                if !txt.is_empty() {
                    out.push((0, std::mem::take(&mut txt)));
                }
                num.push(ch);
            } else {
                if !num.is_empty() {
                    out.push((num.parse().unwrap_or(0), String::new()));
                    num.clear();
                }
                txt.push(ch);
            }
        }
        if !num.is_empty() {
            out.push((num.parse().unwrap_or(0), String::new()));
        }
        if !txt.is_empty() {
            out.push((0, txt));
        }
        out
    }
    chunks(a) < chunks(b)
}

/// The language census for every package we have ever asked about, keyed by
/// name and stamped with the version measured.
///
/// This exists because `daily.sh` re-ranks every day: without it, a run at the
/// default `TREEBANK_RANK_K=1000` makes ~1,250 sequential requests to
/// sources.debian.org for facts that change only when a package does. With it,
/// the daily run queries exactly the packages whose version moved since
/// yesterday — which is precisely the set whose language mix could have
/// changed — and reuses the rest.
fn load_sloc_cache(db: &Path) -> HashMap<String, Sloc> {
    std::fs::read_to_string(db.join("sloc.json"))
        .ok()
        .and_then(|s| serde_json::from_str(&s).ok())
        .unwrap_or_default()
}

/// Look up every miss in `want` at once. Concurrency is bounded and modest:
/// this is someone else's public API, and the cache means a healthy run makes
/// almost no requests at all.
fn fetch_sloc(want: &[(String, String)]) -> Vec<(String, Result<Sloc>)> {
    use rayon::prelude::*;
    let pool = rayon::ThreadPoolBuilder::new().num_threads(8).build();
    let run = || {
        want.par_iter()
            .map(|(name, version)| (name.clone(), sloc_of(name, version)))
            .collect::<Vec<_>>()
    };
    match pool {
        Ok(p) => p.install(run),
        Err(_) => run(),
    }
}

/// What is this source package actually written in? popcon ranks everything
/// Debian ships, so without this the top of the list spends its downloads on
/// packages with none of the language in them at all.
///
/// **sources.debian.org lags the archive.** The Sources index is refreshed
/// daily, so on any day Debian has just accepted an upload the newest version
/// is in the index but not yet indexed for SLOC — measured the first time this
/// ran after a refresh, glibc 2.43-3 and mesa 26.1.6-1 both had no info. A
/// package silently dropped for that reason would take the two largest C
/// sources in the corpus with it, so a failure falls back to the newest
/// version sources.debian.org actually holds. A warm cache also covers this
/// (the previous entry survives a failed lookup), which is why the fallback
/// matters most on a cold start — a fresh machine has no cache at all.
fn sloc_of(name: &str, version: &str) -> Result<Sloc> {
    match sloc_at(name, version) {
        Ok(s) => Ok(s),
        Err(first) => {
            for fallback in indexed_versions(name)?.into_iter().take(3) {
                if fallback == version {
                    continue;
                }
                if let Ok(mut s) = sloc_at(name, &fallback) {
                    eprintln!(
                        "rank: {name} {version} not yet indexed by sources.debian.org — \
                         measured {fallback} instead"
                    );
                    // Stamped with what was actually measured, so the next run
                    // re-queries once the archive version is indexed.
                    s.version = fallback;
                    return Ok(s);
                }
            }
            Err(first)
        }
    }
}

/// Versions sources.debian.org holds for a package, newest first as the API
/// returns them.
fn indexed_versions(name: &str) -> Result<Vec<String>> {
    let url = format!("https://sources.debian.org/api/src/{name}/");
    let doc: serde_json::Value = ureq::get(&url)
        .call()
        .with_context(|| format!("GET {url}"))?
        .into_json()?;
    Ok(doc["versions"]
        .as_array()
        .map(|vs| {
            vs.iter()
                .filter_map(|v| v["version"].as_str().map(str::to_string))
                .collect()
        })
        .unwrap_or_default())
}

fn sloc_at(name: &str, version: &str) -> Result<Sloc> {
    let url = format!("https://sources.debian.org/api/info/package/{name}/{version}/");
    let doc: serde_json::Value = ureq::get(&url)
        .call()
        .with_context(|| format!("GET {url}"))?
        .into_json()?;
    let sloc = doc["pkg_infos"]["sloc"]
        .as_array()
        .with_context(|| format!("{name} {version}: no sloc in sources.debian.org info"))?;
    let mut langs = BTreeMap::new();
    for pair in sloc {
        if let (Some(lang), Some(n)) = (pair[0].as_str(), pair[1].as_i64()) {
            langs.insert(lang.to_string(), n);
        }
    }
    Ok(Sloc { version: version.to_string(), langs })
}
