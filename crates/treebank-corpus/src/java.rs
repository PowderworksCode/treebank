use std::path::Path;

use anyhow::{bail, Context, Result};

use crate::rank::RankedCrate;
use crate::{Ecosystem, LangName};

pub struct Java;

const CENTRAL: &str = "https://repo1.maven.org/maven2";

impl Ecosystem for Java {
    fn name(&self) -> LangName {
        LangName::Java
    }

    fn rank(&self, _db: &Path, k: usize) -> Result<Vec<RankedCrate>> {
        rank_maven(k)
    }

    /// Maven Central's own metadata for the current release, then the
    /// convention-named sources jar. Artifacts publishing no sources jar
    /// (pom-only aggregators, a few relocated coordinates) 404 here and the
    /// fetch driver skips them — the same shape as PyPI packages with no
    /// sdist.
    ///
    /// A sources jar rather than the ordinary one for the reason python
    /// refuses wheels: the ordinary jar is `.class` files, and even a
    /// sources-bearing build is not the tree the author wrote if it went
    /// through an annotation processor. The sources jar is what was
    /// compiled.
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
        // `<release>` is the newest non-snapshot; `<latest>` can be one.
        let version = tag(&xml, "release")
            .or_else(|| tag(&xml, "latest"))
            .with_context(|| format!("{}: no release version in maven-metadata.xml", pkg.name))?;
        let jar = format!("{CENTRAL}/{path}/{version}/{artifact}-{version}-sources.jar");
        Ok((version, jar))
    }

    fn classify(&self, rel: &Path) -> Option<Option<String>> {
        (rel.extension()?.to_str()? == "java").then_some(None)
    }
}

/// First `<tag>…</tag>` body in an XML document.
fn tag(xml: &str, name: &str) -> Option<String> {
    let open = format!("<{name}>");
    let close = format!("</{name}>");
    let start = xml.find(&open)? + open.len();
    let end = start + xml[start..].find(&close)?;
    Some(xml[start..end].trim().to_string())
}

/// Maven Central publishes no download counts, so "popular" has to come from
/// somewhere else: ecosyste.ms indexes the registry and exposes how many
/// public repositories depend on each artifact.
///
/// That is a different metric from crates.io and npm downloads — a
/// dependency-graph proxy rather than traffic — and the difference is worth
/// stating rather than smoothing over. It over-weights libraries that
/// everything depends on transitively and under-weights applications, which
/// for a *syntax* corpus is the harmless direction: what it selects is
/// library code, and library code is where the language's less common
/// constructs live. The ledger records the metric.
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
            let (Some(name), Some(dependents)) =
                (entry["name"].as_str(), entry["dependent_repos_count"].as_u64())
            else {
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
