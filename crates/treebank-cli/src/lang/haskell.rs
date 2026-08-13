use std::collections::HashMap;
use std::path::Path;

use anyhow::{bail, Context, Result};

use super::{cabal, stdin_oracle, Lang};
use crate::ledger::LangName;
use crate::rank::RankedCrate;

pub struct Haskell;

impl Lang for Haskell {
    fn name(&self) -> LangName {
        LangName::Haskell
    }

    fn rank(&self, _db: &Path, k: usize) -> Result<Vec<RankedCrate>> {
        rank_hackage(k)
    }

    /// Hackage's per-package `preferred` document names the versions the
    /// maintainer has not deprecated, newest first, which is the release a
    /// user installing today would get. The tarball is the author's source:
    /// Hackage has no wheel-equivalent and no build output to confuse it
    /// with, so unlike PyPI there is nothing to choose between.
    fn resolve(&self, pkg: &RankedCrate) -> Result<(String, String)> {
        let url = format!("https://hackage.haskell.org/package/{}/preferred", pkg.name);
        let doc: serde_json::Value = ureq::get(&url)
            .set("Accept", "application/json")
            .call()
            .with_context(|| format!("GET {url}"))?
            .into_json()?;
        let version = doc["normal-version"]
            .as_array()
            .and_then(|v| v.first())
            .and_then(|v| v.as_str())
            .with_context(|| format!("{}: no undeprecated version at {url}", pkg.name))?
            .to_string();
        let name = &pkg.name;
        Ok((
            version.clone(),
            format!("https://hackage.haskell.org/package/{name}-{version}/{name}-{version}.tar.gz"),
        ))
    }

    /// `.hs` only.
    ///
    /// tree-sitter-haskell's tree-sitter.json claims `hs` and `hs-boot`, and
    /// the grammar does parse the latter, but `.hs-boot` files are excluded
    /// for now so `classify()` matches the corpus this grammar's numbers were
    /// measured on; adding them is a deliberate change with its own sweep
    /// evidence rather than a silent widening. The same reasoning python
    /// applies to `.pyi`.
    ///
    /// Two other Haskell-ish extensions are deliberately NOT Haskell:
    ///
    /// - `.lhs` is *literate* Haskell, where code is the minority of the
    ///   file and the rest is prose or LaTeX. It needs unliterating before
    ///   any Haskell parser sees it, GHC included, and it has its own
    ///   tree-sitter grammar (tree-sitter-haskell-literate, which Zed ships
    ///   beside this one). Feeding one to this grammar measures the wrong
    ///   thing.
    /// - `.hsc` is hsc2hs input: Haskell with `#let`, `#peek` and C
    ///   fragments that a code generator turns into Haskell. The generated
    ///   `.hs` is the Haskell; the `.hsc` is a template for it.
    fn classify(&self, rel: &Path) -> Option<Option<String>> {
        (rel.extension()?.to_str()? == "hs").then_some(None)
    }

    /// The package's own `.cabal`, and only that one.
    ///
    /// Depth one is the whole rule and it is load-bearing rather than tidy:
    /// `.cabal` files appear deeper in a corpus as test fixtures, and
    /// cabal2nix — a tool that converts cabal files to nix — ships **357**
    /// of them, 356 being golden-test inputs for itself. A reader that took
    /// the tree would configure the package from another package's manifest.
    fn configuration(&self, rel: &Path) -> bool {
        rel.parent().is_some_and(|p| p.as_os_str().is_empty())
            && rel.extension().is_some_and(|e| e == "cabal")
    }

    fn grammar_dirs(&self) -> &'static [&'static str] {
        &["."]
    }

    /// tools/hs-oracle: GHC's own `parseModule`, in one long-lived process,
    /// **plus the file's package configuration**.
    ///
    /// The second half is what makes this language different from every
    /// oracle before it except C's. GHC's parser is configured by `LANGUAGE`
    /// extensions, and real packages declare them in the `.cabal` file
    /// rather than in the source, so a file that parses inside its package
    /// fails alone: `\case` is a parse error without `LambdaCase`. Measured
    /// on 5,631 files from the top 40 Hackage packages, 575 (10.2%) change
    /// verdict when their package's configuration is applied, all of them
    /// invalid → valid. Judging Haskell files one at a time with no
    /// configuration would book those 575 as corpus noise, and noise is
    /// where a real grammar gap goes to hide.
    ///
    /// The configuration is derived per component and memoized per package;
    /// see `cabal.rs` for why it is not simply unioned over the package.
    fn validate(&self, srcroot: &Path, paths: &[String]) -> Result<HashMap<String, bool>> {
        let oracle = Path::new("tools/hs-oracle/hs-oracle");
        if !oracle.exists() {
            eprintln!("oracle: building tools/hs-oracle (GHC API)");
            let ok = std::process::Command::new("tools/hs-oracle/build.sh")
                .status()
                .context("run tools/hs-oracle/build.sh — run from the repo root")?
                .success();
            anyhow::ensure!(ok, "tools/hs-oracle/build.sh failed");
        }
        stdin_oracle::run_configured(
            &oracle.to_string_lossy(),
            &[],
            "tools/hs-oracle/hs-oracle — is GHC installed? (https://www.haskell.org/ghcup/)",
            srcroot,
            paths,
            |rel| {
                // corpus/<lang>/<package>/<path inside the package>
                let Some((pkgdir, within)) = rel.split_once('/') else { return Vec::new() };
                cabal::for_package(srcroot, pkgdir).flags_for(within)
            },
        )
    }
}

/// Hackage publishes a complete download ranking as an HTML table at
/// `/packages/top`, 18,660 packages in one request — no API key, no
/// pagination, no dataset to reconstruct, which makes it the cheapest
/// ranking source of any language here.
///
/// **The metric is downloads in the last 30 days, not all-time**, and the
/// page does not say so. Verified against the per-package pages, which state
/// both: git-annex reads 1,642 on `/packages/top` and "286,097 total (1,642
/// in the last 30 days)" on its own page. That makes it a different kind of
/// number from crates.io's and LuaRocks' cumulative totals — it favours
/// what is being installed now over what has been installed for a decade,
/// which is the better bias for a corpus meant to reflect current code, but
/// it is a bias and it is invisible in the number.
///
/// One consequence worth stating: the top of this ranking is applications
/// rather than libraries — git-annex, pandoc, hlint, purescript, futhark —
/// because Hackage counts tarball downloads and CI installs the tools it
/// runs. Their code is ordinary Haskell, but they are bigger and less
/// library-shaped than a crates.io top 40.
fn rank_hackage(k: usize) -> Result<Vec<RankedCrate>> {
    let url = "https://hackage.haskell.org/packages/top";
    let body = ureq::get(url)
        .call()
        .with_context(|| format!("GET {url}"))?
        .into_string()?;
    let mut rows: Vec<(u64, String)> = Vec::new();
    for row in body.split("<tr").skip(1) {
        let Some(name) = between(row, "<td><a href=\"/package/", "\">") else { continue };
        let Some(rest) = row.split("</a></td>").nth(1) else { continue };
        let Some(count) = between(rest, "<td>", "</td>") else { continue };
        let Ok(downloads) = count.trim().replace(',', "").parse::<u64>() else { continue };
        rows.push((downloads, name.to_string()));
    }
    if rows.is_empty() {
        bail!("hackage {url} came out empty — has the page markup changed?");
    }
    // The page is already sorted, but sorting locally makes the order this
    // depends on explicit rather than inherited, and settles ties by name so
    // two fetches of the same data rank identically.
    rows.sort_by(|a, b| b.0.cmp(&a.0).then_with(|| a.1.cmp(&b.1)));
    Ok(rows
        .into_iter()
        .take(k)
        .enumerate()
        .map(|(i, (downloads, name))| RankedCrate {
            rank: i + 1,
            name,
            // Resolved at fetch time from the package's preferred versions,
            // like python and lua.
            version: String::new(),
            downloads,
        })
        .collect())
}

fn between<'a>(haystack: &'a str, start: &str, end: &str) -> Option<&'a str> {
    let i = haystack.find(start)? + start.len();
    let rest = &haystack[i..];
    Some(&rest[..rest.find(end)?])
}
