use std::collections::{HashMap, HashSet};
use std::io::{Read, Write};
use std::path::Path;
use std::process::{Command, Stdio};
use std::sync::{LazyLock, Mutex};

use anyhow::{bail, Context, Result};

use super::Lang;
use crate::ledger::LangName;
use crate::rank::RankedCrate;

pub struct CSharp;

/// Source repositories already claimed by a higher-ranked package. NuGet's
/// top packages are dominated by a few monorepos — eleven of the top twenty
/// resolve to dotnet/dotnet — and fetching one repo once per package that
/// ships out of it would download gigabytes to extract the same files.
static SEEN_SOURCES: LazyLock<Mutex<HashSet<String>>> =
    LazyLock::new(|| Mutex::new(HashSet::new()));

impl Lang for CSharp {
    fn name(&self) -> LangName {
        LangName::Csharp
    }

    fn rank(&self, _db: &Path, k: usize) -> Result<Vec<RankedCrate>> {
        rank_nuget(k)
    }

    /// NuGet packages ship compiled assemblies, not source: there is not one
    /// `.cs` file in any of the top twenty. What they do ship is SourceLink
    /// metadata — `<repository url … commit …>` in the nuspec — so the
    /// corpus comes from the git commit the package was built from.
    ///
    /// This is a real departure from the other languages, where the corpus
    /// is the published artifact. Here it is repository source: tests,
    /// samples and build tooling included, and code that never shipped in
    /// the package. The ledger says so.
    fn resolve(&self, pkg: &RankedCrate) -> Result<(String, String)> {
        let id = pkg.name.to_lowercase();
        let version = pkg.version.clone();
        let nupkg = format!(
            "https://api.nuget.org/v3-flatcontainer/{id}/{version}/{id}.{version}.nupkg"
        );
        let mut body = Vec::new();
        ureq::get(&nupkg)
            .call()
            .with_context(|| format!("GET {nupkg}"))?
            .into_reader()
            .read_to_end(&mut body)?;

        let nuspec = read_nuspec(&body)
            .with_context(|| format!("{}: reading .nuspec from the nupkg", pkg.name))?;
        let repo = attr(&nuspec, "repository", "url")
            .with_context(|| format!("{}: nuspec has no <repository url>", pkg.name))?;
        let commit = attr(&nuspec, "repository", "commit")
            .with_context(|| format!("{}: nuspec <repository> has no commit", pkg.name))?;
        let slug = repo
            .trim_end_matches(".git")
            .strip_prefix("https://github.com/")
            .with_context(|| format!("{}: source repo is not on github ({repo})", pkg.name))?
            .to_string();

        let key = format!("{slug}@{commit}");
        if !SEEN_SOURCES.lock().unwrap().insert(key.clone()) {
            bail!("source {key} already fetched for a higher-ranked package");
        }
        Ok((
            version,
            format!("https://codeload.github.com/{slug}/tar.gz/{commit}"),
        ))
    }

    fn classify(&self, rel: &Path) -> Option<Option<String>> {
        (rel.extension()?.to_str()? == "cs").then_some(None)
    }

    fn grammar_dirs(&self) -> &'static [&'static str] {
        &["."]
    }

    /// tools/cs-oracle: Roslyn's own parser (CSharpSyntaxTree.ParseText),
    /// built on first use. Parse-only, so unresolved types are not errors
    /// and each file is judged on its own.
    fn validate(&self, srcroot: &Path, paths: &[String]) -> Result<HashMap<String, bool>> {
        let tool = Path::new("tools/cs-oracle");
        let dll = tool.join("bin/Release/net8.0/cs-oracle.dll");
        if !dll.exists() {
            eprintln!("oracle: building tools/cs-oracle (dotnet build -c Release)");
            let ok = Command::new("dotnet")
                .args(["build", "-c", "Release", "--nologo"])
                .current_dir(tool)
                .status()
                .context("run dotnet build in tools/cs-oracle — is the .NET SDK installed?")?
                .success();
            anyhow::ensure!(ok, "dotnet build failed in tools/cs-oracle");
        }
        let mut child = Command::new("dotnet")
            .arg(&dll)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .spawn()
            .with_context(|| format!("spawn dotnet {}", dll.display()))?;

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
            "cs-oracle failed: {}",
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

/// The single root-level `.nuspec` inside a `.nupkg` (which is a zip).
fn read_nuspec(nupkg: &[u8]) -> Result<String> {
    let mut zip = zip::ZipArchive::new(std::io::Cursor::new(nupkg))?;
    for i in 0..zip.len() {
        let mut entry = zip.by_index(i)?;
        let is_nuspec = entry
            .name()
            .rsplit('/')
            .next()
            .is_some_and(|n| n.ends_with(".nuspec"));
        if is_nuspec && !entry.name().contains('/') {
            let mut text = String::new();
            entry.read_to_string(&mut text)?;
            return Ok(text);
        }
    }
    bail!("no .nuspec at the root of the package")
}

/// Value of `attribute` on the first `<element …>` tag.
fn attr(xml: &str, element: &str, attribute: &str) -> Option<String> {
    let open = format!("<{element}");
    let start = xml.find(&open)?;
    let tag = &xml[start..start + xml[start..].find('>')?];
    let key = format!("{attribute}=\"");
    let at = tag.find(&key)? + key.len();
    let end = at + tag[at..].find('"')?;
    Some(tag[at..end].to_string())
}

/// NuGet's search service ranks an empty query by download count, and
/// returns the count alongside each hit — the closest thing the registry has
/// to crates.io's "top by downloads".
fn rank_nuget(k: usize) -> Result<Vec<RankedCrate>> {
    const PER_PAGE: usize = 100;
    let mut ranked = Vec::new();
    while ranked.len() < k {
        let url = format!(
            "https://azuresearch-usnc.nuget.org/query?q=&skip={}&take={PER_PAGE}&prerelease=false&semVerLevel=2.0.0",
            ranked.len()
        );
        let doc: serde_json::Value = ureq::get(&url)
            .call()
            .with_context(|| format!("GET {url}"))?
            .into_json()?;
        let batch = doc["data"].as_array().context("nuget search: no data array")?;
        if batch.is_empty() {
            break;
        }
        eprintln!("rank: nuget search page at {} ({} hits)", ranked.len(), batch.len());
        for entry in batch {
            let (Some(id), Some(version), Some(downloads)) = (
                entry["id"].as_str(),
                entry["version"].as_str(),
                entry["totalDownloads"].as_u64(),
            ) else {
                continue;
            };
            ranked.push(RankedCrate {
                rank: ranked.len() + 1,
                name: id.to_string(),
                version: version.to_string(),
                downloads,
            });
            if ranked.len() == k {
                break;
            }
        }
    }
    if ranked.is_empty() {
        bail!("nuget rank list came out empty");
    }
    Ok(ranked)
}
