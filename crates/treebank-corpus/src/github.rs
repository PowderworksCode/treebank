//! The other artifact corpus: GitHub repositories.
//!
//! Debian answers "what code does a distribution ship". This answers "what
//! code do people star", and for a language with no registry the two are the
//! only honest sources available. They are **different populations**, and
//! for shell they are unusually far apart — Debian's shell is configure
//! scripts, init scripts, maintainer helpers and test harnesses inside
//! projects written in C; GitHub's shell is dotfile frameworks, installers,
//! and self-contained tools where shell is the whole product. A gap number
//! from one is not a gap number from the other, which is why bash can select
//! between them and reports both.
//!
//! What it biases toward, stated as plainly as the Debian module's:
//!
//! - Ranking is **stars**, which is attention, not use. It is the weakest
//!   popularity metric in this repo — crates.io/npm/PyPI count downloads and
//!   popcon counts machines, while a star costs nothing and never decays.
//!   It is used here because GitHub publishes no download counts and
//!   nothing else ranks a repository.
//! - `language:` is GitHub's own linguist classification of the *repository*,
//!   which is a majority vote over its files. So this selects repositories
//!   that are mostly shell, where the Debian path deliberately selects
//!   packages that merely contain shell.
//! - There is no release identity. A repository has no version, so the
//!   pinned commit sha is used as one: the corpus is reproducible, but
//!   "version" here means "what HEAD was when we fetched", not something
//!   the author released.
//! - Archived and fork repositories are excluded; a fork would enter the
//!   corpus as a near-duplicate of code already in it.

use std::collections::HashMap;
use std::sync::{LazyLock, Mutex};

use anyhow::{bail, Context, Result};
use serde::{Deserialize, Serialize};

use treebank_lang::LangName;
use crate::rank::RankedCrate;

const API: &str = "https://api.github.com";
const PER_PAGE: usize = 100;

/// Where `rank()` leaves the resolved repositories for `resolve()`, which
/// gets no `db` path — the same arrangement as the Debian pool index.
fn index_path(lang: LangName) -> String {
    format!("corpus/{lang}/db/github-index.json")
}

#[derive(Serialize, Deserialize, Clone)]
pub struct Repo {
    /// "owner/name"
    pub full_name: String,
    pub default_branch: String,
    pub stars: u64,
}

static INDEX: LazyLock<Mutex<HashMap<LangName, std::sync::Arc<HashMap<String, Repo>>>>> =
    LazyLock::new(|| Mutex::new(HashMap::new()));

fn index(lang: LangName) -> std::sync::Arc<HashMap<String, Repo>> {
    if let Some(hit) = INDEX.lock().unwrap().get(&lang) {
        return hit.clone();
    }
    let loaded: HashMap<String, Repo> = std::fs::read_to_string(index_path(lang))
        .ok()
        .and_then(|s| serde_json::from_str(&s).ok())
        .unwrap_or_default();
    let arc = std::sync::Arc::new(loaded);
    INDEX.lock().unwrap().insert(lang, arc.clone());
    arc
}

/// GitHub's search API is 10 requests/minute unauthenticated and its core
/// API 60/hour, neither of which will build a 500-repository corpus. The
/// token is read from the environment first and from `gh auth token` second,
/// so a machine with the CLI already logged in needs no setup — the same
/// reasoning as the oracles finding their interpreters on PATH.
fn token() -> Option<String> {
    for var in ["GITHUB_TOKEN", "GH_TOKEN"] {
        if let Ok(t) = std::env::var(var) {
            if !t.trim().is_empty() {
                return Some(t.trim().to_string());
            }
        }
    }
    let out = std::process::Command::new("gh")
        .args(["auth", "token"])
        .output()
        .ok()?;
    let t = String::from_utf8_lossy(&out.stdout).trim().to_string();
    (out.status.success() && !t.is_empty()).then_some(t)
}

/// Timeouts for the same measured reason `fetch::download` carries them, and
/// found the same way: a `resolve()` call to `/repos/{repo}/commits/{branch}`
/// wedged mid-request during the html corpus fetch — socket alive, zero bytes
/// read, no progress for six minutes — and ureq's default agent has no read
/// timeout, so the whole serial fetch stopped behind one API call. The
/// download path was already immune to this; the API path was not.
fn get(url: &str) -> Result<serde_json::Value> {
    let mut req = ureq::AgentBuilder::new()
        .timeout_connect(std::time::Duration::from_secs(30))
        .timeout_read(std::time::Duration::from_secs(60))
        .build()
        .get(url)
        .set("Accept", "application/vnd.github+json")
        .set("User-Agent", "treebank-corpus");
    if let Some(t) = token() {
        req = req.set("Authorization", &format!("Bearer {t}"));
    }
    Ok(req.call().with_context(|| format!("GET {url}"))?.into_json()?)
}

/// The top `k` non-fork, non-archived repositories GitHub classifies as
/// `gh_language`, by stars.
///
/// Paged in blocks of 100 down through the star ranking. GitHub caps any one
/// search at 1000 results, so beyond that the walk re-queries with an upper
/// star bound taken from the last repository seen — the standard way to page
/// past the cap, and it is why the results are read in order rather than by
/// page number.
pub fn rank(lang: LangName, gh_language: &str, k: usize) -> Result<Vec<RankedCrate>> {
    if token().is_none() {
        eprintln!(
            "rank: no GitHub token (GITHUB_TOKEN/GH_TOKEN or `gh auth token`) — \
             the search API allows 10 requests/minute without one"
        );
    }
    let mut out: Vec<RankedCrate> = Vec::new();
    let mut repos: HashMap<String, Repo> = HashMap::new();
    let mut seen: std::collections::HashSet<String> = std::collections::HashSet::new();
    let mut upper: Option<u64> = None;
    'outer: while out.len() < k {
        let stars = match upper {
            Some(u) => format!(" stars:<={u}"),
            None => String::new(),
        };
        let query = format!("language:{gh_language} fork:false archived:false{stars}");
        let mut page = 1;
        let mut added_this_query = 0usize;
        while out.len() < k && page <= 10 {
            let url = format!(
                "{API}/search/repositories?q={}&sort=stars&order=desc&per_page={PER_PAGE}&page={page}",
                urlencode(&query)
            );
            let doc = get(&url)?;
            let items = doc["items"].as_array().cloned().unwrap_or_default();
            if items.is_empty() {
                break 'outer;
            }
            for item in &items {
                let (Some(full_name), Some(branch)) = (
                    item["full_name"].as_str(),
                    item["default_branch"].as_str(),
                ) else {
                    continue;
                };
                let star_count = item["stargazers_count"].as_u64().unwrap_or(0);
                upper = Some(star_count);
                if !seen.insert(full_name.to_string()) {
                    continue;
                }
                added_this_query += 1;
                repos.insert(
                    full_name.to_string(),
                    Repo {
                        full_name: full_name.to_string(),
                        default_branch: branch.to_string(),
                        stars: star_count,
                    },
                );
                out.push(RankedCrate {
                    rank: out.len() + 1,
                    name: full_name.to_string(),
                    // Resolved at fetch time: a repository has no version, so
                    // the sha it is pinned at becomes one.
                    version: String::new(),
                    downloads: star_count,
                });
                if out.len() >= k {
                    break;
                }
            }
            eprintln!("rank: {} of {k} {gh_language} repositories", out.len());
            page += 1;
        }
        // A whole re-query that added nothing means the star bound is not
        // advancing and paging further would loop forever.
        if added_this_query == 0 {
            break;
        }
    }
    if out.is_empty() {
        bail!("github rank list came out empty");
    }
    let index_out = std::path::PathBuf::from(index_path(lang));
    std::fs::create_dir_all(index_out.parent().unwrap())?;
    std::fs::write(&index_out, serde_json::to_string_pretty(&repos)?)?;
    eprintln!("rank: kept {} {gh_language} repositories (>= {} stars)", out.len(), out.last().map(|r| r.downloads).unwrap_or(0));
    Ok(out)
}

/// The default branch's head commit, and the codeload tarball for exactly
/// that sha. Pinning the sha rather than the branch is what makes the corpus
/// reproducible: a branch name resolves to different bytes every day, and
/// the sweep cache keys on (package, version).
pub fn resolve(lang: LangName, pkg: &RankedCrate) -> Result<(String, String)> {
    let idx = index(lang);
    let repo = idx.get(&pkg.name).with_context(|| {
        format!(
            "{}: not in {} — re-run `treebank rank --lang {lang}`",
            pkg.name,
            index_path(lang)
        )
    })?;
    let url = format!(
        "{API}/repos/{}/commits/{}",
        repo.full_name, repo.default_branch
    );
    let doc = get(&url)?;
    let sha = doc["sha"]
        .as_str()
        .with_context(|| format!("{}: no head sha", repo.full_name))?;
    Ok((
        sha[..12.min(sha.len())].to_string(),
        format!("https://codeload.github.com/{}/tar.gz/{sha}", repo.full_name),
    ))
}

/// Percent-encoding for the handful of characters a search query uses.
/// Pulling in a URL crate for `:` and a space would be the larger change.
fn urlencode(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for b in s.bytes() {
        match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                out.push(b as char)
            }
            b' ' => out.push('+'),
            _ => out.push_str(&format!("%{b:02X}")),
        }
    }
    out
}
