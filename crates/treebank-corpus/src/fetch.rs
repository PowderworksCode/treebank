use std::io::Read;
use std::path::{Component, Path, PathBuf};

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::Ecosystem;
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
fn strip_root(lang: &dyn Ecosystem, entry_path: &Path, is_zip: bool) -> Option<PathBuf> {
    let mut comps = entry_path.components();
    for _ in 0..lang.archive_strip(entry_path, is_zip) {
        comps.next()?;
    }
    safe_path(comps.as_path())
}

/// Read timeouts are not hygiene here, they are a measured need. A Debian
/// mirror stalled mid-body on a 726 MB tarball — socket ESTABLISHED, bytes
/// sitting unread in the receive queue, zero progress for fifteen minutes —
/// and with ureq's default of no read timeout that wedges the whole fetch
/// behind one package. The limit is per read syscall, so a slow-but-alive
/// transfer is unaffected and only a dead one is cut.
fn download(url: &str, max_bytes: Option<u64>) -> Result<Vec<u8>> {
    let resp = ureq::AgentBuilder::new()
        .timeout_connect(std::time::Duration::from_secs(30))
        .timeout_read(std::time::Duration::from_secs(60))
        .build()
        .get(url)
        .call()
        .with_context(|| format!("GET {url}"))?;
    // Content-Length is advisory, so the cap is also enforced while reading;
    // checking the header first is what avoids spending the download to find
    // out.
    if let (Some(cap), Some(len)) = (
        max_bytes,
        resp.header("Content-Length").and_then(|v| v.parse::<u64>().ok()),
    ) {
        anyhow::ensure!(
            len <= cap,
            "artifact is {} MB, over this language's {} MB cap",
            len / 1_000_000,
            cap / 1_000_000
        );
    }
    let mut buf = Vec::new();
    match max_bytes {
        None => {
            resp.into_reader().read_to_end(&mut buf)?;
        }
        Some(cap) => {
            resp.into_reader().take(cap + 1).read_to_end(&mut buf)?;
            anyhow::ensure!(
                buf.len() as u64 <= cap,
                "artifact exceeds this language's {} MB cap",
                cap / 1_000_000
            );
        }
    }
    Ok(buf)
}

/// Write one extracted archive member into the corpus and describe it.
fn record(
    lang: &dyn Ecosystem,
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
    let mut head = vec![0u8; 512];
    {
        use std::io::Read as _;
        let mut f = std::fs::File::open(archive)?;
        let n = f.read(&mut head)?;
        head.truncate(n);
    }
    let file = std::fs::File::open(archive)?;
    if matches!(shape(&head), Shape::PlainTar) {
        return Ok(Box::new(file));
    }
    Ok(match magic {
        [0xfd, b'7', b'z', b'X', b'Z', 0x00] => Box::new(liblzma::read::XzDecoder::new(file)),
        [b'B', b'Z', b'h', ..] => Box::new(bzip2::read::BzDecoder::new(file)),
        _ => Box::new(flate2::read::GzDecoder::new(file)),
    })
}

/// Decompress an in-memory archive the same way `decompress` does a file.
fn decompress_bytes(buf: Vec<u8>) -> Box<dyn Read> {
    if matches!(shape(&buf), Shape::PlainTar) {
        return Box::new(std::io::Cursor::new(buf));
    }
    match buf.first_chunk::<6>() {
        Some([0xfd, b'7', b'z', b'X', b'Z', 0x00]) => {
            Box::new(liblzma::read::XzDecoder::new(std::io::Cursor::new(buf)))
        }
        Some([b'B', b'Z', b'h', ..]) => {
            Box::new(bzip2::read::BzDecoder::new(std::io::Cursor::new(buf)))
        }
        _ => Box::new(flate2::read::GzDecoder::new(std::io::Cursor::new(buf))),
    }
}

/// Which archive shape a blob is, by magic bytes.
enum Shape {
    Zip,
    Tar,
    /// An uncompressed tar. Nothing is *published* in this shape — it is what
    /// a container looks like. A RubyGems `.gem` is one: a plain tar holding
    /// `metadata.gz`, `checksums.yaml.gz` and `data.tar.gz`, with every source
    /// file inside that last member.
    PlainTar,
    NotAnArchive,
}

fn shape(buf: &[u8]) -> Shape {
    // `ustar` sits at offset 257 of a tar header, so this needs more than the
    // magic-byte prefix the compressed shapes need. A buffer too short to
    // reach it simply is not a plain tar.
    if buf.len() >= 262 && &buf[257..262] == b"ustar" {
        return Shape::PlainTar;
    }
    match buf.first_chunk::<6>() {
        Some([b'P', b'K', ..]) => Shape::Zip,
        Some([0x1f, 0x8b, ..]) | Some([b'B', b'Z', b'h', ..]) => Shape::Tar,
        Some([0xfd, b'7', b'z', b'X', b'Z', 0x00]) => Shape::Tar,
        _ => Shape::NotAnArchive,
    }
}

/// Extract corpus files (per lang.classify) from a package archive.
///
/// Registries ship two shapes. Compressed tarballs (npm, crates.io, GitHub
/// source archives, Debian `.orig.tar.*`) wrap everything in one top-level
/// directory, which is stripped. Zips (Maven `-sources.jar`, `.nupkg`) do not:
/// their entries are already root-relative, so stripping would drop the first
/// path segment — the whole `com/` of `com/google/common/base/Ascii.java`.
///
/// A third shape appears with LuaRocks: an archive whose *member* is itself
/// an archive. A `.src.rock` is a zip holding the rockspec plus the package's
/// source, and how that source is carried is the packager's choice — often an
/// unpacked directory, but for roughly a quarter of rocks (measured: 12 of the
/// top 50 by downloads, including argparse, lpeg, luasocket and lua_cliargs)
/// it is upstream's release tarball, dropped in whole. Walking only the outer
/// archive finds no source files in those at all and reports them as empty
/// packages, which reads as "this package has no Lua in it" rather than "the
/// extractor stopped one level too early". Languages opt in through
/// `Lang::nested_archives`; recursion is one level only, which is all any
/// observed rock needs and keeps a zip bomb from turning into an unbounded
/// walk.
fn extract(lang: &dyn Ecosystem, archive: &Path, pkgdir: &Path) -> Result<Vec<ManifestFile>> {
    let mut files = Vec::new();
    // The package archive is streamed from disk rather than read into memory:
    // Debian .orig.tar.* in the C corpus run to hundreds of megabytes, and
    // slurping one to reach its members would be a real regression for every
    // language that has never needed nested extraction. Only nested members —
    // which the entry reader has already produced as bytes — go through the
    // in-memory path.
    let mut magic = [0u8; 6];
    {
        let mut f = std::fs::File::open(archive)
            .with_context(|| format!("open {}", archive.display()))?;
        let _ = f.read(&mut magic)?;
    }
    let ctx = || format!("extract {}", archive.display());
    match shape(&magic) {
        Shape::Zip => {
            let mut zip = zip::ZipArchive::new(std::fs::File::open(archive)?)
                .with_context(|| format!("open zip {}", archive.display()))?;
            for i in 0..zip.len() {
                let mut entry = zip.by_index(i).with_context(ctx)?;
                if !entry.is_file() {
                    continue;
                }
                let Some(rel) = entry.enclosed_name() else { continue };
                let Some(rel) = strip_root(lang, &rel, true) else { continue };
                let mut buf = Vec::new();
                entry.read_to_end(&mut buf).with_context(ctx)?;
                take(lang, buf, pkgdir, Path::new(""), &rel, true, &mut files).with_context(ctx)?;
            }
        }
        _ => {
            let mut tar = tar::Archive::new(decompress(archive)?);
            for entry in tar.entries().with_context(ctx)? {
                let mut entry = entry.with_context(ctx)?;
                // Regular files only, the same check the zip branch above makes.
                // A tar carries directory, symlink and hardlink entries too, and
                // `record` would write a zero-byte regular FILE at such an
                // entry's path whenever the name passes `classify` — after which
                // no entry underneath it can create its parent directory, and
                // the whole fetch dies with `File exists (os error 17)` rather
                // than skipping one package.
                //
                // Two sessions found this independently, from different
                // triggers, which is worth recording because it is one defect
                // with two doors. A SYMLINK entry read for its bytes yields
                // none, and killed a 500-repo bash fetch. A DIRECTORY whose name
                // carries the source extension does the same: Zig repositories
                // name directories after their build entry point, so
                // `examples/example-with-build.zig/` is idiomatic rather than
                // exotic (measured: jedisct1/zigly, rank 380 of the Zig
                // top-500). No other language here puts its source extension on
                // a directory, which is part of why the tar path went this long
                // without the check the zip path always had.
                if !entry.header().entry_type().is_file() {
                    continue;
                }
                let entry_path = entry.path().with_context(ctx)?.to_path_buf();
                let Some(rel) = strip_root(lang, &entry_path, false) else { continue };
                let mut buf = Vec::new();
                entry.read_to_end(&mut buf).with_context(ctx)?;
                take(lang, buf, pkgdir, Path::new(""), &rel, true, &mut files).with_context(ctx)?;
            }
        }
    }
    files.sort_by(|a, b| a.path.cmp(&b.path));
    files.dedup_by(|a, b| a.path == b.path);
    Ok(files)
}

/// Walk a NESTED archive, already in memory because its containing entry was
/// read to produce it. Unlike the package archive it keeps its own wrapping
/// directory: that is what stops `dkjson-2.11/dkjson.lua` colliding with
/// another member's `dkjson.lua`, and it records which inner archive a file
/// came from.
fn walk(
    lang: &dyn Ecosystem,
    buf: Vec<u8>,
    pkgdir: &Path,
    prefix: &Path,
    files: &mut Vec<ManifestFile>,
) -> Result<()> {
    match shape(&buf) {
        Shape::Zip => {
            let mut zip = zip::ZipArchive::new(std::io::Cursor::new(buf)).context("open zip")?;
            for i in 0..zip.len() {
                let mut entry = zip.by_index(i)?;
                if !entry.is_file() {
                    continue;
                }
                let Some(rel) = entry.enclosed_name() else { continue };
                let Some(rel) = safe_path(&rel) else { continue };
                let mut inner = Vec::new();
                entry.read_to_end(&mut inner)?;
                take(lang, inner, pkgdir, prefix, &rel, false, files)?;
            }
        }
        Shape::Tar | Shape::PlainTar => {
            let mut tar = tar::Archive::new(decompress_bytes(buf));
            for entry in tar.entries()? {
                let mut entry = entry?;
                // Regular files only, for the reason the outer tar branch
                // gives: a symlink yields no bytes, lands as an empty regular
                // file, and blocks the directory beneath it. A rock's inner
                // release tarball is an upstream tarball like any other and
                // carries symlinks just the same.
                if !entry.header().entry_type().is_file() {
                    continue;
                }
                let entry_path = entry.path()?.to_path_buf();
                // A nested archive keeps its own root.
                let Some(rel) = safe_path(&entry_path) else { continue };
                let mut inner = Vec::new();
                entry.read_to_end(&mut inner)?;
                take(lang, inner, pkgdir, prefix, &rel, false, files)?;
            }
        }
        Shape::NotAnArchive => {}
    }
    Ok(())
}

/// One archive member: recurse into it if it is itself an archive and this
/// language asked for that, otherwise record it.
fn take(
    lang: &dyn Ecosystem,
    buf: Vec<u8>,
    pkgdir: &Path,
    prefix: &Path,
    rel: &Path,
    outermost: bool,
    files: &mut Vec<ManifestFile>,
) -> Result<()> {
    if outermost
        && lang.nested_archives()
        && lang.nested_archive_member(rel)
        && !matches!(shape(&buf), Shape::NotAnArchive)
    {
        // Nest under the archive's own path so two inner archives cannot
        // overwrite each other, and so the manifest shows the provenance.
        let nested_prefix = prefix.join(rel);
        // Non-fatal on purpose. Magic bytes say "this is an archive", not
        // "this is a WELL-FORMED archive", and corpus packages ship
        // deliberately broken ones: luarocks itself carries corrupt-archive
        // fixtures for its own error handling, whose gzip header is followed
        // by garbage. Those must skip the member, not abort the package and
        // take the rest of the fetch down with it.
        if let Err(e) = walk(lang, buf, pkgdir, &nested_prefix, files) {
            eprintln!("fetch: skipping unreadable nested archive {}: {e:#}", nested_prefix.display());
        }
        return Ok(());
    }
    let full = prefix.join(rel);
    files.extend(record(lang, pkgdir, &full, &buf)?);
    Ok(())
}

pub fn run(lang: &dyn Ecosystem, list: &Path, limit: usize, corpus: &Path) -> Result<()> {
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
        let ext = ["jar", "src.rock", "zip", "nupkg", "tar.xz", "tar.bz2"]
            .into_iter()
            .find(|e| tarball_url.ends_with(&format!(".{e}")))
            .unwrap_or("tar.gz");
        let tarball = ["tar.gz", "tar.xz", "tar.bz2", "crate", "tgz", "jar", "src.rock", "zip", "nupkg"]
            .iter()
            .map(|e| cache.join(format!("{stem}.{e}")))
            .find(|p| p.exists())
            .unwrap_or_else(|| cache.join(format!("{stem}.{ext}")));
        if !tarball.exists() {
            match download(&tarball_url, lang.max_artifact_bytes()) {
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
