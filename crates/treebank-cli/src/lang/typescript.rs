use std::collections::HashMap;
use std::io::Read;
use std::path::Path;

use anyhow::{bail, Context, Result};

use super::Lang;
use crate::rank::RankedCrate;

pub struct TypeScript;

impl Lang for TypeScript {
    fn name(&self) -> &'static str {
        "typescript"
    }

    /// npm has no public "top N" endpoint; the npm-high-impact package
    /// (wooorm) tracks the top packages by downloads and ships them as a
    /// data array. We pull its latest tarball and read lib/top.js.
    fn rank(&self, _db: &Path, k: usize) -> Result<Vec<RankedCrate>> {
        rank_npm(k)
    }

    /// Latest version + tarball url from the registry (abbreviated metadata).
    fn resolve(&self, pkg: &RankedCrate) -> Result<(String, String)> {
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

    fn classify(&self, rel: &Path) -> Option<Option<String>> {
        match rel.extension()?.to_str()? {
            "tsx" => Some(Some("tsx".into())),
            "ts" | "mts" | "cts" => Some(None),
            _ => None,
        }
    }

    fn grammar_dirs(&self) -> &'static [&'static str] {
        &["typescript", "tsx"]
    }

    fn route(&self, dialect: &Option<String>, rel: &str) -> usize {
        let is_tsx = dialect
            .as_deref()
            .map(|d| d == "tsx")
            .unwrap_or_else(|| rel.ends_with(".tsx"));
        usize::from(is_tsx)
    }

    /// tools/ts-oracle: ts.createSourceFile parseDiagnostics — syntax-only,
    /// and .d.ts-safe (ts.transpileModule throws on declaration files). One
    /// node process per batch; stdout is "path\tvalid|invalid" lines.
    fn validate(&self, srcroot: &Path, paths: &[String]) -> Result<HashMap<String, bool>> {
        use std::io::Write;
        let tool = Path::new("tools/ts-oracle");
        if !tool.join("node_modules").exists() {
            eprintln!("oracle: installing tools/ts-oracle deps (npm ci)");
            let ok = std::process::Command::new("npm")
                .args(["ci", "--no-audit", "--no-fund"])
                .current_dir(tool)
                .status()?
                .success();
            anyhow::ensure!(ok, "npm ci failed in tools/ts-oracle");
        }
        let mut child = std::process::Command::new("node")
            .arg(tool.join("check.mjs"))
            .stdin(std::process::Stdio::piped())
            .stdout(std::process::Stdio::piped())
            .spawn()
            .context("spawn node tools/ts-oracle/check.mjs")?;
        {
            let stdin = child.stdin.as_mut().unwrap();
            for p in paths {
                writeln!(stdin, "{}", srcroot.join(p).display())?;
            }
        }
        let output = child.wait_with_output()?;
        anyhow::ensure!(output.status.success(), "ts-oracle failed");
        let mut map = HashMap::new();
        for line in String::from_utf8_lossy(&output.stdout).lines() {
            if let Some((path, verdict)) = line.rsplit_once('\t') {
                let rel = Path::new(path)
                    .strip_prefix(srcroot)
                    .map(|r| r.to_string_lossy().into_owned())
                    .unwrap_or_else(|_| path.to_string());
                map.insert(rel, verdict == "valid");
            }
        }
        Ok(map)
    }
}

fn rank_npm(k: usize) -> Result<Vec<RankedCrate>> {
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
    ureq::get(tarball_url).call()?.into_reader().read_to_end(&mut buf)?;

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
