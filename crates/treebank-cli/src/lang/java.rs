use std::collections::HashMap;
use std::io::Write;
use std::path::Path;
use std::process::{Command, Stdio};

use anyhow::{bail, Context, Result};

use super::Lang;
use crate::ledger::LangName;
use crate::rank::RankedCrate;

pub struct Java;

pub(super) const CENTRAL: &str = "https://repo1.maven.org/maven2";

impl Lang for Java {
    fn name(&self) -> LangName {
        LangName::Java
    }

    fn rank(&self, _db: &Path, k: usize) -> Result<Vec<RankedCrate>> {
        rank_maven(k)
    }

    /// Maven Central's own metadata for the current release, then the
    /// convention-named sources jar. Artifacts that publish no sources jar
    /// (pom-only aggregators, a few relocated coordinates) 404 here and the
    /// fetch driver skips them.
    fn resolve(&self, pkg: &RankedCrate) -> Result<(String, String)> {
        let (group, artifact) = pkg
            .name
            .split_once(':')
            .with_context(|| format!("{}: not a group:artifact coordinate", pkg.name))?;
        let path = format!("{}/{}", group.replace('.', "/"), artifact);
        let url = format!("{CENTRAL}/{path}/maven-metadata.xml");
        let xml = ureq::get(&url)
            .call()
            .with_context(|| format!("GET {url}"))?
            .into_string()?;
        // <release> is the newest non-snapshot; <latest> can be a snapshot.
        let version = tag(&xml, "release")
            .or_else(|| tag(&xml, "latest"))
            .with_context(|| format!("{}: no release version in maven-metadata.xml", pkg.name))?;
        let jar = format!("{CENTRAL}/{path}/{version}/{artifact}-{version}-sources.jar");
        Ok((version, jar))
    }

    fn classify(&self, rel: &Path) -> Option<Option<String>> {
        (rel.extension()?.to_str()? == "java").then_some(None)
    }

    fn grammar_dirs(&self) -> &'static [&'static str] {
        &["."]
    }

    /// tools/java-oracle: javac's own parser via JavacTask.parse(), run
    /// through the JDK's single-file source launcher. Parse-only, so
    /// unresolved imports are not errors and a file is judged on its own.
    fn validate(&self, srcroot: &Path, paths: &[String]) -> Result<HashMap<String, bool>> {
        let script = Path::new("tools/java-oracle/Check.java");
        anyhow::ensure!(
            script.exists(),
            "java oracle missing at {} (run from the repo root)",
            script.display()
        );
        let mut child = Command::new("java")
            .arg(script)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .spawn()
            .context("spawn `java tools/java-oracle/Check.java` — is a JDK installed?")?;

        let mut stdin = child.stdin.take().context("oracle stdin")?;
        let lines: Vec<String> = paths
            .iter()
            .map(|p| srcroot.join(p).display().to_string())
            .collect();
        let writer = std::thread::spawn(move || -> std::io::Result<()> {
            for line in &lines {
                writeln!(stdin, "{line}")?;
            }
            stdin.flush()
        });
        let output = child.wait_with_output()?;
        let _ = writer.join().map_err(|_| anyhow::anyhow!("oracle stdin thread panicked"))?;
        anyhow::ensure!(
            output.status.success(),
            "java-oracle failed: {}",
            String::from_utf8_lossy(&output.stderr)
        );

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

/// First `<tag>…</tag>` body in an XML document.
pub(super) fn tag(xml: &str, name: &str) -> Option<String> {
    let open = format!("<{name}>");
    let close = format!("</{name}>");
    let start = xml.find(&open)? + open.len();
    let end = start + xml[start..].find(&close)?;
    Some(xml[start..end].trim().to_string())
}

/// Maven Central publishes no download counts, so "popular" has to come from
/// somewhere else: ecosyste.ms indexes the registry and exposes how many
/// public repositories depend on each artifact. That is a different metric
/// from crates.io/npm downloads — a dependency-graph proxy, not traffic —
/// and the ledger says so.
fn rank_maven(k: usize) -> Result<Vec<RankedCrate>> {
    const PER_PAGE: usize = 100;
    let mut ranked = Vec::new();
    let mut page = 1;
    while ranked.len() < k {
        let url = format!(
            "https://packages.ecosyste.ms/api/v1/registries/repo1.maven.org/packages\
             ?sort=dependent_repos_count&order=desc&per_page={PER_PAGE}&page={page}"
        );
        let batch: Vec<serde_json::Value> = ureq::get(&url)
            .call()
            .with_context(|| format!("GET {url}"))?
            .into_json()?;
        if batch.is_empty() {
            break;
        }
        eprintln!("rank: ecosyste.ms maven page {page} ({} artifacts)", batch.len());
        for entry in batch {
            let (Some(name), Some(dependents)) = (
                entry["name"].as_str(),
                entry["dependent_repos_count"].as_u64(),
            ) else {
                continue;
            };
            if !name.contains(':') {
                continue;
            }
            ranked.push(RankedCrate {
                rank: ranked.len() + 1,
                name: name.to_string(),
                version: String::new(), // resolved at fetch time from Central
                downloads: dependents,
            });
            if ranked.len() == k {
                break;
            }
        }
        page += 1;
    }
    if ranked.is_empty() {
        bail!("maven rank list came out empty");
    }
    Ok(ranked)
}
