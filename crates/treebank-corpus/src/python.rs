use std::path::Path;

use anyhow::{bail, Context, Result};

use crate::rank::RankedCrate;
use crate::{Ecosystem, LangName};

pub struct Python;

impl Ecosystem for Python {
    fn name(&self) -> LangName {
        LangName::Python
    }

    fn rank(&self, _db: &Path, k: usize) -> Result<Vec<RankedCrate>> {
        rank_pypi(k)
    }

    /// PyPI's JSON API for the current release, then its **source**
    /// distribution. Wheels are deliberately not used as a fallback: a wheel
    /// is build output, and for anything with a build step it is not the
    /// tree the author wrote. Packages that publish no sdist 404 here and
    /// the fetch driver skips them, exactly as Java skips artifacts with no
    /// sources jar.
    fn resolve(&self, pkg: &RankedCrate) -> Result<(String, String)> {
        let url = format!("https://pypi.org/pypi/{}/json", pkg.name);
        let doc: serde_json::Value = ureq::get(&url)
            .call()
            .with_context(|| format!("GET {url}"))?
            .into_json()?;
        let version = doc["info"]["version"]
            .as_str()
            .with_context(|| format!("{}: no info.version", pkg.name))?
            .to_string();
        let sdist = doc["urls"]
            .as_array()
            .with_context(|| format!("{}: no urls array", pkg.name))?
            .iter()
            .find(|u| u["packagetype"] == "sdist")
            .and_then(|u| u["url"].as_str())
            .with_context(|| format!("{} {version}: publishes no sdist", pkg.name))?
            .to_string();
        Ok((version, sdist))
    }

    /// `.py` only — the single extension tree-sitter-python's
    /// tree-sitter.json claims, following the same rule as javascript.
    /// `.pyi` stubs are also Python syntax and this same grammar parses
    /// them; they are left out for now so `classify()` matches what the
    /// grammar advertises, and adding them is a deliberate change with its
    /// own sweep evidence rather than a silent widening.
    ///
    /// `_vendor/` trees are excluded for the reason javascript excludes
    /// bundles: pip and friends vendor entire dependency sets, so a failure
    /// there is attributed to the wrong package, and the same code is
    /// already in the corpus under the package that really owns it.
    fn classify(&self, rel: &Path) -> Option<Option<String>> {
        if rel
            .components()
            .any(|c| matches!(c.as_os_str().to_str(), Some("_vendor") | Some("_vendored")))
        {
            return None;
        }
        (rel.extension()?.to_str()? == "py").then_some(None)
    }

}

/// PyPI publishes no download counts through its own API — the numbers live
/// in the public BigQuery/ClickHouse download dataset, and hugovk's
/// top-pypi-packages is the standard published extract of it. That makes
/// this the same *kind* of metric as crates.io and npm downloads (traffic),
/// unlike Java's dependent-repos proxy. The ledger says so, and records the
/// dataset's own `last_update` at fetch time.
fn rank_pypi(k: usize) -> Result<Vec<RankedCrate>> {
    const URL: &str = "https://hugovk.github.io/top-pypi-packages/top-pypi-packages.min.json";
    let doc: serde_json::Value = ureq::get(URL)
        .call()
        .with_context(|| format!("GET {URL}"))?
        .into_json()?;
    if let Some(updated) = doc["last_update"].as_str() {
        eprintln!("rank: top-pypi-packages dataset, last_update {updated}");
    }
    let rows = doc["rows"]
        .as_array()
        .context("top-pypi-packages: no rows array")?;
    let mut ranked = Vec::new();
    for row in rows {
        let (Some(project), Some(downloads)) =
            (row["project"].as_str(), row["download_count"].as_u64())
        else {
            continue;
        };
        ranked.push(RankedCrate {
            rank: ranked.len() + 1,
            name: project.to_string(),
            // Resolved at fetch time from PyPI, like java.
            version: String::new(),
            downloads,
        });
        if ranked.len() == k {
            break;
        }
    }
    if ranked.is_empty() {
        bail!("pypi rank list came out empty");
    }
    Ok(ranked)
}
