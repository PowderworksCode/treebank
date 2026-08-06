use std::io::Read;
use std::path::{Component, Path, PathBuf};

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::lang::Lang;
use crate::rank::RankedCrate;

#[derive(Serialize, Deserialize)]
pub struct ManifestFile {
    pub path: String,
    pub bytes: u64,
    pub sha256: String,
    /// Grammar routing hint (e.g. "tsx"); absent means the language default.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub dialect: Option<String>,
}

#[derive(Serialize, Deserialize)]
pub struct ManifestPackage {
    pub package: String,
    pub version: String,
    pub downloads: u64,
    pub files: Vec<ManifestFile>,
}

#[derive(Serialize, Deserialize)]
pub struct Manifest {
    pub packages: Vec<ManifestPackage>,
}

impl Manifest {
    pub fn load(path: &Path) -> Result<Manifest> {
        Ok(serde_json::from_str(&std::fs::read_to_string(path)?)?)
    }

    /// One entry per corpus file.
    pub fn files(&self) -> Vec<FileEntry> {
        let mut out = Vec::new();
        for p in &self.packages {
            let dir = pkg_dir(&p.package, &p.version);
            for f in &p.files {
                out.push(FileEntry {
                    pkgdir: dir.clone(),
                    rel: f.path.clone(),
                    dialect: f.dialect.clone(),
                    sha256: f.sha256.clone(),
                });
            }
        }
        out
    }
}

pub struct FileEntry {
    pub pkgdir: String,
    pub rel: String,
    pub dialect: Option<String>,
    pub sha256: String,
}

/// Directory name for a package (npm scopes contain '/').
pub fn pkg_dir(name: &str, version: &str) -> String {
    format!("{}-{}", name.replace('/', "__"), version)
}

/// Strip the leading archive component and reject path traversal.
fn safe_rel_path(entry_path: &Path) -> Option<PathBuf> {
    let mut comps = entry_path.components();
    comps.next()?;
    let rel: PathBuf = comps.as_path().to_path_buf();
    if rel.as_os_str().is_empty()
        || rel
            .components()
            .any(|c| !matches!(c, Component::Normal(_)))
    {
        return None;
    }
    Some(rel)
}

fn download(url: &str) -> Result<Vec<u8>> {
    let resp = ureq::get(url).call().with_context(|| format!("GET {url}"))?;
    let mut buf = Vec::new();
    resp.into_reader().read_to_end(&mut buf)?;
    Ok(buf)
}

/// Extract corpus files (per lang.classify) from a gzipped tarball.
fn extract(lang: &dyn Lang, tarball: &Path, pkgdir: &Path) -> Result<Vec<ManifestFile>> {
    let mut files = Vec::new();
    let gz = flate2::read::GzDecoder::new(std::fs::File::open(tarball)?);
    let mut archive = tar::Archive::new(gz);
    for entry in archive.entries()? {
        let mut entry = entry?;
        let entry_path = entry.path()?.to_path_buf();
        let Some(rel) = safe_rel_path(&entry_path) else { continue };
        let Some(dialect) = lang.classify(&rel) else { continue };
        let dest = pkgdir.join(&rel);
        let mut buf = Vec::new();
        entry.read_to_end(&mut buf)?;
        std::fs::create_dir_all(dest.parent().unwrap())?;
        std::fs::write(&dest, &buf)?;
        files.push(ManifestFile {
            path: rel.to_string_lossy().into_owned(),
            bytes: buf.len() as u64,
            sha256: format!("{:x}", Sha256::digest(&buf)),
            dialect,
        });
    }
    files.sort_by(|a, b| a.path.cmp(&b.path));
    Ok(files)
}

pub fn run(lang: &dyn Lang, list: &Path, limit: usize, corpus: &Path) -> Result<()> {
    let ranked: Vec<RankedCrate> = serde_json::from_str(&std::fs::read_to_string(list)?)?;
    let cache = corpus.join("cache");
    let srcroot = corpus.join("src");
    std::fs::create_dir_all(&cache)?;
    std::fs::create_dir_all(&srcroot)?;

    let mut packages = Vec::new();
    for c in ranked.iter().take(limit) {
        let (version, tarball_url) = match lang.resolve(c) {
            Ok(v) => v,
            Err(e) => {
                eprintln!("fetch: skipping {}: {e:#}", c.name);
                continue;
            }
        };
        let stem = pkg_dir(&c.name, &version);
        // .crate/.tgz are legacy cache names from before the lang refactor.
        let tarball = ["tar.gz", "crate", "tgz"]
            .iter()
            .map(|e| cache.join(format!("{stem}.{e}")))
            .find(|p| p.exists())
            .unwrap_or_else(|| cache.join(format!("{stem}.tar.gz")));
        if !tarball.exists() {
            match download(&tarball_url) {
                Ok(buf) => {
                    std::fs::write(&tarball, &buf)?;
                    eprintln!("fetch: downloaded {stem} ({} KB)", buf.len() / 1024);
                }
                Err(e) => {
                    eprintln!("fetch: skipping {stem}: {e:#}");
                    continue;
                }
            }
        }
        let files = extract(lang, &tarball, &srcroot.join(&stem))?;
        eprintln!("fetch: {stem}: {} files", files.len());
        packages.push(ManifestPackage {
            package: c.name.clone(),
            version,
            downloads: c.downloads,
            files,
        });
    }

    let manifest = Manifest { packages };
    let total: usize = manifest.packages.iter().map(|p| p.files.len()).sum();
    std::fs::write(
        corpus.join("manifest.json"),
        serde_json::to_string_pretty(&manifest)?,
    )?;
    println!(
        "fetch: manifest written — {} packages, {} source files",
        manifest.packages.len(),
        total
    );
    Ok(())
}
