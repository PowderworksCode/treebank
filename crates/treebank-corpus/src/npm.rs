//! Shared npm registry plumbing: the download ranking and version/tarball
//! resolution used by every language whose corpus comes from npm.

use std::io::Read;

use anyhow::{bail, Context, Result};

use crate::rank::RankedCrate;

/// npm has no public "top N" endpoint; the npm-high-impact package (wooorm)
/// tracks the top packages by downloads and ships them as a data array. We
/// pull its latest tarball and read lib/top.js.
pub fn rank(k: usize) -> Result<Vec<RankedCrate>> {
    let doc: serde_json::Value = ureq::get("https://registry.npmjs.org/npm-high-impact")
        .call()
        .context("GET npm-high-impact metadata")?
        .into_json()?;
    let latest = doc["dist-tags"]["latest"]
        .as_str()
        .context("npm-high-impact has no latest tag")?;
    let tarball_url = doc["versions"][latest]["dist"]["tarball"]
        .as_str()
        .context("npm-high-impact has no tarball url")?;
    eprintln!("rank: npm-high-impact {latest}");
    let mut buf = Vec::new();
    ureq::get(tarball_url)
        .call()?
        .into_reader()
        .read_to_end(&mut buf)?;

    // The download-ranked list is `export const top = [...]` in lib/top.js.
    const MARKER: &str = "const top = [";
    let gz = flate2::read::GzDecoder::new(&buf[..]);
    let mut archive = tar::Archive::new(gz);
    let mut data = None;
    for entry in archive.entries()? {
        let mut entry = entry?;
        let path = entry.path()?.to_path_buf();
        if path.ends_with("lib/top.js") {
            let mut text = String::new();
            entry.read_to_string(&mut text)?;
            if text.contains(MARKER) {
                data = Some(text);
                break;
            }
        }
    }
    let text = data.context("top array not found in npm-high-impact package")?;
    let start = text.find(MARKER).unwrap() + MARKER.len();
    let end = start + text[start..].find(']').context("unterminated array")?;
    let mut ranked = Vec::new();
    for (i, name) in text[start..end]
        .split('\'')
        .skip(1)
        .step_by(2)
        .take(k)
        .enumerate()
    {
        ranked.push(RankedCrate {
            rank: i + 1,
            name: name.to_string(),
            version: String::new(), // resolved at fetch time from the registry
            downloads: 0,
        });
    }
    if ranked.is_empty() {
        bail!("npm rank list came out empty");
    }
    Ok(ranked)
}

/// Latest version + tarball url from the registry (abbreviated metadata).
pub fn resolve(pkg: &RankedCrate) -> Result<(String, String)> {
    let url = format!("https://registry.npmjs.org/{}", pkg.name);
    let doc: serde_json::Value = ureq::get(&url)
        .set("Accept", "application/vnd.npm.install-v1+json")
        .call()
        .with_context(|| format!("GET {url}"))?
        .into_json()?;
    let latest = doc["dist-tags"]["latest"]
        .as_str()
        .with_context(|| format!("{}: no latest dist-tag", pkg.name))?
        .to_string();
    let tarball = doc["versions"][&latest]["dist"]["tarball"]
        .as_str()
        .with_context(|| format!("{}@{latest}: no tarball url", pkg.name))?
        .to_string();
    Ok((latest, tarball))
}
