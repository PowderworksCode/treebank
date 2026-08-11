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

/// Directory name for a package (npm scopes contain '/', Maven
/// group:artifact coordinates contain ':').
pub fn pkg_dir(name: &str, version: &str) -> String {
    format!("{}-{}", name.replace(['/', ':'], "__"), version)
}

/// Reject empty paths and anything that is not a plain relative path.
fn safe_path(rel: &Path) -> Option<PathBuf> {
    if rel.as_os_str().is_empty()
        || rel
            .components()
            .any(|c| !matches!(c, Component::Normal(_)))
    {
        return None;
    }
    Some(rel.to_path_buf())
}

/// Strip the archive's own wrapper components and reject path traversal.
/// How many to strip is the language's call — see `Lang::archive_strip`.
fn strip_root(lang: &dyn Lang, entry_path: &Path, is_zip: bool) -> Option<PathBuf> {
    let mut comps = entry_path.components();
    for _ in 0..lang.archive_strip(entry_path, is_zip) {
        comps.next()?;
    }
    safe_path(comps.as_path())
}

fn download(url: &str) -> Result<Vec<u8>> {
    let resp = ureq::get(url).call().with_context(|| format!("GET {url}"))?;
    let mut buf = Vec::new();
    resp.into_reader().read_to_end(&mut buf)?;
    Ok(buf)
}

/// Write one extracted archive member into the corpus and describe it.
fn record(
    lang: &dyn Lang,
    pkgdir: &Path,
    rel: &Path,
    buf: &[u8],
) -> Result<Option<ManifestFile>> {
    let Some(dialect) = lang.classify(rel) else { return Ok(None) };
    if !lang.admit(rel, buf) {
        return Ok(None);
    }
    let dest = pkgdir.join(rel);
    std::fs::create_dir_all(dest.parent().unwrap())?;
    std::fs::write(&dest, buf)?;
    Ok(Some(ManifestFile {
        path: rel.to_string_lossy().into_owned(),
        bytes: buf.len() as u64,
        sha256: format!("{:x}", Sha256::digest(buf)),
        dialect,
    }))
}

/// Tar compression, by magic bytes. Registry tarballs are gzip, but Debian
/// ships whatever upstream released: measured over sid main, 60674 `.orig.tar.gz`,
/// 22708 `.orig.tar.xz` and 2384 `.orig.tar.bz2` — and all three appear inside
/// the top 25 C sources by popcon, so all three have to work.
fn decompress(archive: &Path) -> Result<Box<dyn Read>> {
    let mut magic = [0u8; 6];
    {
        use std::io::Read as _;
        let mut f = std::fs::File::open(archive)?;
        let _ = f.read(&mut magic)?;
    }
    let file = std::fs::File::open(archive)?;
    Ok(match magic {
        [0xfd, b'7', b'z', b'X', b'Z', 0x00] => Box::new(liblzma::read::XzDecoder::new(file)),
        [b'B', b'Z', b'h', ..] => Box::new(bzip2::read::BzDecoder::new(file)),
        _ => Box::new(flate2::read::GzDecoder::new(file)),
    })
}

/// Extract corpus files (per lang.classify) from a package archive.
///
/// Registries ship two shapes. Compressed tarballs (npm, crates.io, GitHub
/// source archives, Debian `.orig.tar.*`) wrap everything in one top-level
/// directory, which is stripped. Zips (Maven `-sources.jar`, `.nupkg`) do not:
/// their entries are already root-relative, so stripping would drop the first
/// path segment — the whole `com/` of `com/google/common/base/Ascii.java`.
fn extract(lang: &dyn Lang, archive: &Path, pkgdir: &Path) -> Result<Vec<ManifestFile>> {
    let mut files = Vec::new();
    let is_zip = {
        use std::io::Read as _;
        let mut magic = [0u8; 4];
        let mut f = std::fs::File::open(archive)?;
        f.read_exact(&mut magic).is_ok() && &magic[..2] == b"PK"
    };

    if is_zip {
        let mut zip = zip::ZipArchive::new(std::fs::File::open(archive)?)
            .with_context(|| format!("open zip {}", archive.display()))?;
        for i in 0..zip.len() {
            let mut entry = zip.by_index(i)?;
            if !entry.is_file() {
                continue;
            }
            let Some(rel) = entry.enclosed_name() else { continue };
            let Some(rel) = strip_root(lang, &rel, true) else { continue };
            let mut buf = Vec::new();
            entry.read_to_end(&mut buf)?;
            files.extend(record(lang, pkgdir, &rel, &buf)?);
        }
    } else {
        let mut tar = tar::Archive::new(decompress(archive)?);
        for entry in tar.entries()? {
            let mut entry = entry?;
            let entry_path = entry.path()?.to_path_buf();
            let Some(rel) = strip_root(lang, &entry_path, false) else { continue };
            let mut buf = Vec::new();
            entry.read_to_end(&mut buf)?;
            files.extend(record(lang, pkgdir, &rel, &buf)?);
        }
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
        // The extension is cosmetic — extract() sniffs the magic bytes — but
        // a cached Maven sources.jar should not be named .tar.gz.
        // .crate/.tgz are legacy cache names from before the lang refactor.
        let ext = ["jar", "zip", "nupkg", "tar.xz", "tar.bz2"]
            .into_iter()
            .find(|e| tarball_url.ends_with(&format!(".{e}")))
            .unwrap_or("tar.gz");
        let tarball = ["tar.gz", "tar.xz", "tar.bz2", "crate", "tgz", "jar", "zip", "nupkg"]
            .iter()
            .map(|e| cache.join(format!("{stem}.{e}")))
            .find(|p| p.exists())
            .unwrap_or_else(|| cache.join(format!("{stem}.{ext}")));
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
