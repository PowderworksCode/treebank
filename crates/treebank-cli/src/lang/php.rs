use std::collections::HashMap;
use std::path::Path;
use std::process::Command;

use anyhow::{bail, Context, Result};

use super::{exec_oracle, Lang};
use crate::ledger::LangName;
use crate::rank::RankedCrate;

pub struct Php;

/// The lowest PHP the oracle will run on, as `PHP_VERSION_ID`.
///
/// This is a hard floor rather than a preference, and it was measured. On
/// 1703 files from the top 40 Packagist packages, PHP 8.3 rejects 7 — every
/// one of them current Symfony, which declares `"php": ">=8.4.1"` and uses
/// property hooks (`public ParameterBag $attributes { set { … } }`),
/// asymmetric visibility (`public public(set) readonly array $variables`)
/// and `new` without parentheses (`new ReflectionClass(…)->getAttributes()`).
/// PHP 8.4 and 8.5 accept all 1703.
///
/// Those 7 would be recorded as corpus noise, so any grammar gap in PHP 8.4
/// syntax would be silently discarded on the most-downloaded package family
/// in the ecosystem — the sweep would under-report exactly where the
/// language is moving fastest. Refusing to run is the honest response;
/// answering with an interpreter that is behind the corpus is not.
const MIN_VERSION_ID: u32 = 80400;

impl Lang for Php {
    fn name(&self) -> LangName {
        LangName::Php
    }

    fn rank(&self, _db: &Path, k: usize) -> Result<Vec<RankedCrate>> {
        rank_packagist(k)
    }

    /// Packagist's p2 metadata for a package lists its versions newest
    /// first, so entry 0 is the current release, and each carries the `dist`
    /// archive Composer itself would download — the published source tree,
    /// not a build artifact, so there is no sdist/wheel choice to make here
    /// the way there is for PyPI.
    ///
    /// The one rewrite: Packagist states the dist URL as
    /// `api.github.com/repos/<o>/<r>/zipball/<ref>`, and the GitHub API
    /// allows 60 unauthenticated requests an hour, which a 500-package fetch
    /// exhausts in the first minute. `codeload.github.com` serves the same
    /// commit with no token and no such limit, and it serves `tar.gz`, which
    /// arrives with the single top-level directory the tar path in `fetch`
    /// already strips. Measured over a sample of the top 500, every dist URL
    /// was on `api.github.com`; anything else is passed through untouched.
    fn resolve(&self, pkg: &RankedCrate) -> Result<(String, String)> {
        let url = format!("https://repo.packagist.org/p2/{}.json", pkg.name);
        let doc: serde_json::Value = ureq::get(&url)
            .call()
            .with_context(|| format!("GET {url}"))?
            .into_json()?;
        let releases = doc["packages"][&pkg.name]
            .as_array()
            .with_context(|| format!("{}: no packages.{} array", pkg.name, pkg.name))?;
        let latest = releases
            .first()
            .with_context(|| format!("{}: no releases", pkg.name))?;
        let version = latest["version"]
            .as_str()
            .with_context(|| format!("{}: release has no version", pkg.name))?
            .to_string();
        let dist = latest["dist"]["url"]
            .as_str()
            .with_context(|| format!("{} {version}: publishes no dist archive", pkg.name))?;
        Ok((version, codeload_url(dist)))
    }

    /// `.php` only — the single extension tree-sitter-php's
    /// `tree-sitter.json` claims for the grammar we load, following the same
    /// rule as python and javascript. `.phtml`, `.php4` and friends are also
    /// this syntax and this grammar would parse them; leaving them out keeps
    /// `classify()` matching what the grammar advertises, so widening it
    /// stays a deliberate change with its own sweep evidence.
    ///
    /// `vendor/` is excluded for the reason javascript excludes bundles and
    /// python excludes `_vendor/`: it is Composer's install directory, so a
    /// failure inside it is attributed to the wrong package, and the same
    /// code is already in the corpus under the package that really owns it.
    fn classify(&self, rel: &Path) -> Option<Option<String>> {
        if rel
            .components()
            .any(|c| c.as_os_str().to_str() == Some("vendor"))
        {
            return None;
        }
        (rel.extension()?.to_str()? == "php").then_some(None)
    }

    /// Only the `php` grammar, not `php_only`.
    ///
    /// Upstream ships both from one shared `common/define-grammar.js`. They
    /// are not dialects of the corpus the way `typescript` and `tsx` are:
    /// `php` parses a file as PHP has always defined one — interleaved text
    /// and `<?php … ?>` regions, where a file with no open tag at all is
    /// simply inline output — while `php_only` parses a region that is
    /// already known to be code, which is what an editor injects into a
    /// host language. `tree-sitter.json` says as much: `php` claims
    /// `file-types: ["php"]` and `php_only` claims none. Corpus files are
    /// whole `.php` files, so `php` is the only grammar that can route them.
    /// Both are still generated (see `generate_dirs`), because both ship in
    /// the published crate.
    fn grammar_dirs(&self) -> &'static [&'static str] {
        &["php"]
    }

    /// `php -l`, the reference parser's own syntax-check mode.
    ///
    /// Verified parse-only: a file whose top level writes to disk and echoes
    /// produced no side effect, no output and exit 0 under `-l`, and a
    /// missing `require`, an undefined base class and an undefined interface
    /// are all accepted — so, like every other Tier-A oracle here, each file
    /// is judged entirely on its own text with no project context.
    ///
    /// `-n` ignores `php.ini`. That is for determinism first: the box's ini
    /// must not be able to move a verdict, and `short_open_tag` genuinely
    /// can — with it on, `<? function f( { ?>` is a parse error; with it off
    /// the same bytes are inline HTML and the file is valid. It is pinned
    /// explicitly to the language default rather than merely left unset, so
    /// a future change to that default cannot move the numbers either.
    /// Measured bonus: `-n` also skips loading the ini's extensions, which
    /// is 35% of the per-file cost (0.94 → 0.61 ms/file), with zero verdict
    /// changes across 1703 corpus files.
    ///
    /// 255 is the status `php -l` uses for a syntax error, and it is passed
    /// explicitly because 1 means *could not open input file* — see
    /// `exec_oracle::run`, which treats any other status as an oracle
    /// failure rather than as a verdict.
    fn validate(&self, srcroot: &Path, paths: &[String]) -> Result<HashMap<String, bool>> {
        let php = oracle_php()?;
        exec_oracle::run(
            &php,
            &["-n", "-d", "short_open_tag=0", "-l"],
            255,
            &format!("spawn {php} -l — is PHP installed?"),
            srcroot,
            paths,
        )
    }
}

/// Rewrite a GitHub API zipball URL to its unauthenticated codeload
/// equivalent; pass anything else through.
fn codeload_url(dist: &str) -> String {
    let Some(rest) = dist.strip_prefix("https://api.github.com/repos/") else {
        return dist.to_string();
    };
    let mut parts = rest.splitn(4, '/');
    match (parts.next(), parts.next(), parts.next(), parts.next()) {
        (Some(owner), Some(repo), Some("zipball"), Some(reference)) => {
            format!("https://codeload.github.com/{owner}/{repo}/tar.gz/{reference}")
        }
        _ => dist.to_string(),
    }
}

/// Pick the PHP the oracle runs, and refuse one that is too old to answer
/// honestly.
///
/// `TREEBANK_PHP` wins if set; otherwise the first of `php8.5`, `php8.4`,
/// `php` that exists and is new enough. Distributions install versioned
/// binaries side by side (`/usr/bin/php8.5`), so preferring those finds the
/// right interpreter without disturbing whatever `php` points at.
fn oracle_php() -> Result<String> {
    let mut tried = Vec::new();
    let candidates: Vec<String> = match std::env::var("TREEBANK_PHP") {
        Ok(p) if !p.is_empty() => vec![p],
        _ => ["php8.5", "php8.4", "php"].iter().map(|s| s.to_string()).collect(),
    };
    for candidate in candidates {
        match version_id(&candidate) {
            Ok(id) if id >= MIN_VERSION_ID => return Ok(candidate),
            Ok(id) => tried.push(format!("{candidate} is {}", human(id))),
            Err(_) => tried.push(format!("{candidate} not found")),
        }
    }
    bail!(
        "no PHP new enough for the oracle: need {} or later, but {}.\n\
         Install one (e.g. `php8.5-cli` from ppa:ondrej/php) or point \
         TREEBANK_PHP at it. Running an older PHP would score valid 8.4+ \
         code as invalid, which records real grammar gaps as corpus noise \
         — see the note on MIN_VERSION_ID.",
        human(MIN_VERSION_ID),
        tried.join(", ")
    )
}

fn version_id(program: &str) -> Result<u32> {
    let out = Command::new(program)
        .args(["-n", "-r", "echo PHP_VERSION_ID;"])
        .output()
        .with_context(|| format!("spawn {program}"))?;
    anyhow::ensure!(out.status.success(), "{program} -r failed");
    Ok(String::from_utf8_lossy(&out.stdout).trim().parse()?)
}

fn human(id: u32) -> String {
    format!("{}.{}.{}", id / 10000, (id / 100) % 100, id % 100)
}

/// Packagist's own popularity ranking, which is download traffic — the same
/// KIND of metric as crates.io and npm downloads, unlike Java's
/// dependent-repos proxy. `explore/popular.json` caps `per_page` at 100, so
/// a top-K list is K/100 requests.
fn rank_packagist(k: usize) -> Result<Vec<RankedCrate>> {
    let mut ranked: Vec<RankedCrate> = Vec::new();
    let mut seen = std::collections::HashSet::new();
    for page in 1.. {
        if ranked.len() >= k {
            break;
        }
        let url =
            format!("https://packagist.org/explore/popular.json?per_page=100&page={page}");
        let doc: serde_json::Value = ureq::get(&url)
            .call()
            .with_context(|| format!("GET {url}"))?
            .into_json()?;
        let packages = doc["packages"]
            .as_array()
            .with_context(|| format!("packagist page {page}: no packages array"))?;
        if packages.is_empty() {
            break;
        }
        for p in packages {
            let (Some(name), Some(downloads)) =
                (p["name"].as_str(), p["downloads"].as_u64())
            else {
                continue;
            };
            // Packagist paginates a live ranking, so a package can move
            // across a page boundary between requests and appear twice.
            if !seen.insert(name.to_string()) {
                continue;
            }
            ranked.push(RankedCrate {
                rank: ranked.len() + 1,
                name: name.to_string(),
                // Resolved at fetch time from the p2 metadata, like java.
                version: String::new(),
                downloads,
            });
            if ranked.len() == k {
                break;
            }
        }
    }
    if ranked.is_empty() {
        bail!("packagist rank list came out empty");
    }
    Ok(ranked)
}
