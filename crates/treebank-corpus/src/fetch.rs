use std::collections::{BTreeMap, BTreeSet};
use std::io::Read;
use std::path::{Component, Path, PathBuf};

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::rank::RankedCrate;
use crate::Ecosystem;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ManifestFile {
    pub path: String,
    pub bytes: u64,
    pub sha256: String,
    /// Grammar routing hint (e.g. "tsx"); absent means the language default.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub dialect: Option<String>,
}

/// The exact package archive from which the corpus files were extracted.
///
/// File hashes alone prove what a local corpus contains, but cannot recreate
/// it. Keeping the immutable URL and archive digest makes a committed manifest
/// a corpus lock rather than a report about one machine's cache.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ManifestArtifact {
    pub url: String,
    pub bytes: u64,
    pub sha256: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ManifestPackage {
    pub package: String,
    pub version: String,
    pub downloads: u64,
    /// Absent only in manifests written before exact corpus hydration existed.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub artifact: Option<ManifestArtifact>,
    pub files: Vec<ManifestFile>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Manifest {
    /// Absent only in manifests written before exact corpus hydration existed.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub language: Option<String>,
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

fn sha256_file(path: &Path) -> Result<(u64, String)> {
    let mut file = std::fs::File::open(path).with_context(|| format!("open {}", path.display()))?;
    let mut hash = Sha256::new();
    let bytes =
        std::io::copy(&mut file, &mut hash).with_context(|| format!("hash {}", path.display()))?;
    Ok((bytes, format!("{:x}", hash.finalize())))
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
    if rel.as_os_str().is_empty() || rel.components().any(|c| !matches!(c, Component::Normal(_))) {
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
        resp.header("Content-Length")
            .and_then(|v| v.parse::<u64>().ok()),
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
    let Some(dialect) = lang.classify(rel) else {
        return Ok(None);
    };
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
        let mut f =
            std::fs::File::open(archive).with_context(|| format!("open {}", archive.display()))?;
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
                let Some(rel) = entry.enclosed_name() else {
                    continue;
                };
                let Some(rel) = strip_root(lang, &rel, true) else {
                    continue;
                };
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
                let Some(rel) = strip_root(lang, &entry_path, false) else {
                    continue;
                };
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
                let Some(rel) = entry.enclosed_name() else {
                    continue;
                };
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
                let Some(rel) = safe_path(&entry_path) else {
                    continue;
                };
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
            eprintln!(
                "fetch: skipping unreadable nested archive {}: {e:#}",
                nested_prefix.display()
            );
        }
        return Ok(());
    }
    let full = prefix.join(rel);
    files.extend(record(lang, pkgdir, &full, &buf)?);
    Ok(())
}

pub fn run(
    lang: &dyn Ecosystem,
    list: &Path,
    limit: usize,
    corpus: &Path,
    lock_out: Option<&Path>,
    lock_only: bool,
) -> Result<()> {
    anyhow::ensure!(
        !lock_only || lock_out.is_some(),
        "fetch: --lock-only requires a lock output path"
    );
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
        let tarball = [
            "tar.gz", "tar.xz", "tar.bz2", "crate", "tgz", "jar", "src.rock", "zip", "nupkg",
        ]
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
        let (artifact_bytes, artifact_sha256) = sha256_file(&tarball)?;
        eprintln!("fetch: {stem}: {} files", files.len());
        packages.push(ManifestPackage {
            package: c.name.clone(),
            version,
            downloads: c.downloads,
            artifact: Some(ManifestArtifact {
                url: tarball_url,
                bytes: artifact_bytes,
                sha256: artifact_sha256,
            }),
            files,
        });
        if lock_only {
            let extracted = srcroot.join(&stem);
            if extracted.exists() {
                std::fs::remove_dir_all(&extracted)
                    .with_context(|| format!("remove extracted package {stem}"))?;
            }
            std::fs::remove_file(&tarball)
                .with_context(|| format!("remove downloaded package {stem}"))?;
        }
    }

    let manifest = Manifest {
        language: Some(lang.name().as_str().to_string()),
        packages,
    };
    let total: usize = manifest.packages.iter().map(|p| p.files.len()).sum();
    let json = format!("{}\n", serde_json::to_string_pretty(&manifest)?);
    if !lock_only {
        std::fs::write(corpus.join("manifest.json"), &json)?;
    }
    if let Some(lock_out) = lock_out {
        if let Some(parent) = lock_out.parent().filter(|p| !p.as_os_str().is_empty()) {
            std::fs::create_dir_all(parent)?;
        }
        std::fs::write(lock_out, &json)?;
    }
    println!(
        "fetch: manifest written — {} packages, {} source files",
        manifest.packages.len(),
        total
    );
    Ok(())
}

/// Materialise the exact corpus described by a committed manifest.
///
/// Unlike `run`, this never asks an ecosystem what "latest" means. Every
/// archive and every extracted source file must match the lock before the
/// staged source tree is published under `corpus/src`.
pub fn hydrate(lang: &dyn Ecosystem, lock: &Path, corpus: &Path) -> Result<()> {
    let manifest = Manifest::load(lock).with_context(|| format!("load {}", lock.display()))?;
    let locked_language = manifest.language.as_deref().context(
        "corpus lock has no language; regenerate it with `treebank fetch --lock-out ...`",
    )?;
    anyhow::ensure!(
        locked_language == lang.name().as_str(),
        "corpus lock is for {locked_language}, not {}",
        lang.name()
    );

    let srcroot = corpus.join("src");
    if srcroot.exists() {
        anyhow::ensure!(
            std::fs::read_dir(&srcroot)?.next().is_none(),
            "refusing to hydrate over non-empty {}",
            srcroot.display()
        );
    }
    std::fs::create_dir_all(corpus)?;
    let cache = corpus.join("cache");
    std::fs::create_dir_all(&cache)?;

    let stamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)?
        .as_nanos();
    let staging = corpus.join(format!(".hydrate-{}-{stamp}", std::process::id()));
    let stage = StageDir::new(staging)?;
    let staged_src = stage.path().join("src");
    std::fs::create_dir_all(&staged_src)?;

    let mut package_dirs = BTreeSet::new();
    for package in &manifest.packages {
        let artifact = package.artifact.as_ref().with_context(|| {
            format!(
                "{} {} has no archive provenance; regenerate the lock with `treebank fetch --lock-out ...`",
                package.package, package.version
            )
        })?;
        anyhow::ensure!(
            artifact.sha256.len() == 64 && artifact.sha256.bytes().all(|b| b.is_ascii_hexdigit()),
            "{} {} has an invalid archive sha256",
            package.package,
            package.version
        );
        if let Some(cap) = lang.max_artifact_bytes() {
            anyhow::ensure!(
                artifact.bytes <= cap,
                "locked artifact for {} is {} MB, over this language's {} MB cap",
                package.package,
                artifact.bytes / 1_000_000,
                cap / 1_000_000
            );
        }

        let archive = cache.join(format!("{}.archive", artifact.sha256));
        if archive.exists() {
            verify_artifact(&archive, artifact)
                .with_context(|| format!("cached artifact for {} is corrupt", package.package))?;
        } else {
            let bytes = download(&artifact.url, lang.max_artifact_bytes())?;
            verify_artifact_bytes(&bytes, artifact).with_context(|| {
                format!(
                    "downloaded artifact for {} does not match lock",
                    package.package
                )
            })?;
            let pending = cache.join(format!(
                ".{}-{}.pending",
                artifact.sha256,
                std::process::id()
            ));
            std::fs::write(&pending, &bytes)?;
            std::fs::rename(&pending, &archive)?;
        }

        let dir = pkg_dir(&package.package, &package.version);
        anyhow::ensure!(
            package_dirs.insert(dir.clone()),
            "duplicate package directory in lock: {dir}"
        );
        let actual = extract(lang, &archive, &staged_src.join(&dir))
            .with_context(|| format!("extract {} {}", package.package, package.version))?;
        compare_files(package, &actual)?;
        eprintln!("hydrate: {dir}: {} files verified", actual.len());
    }

    let staged_manifest = stage.path().join("manifest.json");
    std::fs::write(
        &staged_manifest,
        format!("{}\n", serde_json::to_string_pretty(&manifest)?),
    )?;
    if srcroot.exists() {
        std::fs::remove_dir(&srcroot)?;
    }
    std::fs::rename(&staged_src, &srcroot)?;
    std::fs::rename(&staged_manifest, corpus.join("manifest.json"))?;
    // Only the now-empty staging directory remains after its two children
    // were atomically moved into place.
    std::fs::remove_dir(stage.path())?;

    let total: usize = manifest.packages.iter().map(|p| p.files.len()).sum();
    println!(
        "hydrate: verified {} packages, {} source files",
        manifest.packages.len(),
        total
    );
    Ok(())
}

fn verify_artifact(path: &Path, expected: &ManifestArtifact) -> Result<()> {
    let (bytes, sha256) = sha256_file(path)?;
    anyhow::ensure!(
        bytes == expected.bytes,
        "expected {} bytes, got {bytes}",
        expected.bytes
    );
    anyhow::ensure!(
        sha256 == expected.sha256,
        "expected sha256 {}, got {sha256}",
        expected.sha256
    );
    Ok(())
}

fn verify_artifact_bytes(bytes: &[u8], expected: &ManifestArtifact) -> Result<()> {
    anyhow::ensure!(
        bytes.len() as u64 == expected.bytes,
        "expected {} bytes, got {}",
        expected.bytes,
        bytes.len()
    );
    let sha256 = format!("{:x}", Sha256::digest(bytes));
    anyhow::ensure!(
        sha256 == expected.sha256,
        "expected sha256 {}, got {sha256}",
        expected.sha256
    );
    Ok(())
}

fn compare_files(package: &ManifestPackage, actual: &[ManifestFile]) -> Result<()> {
    let mut expected_by_path = BTreeMap::new();
    for file in &package.files {
        anyhow::ensure!(
            expected_by_path.insert(&file.path, file).is_none(),
            "{} {} repeats file {} in the lock",
            package.package,
            package.version,
            file.path
        );
    }
    let actual_by_path: BTreeMap<_, _> = actual.iter().map(|f| (&f.path, f)).collect();

    let missing: Vec<_> = expected_by_path
        .keys()
        .filter(|path| !actual_by_path.contains_key(*path))
        .copied()
        .collect();
    let extra: Vec<_> = actual_by_path
        .keys()
        .filter(|path| !expected_by_path.contains_key(*path))
        .copied()
        .collect();
    anyhow::ensure!(
        missing.is_empty(),
        "{} {} is missing locked files: {}",
        package.package,
        package.version,
        missing
            .iter()
            .map(|p| p.as_str())
            .collect::<Vec<_>>()
            .join(", ")
    );
    anyhow::ensure!(
        extra.is_empty(),
        "{} {} has extra files: {}",
        package.package,
        package.version,
        extra
            .iter()
            .map(|p| p.as_str())
            .collect::<Vec<_>>()
            .join(", ")
    );
    for (path, expected) in expected_by_path {
        let got = actual_by_path[path];
        anyhow::ensure!(
            got == expected,
            "{} {} file {path} changed: expected {} bytes sha256 {} dialect {:?}, got {} bytes sha256 {} dialect {:?}",
            package.package,
            package.version,
            expected.bytes,
            expected.sha256,
            expected.dialect,
            got.bytes,
            got.sha256,
            got.dialect
        );
    }
    Ok(())
}

struct StageDir {
    path: PathBuf,
}

impl StageDir {
    fn new(path: PathBuf) -> Result<Self> {
        std::fs::create_dir(&path)
            .with_context(|| format!("create staging directory {}", path.display()))?;
        Ok(Self { path })
    }

    fn path(&self) -> &Path {
        &self.path
    }
}

impl Drop for StageDir {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.path);
    }
}

#[cfg(test)]
mod hydrate_tests {
    use super::*;
    use crate::rank::RankedCrate;
    use crate::{Ecosystem, LangName};
    use std::sync::atomic::{AtomicU64, Ordering};

    struct RustFixture;

    impl Ecosystem for RustFixture {
        fn name(&self) -> LangName {
            LangName::Rust
        }

        fn rank(&self, _db: &Path, _k: usize) -> Result<Vec<RankedCrate>> {
            unreachable!("hydration never ranks")
        }

        fn resolve(&self, _pkg: &RankedCrate) -> Result<(String, String)> {
            Ok((
                "1.0.0".to_string(),
                "https://registry.invalid/fixture-1.0.0.tar.gz".to_string(),
            ))
        }

        fn classify(&self, rel: &Path) -> Option<Option<String>> {
            (rel.extension().and_then(|e| e.to_str()) == Some("rs")).then_some(None)
        }
    }

    static NEXT_TEMP: AtomicU64 = AtomicU64::new(0);

    struct TempRoot(PathBuf);

    impl TempRoot {
        fn new() -> Self {
            let n = NEXT_TEMP.fetch_add(1, Ordering::Relaxed);
            let path = std::env::temp_dir()
                .join(format!("treebank-hydrate-test-{}-{n}", std::process::id()));
            let _ = std::fs::remove_dir_all(&path);
            std::fs::create_dir(&path).unwrap();
            Self(path)
        }
    }

    impl Drop for TempRoot {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    fn archive(files: &[(&str, &[u8])]) -> Vec<u8> {
        let gzip = flate2::write::GzEncoder::new(Vec::new(), flate2::Compression::default());
        let mut tar = tar::Builder::new(gzip);
        for (path, bytes) in files {
            let mut header = tar::Header::new_gnu();
            header.set_size(bytes.len() as u64);
            header.set_mode(0o644);
            header.set_cksum();
            tar.append_data(&mut header, format!("fixture-1.0.0/{path}"), *bytes)
                .unwrap();
        }
        tar.into_inner().unwrap().finish().unwrap()
    }

    fn file(path: &str, bytes: &[u8]) -> ManifestFile {
        ManifestFile {
            path: path.to_string(),
            bytes: bytes.len() as u64,
            sha256: format!("{:x}", Sha256::digest(bytes)),
            dialect: None,
        }
    }

    fn locked(root: &TempRoot, archive: &[u8], files: Vec<ManifestFile>) -> (PathBuf, PathBuf) {
        let digest = format!("{:x}", Sha256::digest(archive));
        let manifest = Manifest {
            language: Some("rust".to_string()),
            packages: vec![ManifestPackage {
                package: "fixture".to_string(),
                version: "1.0.0".to_string(),
                downloads: 42,
                artifact: Some(ManifestArtifact {
                    // Hydration must not touch this URL when the verified
                    // content-addressed cache entry already exists.
                    url: "http://127.0.0.1:9/should-not-be-fetched".to_string(),
                    bytes: archive.len() as u64,
                    sha256: digest.clone(),
                }),
                files,
            }],
        };
        let lock = root.0.join("lock.json");
        std::fs::write(&lock, serde_json::to_string_pretty(&manifest).unwrap()).unwrap();
        let corpus = root.0.join("corpus");
        std::fs::create_dir_all(corpus.join("cache")).unwrap();
        std::fs::write(
            corpus.join("cache").join(format!("{digest}.archive")),
            archive,
        )
        .unwrap();
        (lock, corpus)
    }

    #[test]
    fn fetch_writes_archive_provenance_to_the_corpus_and_lock() {
        let root = TempRoot::new();
        let source = b"fn fetched() {}\n";
        let bytes = archive(&[("src/lib.rs", source)]);
        let list = root.0.join("top-k.json");
        std::fs::write(
            &list,
            serde_json::to_string(&vec![RankedCrate {
                rank: 1,
                name: "fixture".to_string(),
                version: String::new(),
                downloads: 42,
            }])
            .unwrap(),
        )
        .unwrap();
        let corpus = root.0.join("corpus");
        std::fs::create_dir_all(corpus.join("cache")).unwrap();
        std::fs::write(corpus.join("cache/fixture-1.0.0.tar.gz"), &bytes).unwrap();
        let lock = root.0.join("locks/rust.json");

        run(&RustFixture, &list, 1, &corpus, Some(&lock), false).unwrap();

        let written = Manifest::load(&lock).unwrap();
        assert_eq!(written.language.as_deref(), Some("rust"));
        let artifact = written.packages[0].artifact.as_ref().unwrap();
        assert_eq!(artifact.bytes, bytes.len() as u64);
        assert_eq!(artifact.sha256, format!("{:x}", Sha256::digest(&bytes)));
        assert_eq!(
            artifact.url,
            "https://registry.invalid/fixture-1.0.0.tar.gz"
        );
        assert_eq!(
            std::fs::read(corpus.join("manifest.json")).unwrap(),
            std::fs::read(lock).unwrap()
        );
    }

    #[test]
    fn lock_only_does_not_retain_the_corpus_or_archive() {
        let root = TempRoot::new();
        let source = b"fn fetched() {}\n";
        let bytes = archive(&[("src/lib.rs", source)]);
        let list = root.0.join("top-k.json");
        std::fs::write(
            &list,
            serde_json::to_string(&vec![RankedCrate {
                rank: 1,
                name: "fixture".to_string(),
                version: String::new(),
                downloads: 42,
            }])
            .unwrap(),
        )
        .unwrap();
        let corpus = root.0.join("corpus");
        std::fs::create_dir_all(corpus.join("cache")).unwrap();
        std::fs::write(corpus.join("cache/fixture-1.0.0.tar.gz"), &bytes).unwrap();
        let lock = root.0.join("locks/rust.json");

        run(&RustFixture, &list, 1, &corpus, Some(&lock), true).unwrap();

        assert!(lock.is_file());
        assert!(!corpus.join("manifest.json").exists());
        assert!(!corpus.join("src/fixture-1.0.0").exists());
        assert!(!corpus.join("cache/fixture-1.0.0.tar.gz").exists());
    }

    #[test]
    fn hydrate_publishes_only_after_archive_and_files_match() {
        let root = TempRoot::new();
        let source = b"pub fn answer() -> u8 { 42 }\n";
        let bytes = archive(&[("src/lib.rs", source)]);
        let (lock, corpus) = locked(&root, &bytes, vec![file("src/lib.rs", source)]);

        hydrate(&RustFixture, &lock, &corpus).unwrap();

        assert_eq!(
            std::fs::read(corpus.join("src/fixture-1.0.0/src/lib.rs")).unwrap(),
            source
        );
        assert_eq!(
            Manifest::load(&corpus.join("manifest.json"))
                .unwrap()
                .packages
                .len(),
            1
        );
    }

    #[test]
    fn hydrate_rejects_a_corrupt_cached_archive_without_publishing() {
        let root = TempRoot::new();
        let source = b"fn main() {}\n";
        let bytes = archive(&[("src/main.rs", source)]);
        let (lock, corpus) = locked(&root, &bytes, vec![file("src/main.rs", source)]);
        let digest = format!("{:x}", Sha256::digest(&bytes));
        std::fs::write(
            corpus.join("cache").join(format!("{digest}.archive")),
            b"corrupt",
        )
        .unwrap();

        let error = hydrate(&RustFixture, &lock, &corpus)
            .unwrap_err()
            .to_string();

        assert!(
            error.contains("cached artifact for fixture is corrupt"),
            "{error}"
        );
        assert!(!corpus.join("src").exists());
    }

    #[test]
    fn hydrate_rejects_missing_locked_files_without_publishing() {
        let root = TempRoot::new();
        let source = b"fn present() {}\n";
        let bytes = archive(&[("src/present.rs", source)]);
        let (lock, corpus) = locked(
            &root,
            &bytes,
            vec![
                file("src/present.rs", source),
                file("src/missing.rs", b"fn missing() {}\n"),
            ],
        );

        let error = hydrate(&RustFixture, &lock, &corpus)
            .unwrap_err()
            .to_string();

        assert!(
            error.contains("missing locked files: src/missing.rs"),
            "{error}"
        );
        assert!(!corpus.join("src").exists());
    }

    #[test]
    fn hydrate_rejects_extra_extracted_files_without_publishing() {
        let root = TempRoot::new();
        let source = b"fn expected() {}\n";
        let bytes = archive(&[
            ("src/expected.rs", source.as_slice()),
            ("src/extra.rs", b"fn extra() {}\n".as_slice()),
        ]);
        let (lock, corpus) = locked(&root, &bytes, vec![file("src/expected.rs", source)]);

        let error = hydrate(&RustFixture, &lock, &corpus)
            .unwrap_err()
            .to_string();

        assert!(error.contains("has extra files: src/extra.rs"), "{error}");
        assert!(!corpus.join("src").exists());
    }

    #[test]
    fn hydrate_rejects_changed_file_content_without_publishing() {
        let root = TempRoot::new();
        let bytes = archive(&[("src/lib.rs", b"fn actual() {}\n")]);
        let (lock, corpus) = locked(
            &root,
            &bytes,
            vec![file("src/lib.rs", b"fn expected() {}\n")],
        );

        let error = hydrate(&RustFixture, &lock, &corpus)
            .unwrap_err()
            .to_string();

        assert!(error.contains("file src/lib.rs changed"), "{error}");
        assert!(!corpus.join("src").exists());
    }

    #[test]
    fn hydrate_explains_why_legacy_manifests_are_not_locks() {
        let root = TempRoot::new();
        let lock = root.0.join("legacy.json");
        std::fs::write(&lock, r#"{"packages":[]}"#).unwrap();

        let error = hydrate(&RustFixture, &lock, &root.0.join("corpus"))
            .unwrap_err()
            .to_string();

        assert!(error.contains("corpus lock has no language"), "{error}");
    }
}
