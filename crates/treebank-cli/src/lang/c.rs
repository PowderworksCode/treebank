use std::collections::HashMap;
use std::io::{BufRead, BufReader, Read, Write};
use std::path::Path;
use std::process::{Command, Stdio};
use std::sync::LazyLock;

use anyhow::{bail, Context, Result};
use serde::{Deserialize, Serialize};

use super::Lang;
use crate::ledger::LangName;
use crate::rank::RankedCrate;

pub struct C;

/// C has no registry, so "popular C" has to be borrowed from somewhere. The
/// choice here is **Debian**, and it is a real choice with real bias:
///
/// - Ranking is popcon (`popularity-contest`) install counts, aggregated per
///   *source* package by Debian itself. Unlike Java's dependent-repos proxy,
///   this is closer to crates.io downloads in kind: it counts machines, not
///   dependency edges.
/// - The corpus is therefore **the C that ships in a distribution**: system
///   libraries, daemons, autotools trees, GNU extensions, decades-old code
///   that still runs everything. It is emphatically *not* "trending C on
///   GitHub", and the gap numbers will differ from that corpus.
/// - Debian is also the only C source that supplies a *popularity signal and
///   a source tarball for the same name*, which is why it beats vcpkg/Conan
///   (no download counts) here.
const POPCON: &str = "https://popcon.debian.org/source/by_inst";
const MIRROR: &str = "https://deb.debian.org/debian";
/// Unstable, not stable: treebank exists because grammars fall behind, so the
/// corpus should be the newest C the distro carries.
const SUITE: &str = "sid";
const SOURCES: &str = "https://deb.debian.org/debian/dists/sid/main/source/Sources.gz";

/// Debian pool coordinates for one source package, resolved at rank time.
/// `resolve()` gets no `db` path, so `rank()` leaves this index behind at a
/// fixed location for it to read — the same shape of arrangement as
/// `tools/*-oracle` being found relative to the repo root.
const INDEX: &str = "corpus/c/db/index.json";

#[derive(Serialize, Deserialize, Clone)]
struct Pool {
    version: String,
    directory: String,
    file: String,
}

static POOL: LazyLock<HashMap<String, Pool>> = LazyLock::new(|| {
    std::fs::read_to_string(INDEX)
        .ok()
        .and_then(|s| serde_json::from_str(&s).ok())
        .unwrap_or_default()
});

impl Lang for C {
    fn name(&self) -> LangName {
        LangName::C
    }

    fn rank(&self, db: &Path, k: usize) -> Result<Vec<RankedCrate>> {
        rank_debian(db, k)
    }

    /// Straight out of the pool, using the coordinates `rank()` recorded.
    /// The tarball is upstream's own release archive, so the corpus is
    /// upstream source — not Debian's patched tree (the `.debian.tar.xz`
    /// carrying the distro patches is deliberately not fetched).
    fn resolve(&self, pkg: &RankedCrate) -> Result<(String, String)> {
        let pool = POOL.get(&pkg.name).with_context(|| {
            format!("{}: not in {INDEX} — re-run `treebank rank --lang c`", pkg.name)
        })?;
        Ok((
            pool.version.clone(),
            format!("{MIRROR}/{}/{}", pool.directory, pool.file),
        ))
    }

    /// `.c` and `.h`. Headers are half of C and the half where declaration
    /// syntax lives — typedefs, bitfields, attributes, macros in declaration
    /// position — which is exactly what a C grammar gets wrong. `admit()`
    /// then drops the C++ ones.
    fn classify(&self, rel: &Path) -> Option<Option<String>> {
        matches!(rel.extension()?.to_str()?, "c" | "h").then_some(None)
    }

    /// `.h` is shared by C and C++ and the extension cannot tell them apart —
    /// the file-to-grammar routing problem `DESIGN.md` flags as unresolved.
    /// Measured reason to filter rather than let the oracle sort it out: a
    /// C++ header comes back **indeterminate**, not `invalid`, so unfiltered
    /// headers would inflate the one bucket whose size decides whether C is
    /// sweepable at all.
    fn admit(&self, rel: &Path, content: &[u8]) -> bool {
        if rel.extension().and_then(|e| e.to_str()) != Some("h") {
            return true;
        }
        // Directory naming is the cheapest signal.
        let dir = rel.parent().map(|p| p.to_string_lossy().to_lowercase()).unwrap_or_default();
        if dir.split('/').any(|c| matches!(c, "c++" | "cxx" | "cpp")) {
            return false;
        }
        !looks_like_cxx(content)
    }

    fn grammar_dirs(&self) -> &'static [&'static str] {
        &["."]
    }

    /// `tools/c-oracle`: libclang, parse-only, verdict from clang's own
    /// diagnostic categories. See `crates/treebank-c/ORACLE.md` for what it
    /// does and does not claim — the short version is "no syntax error, in
    /// GNU C, given these include paths", NOT "this compiles".
    ///
    /// The oracle is three-valued. `Lang::validate` is two-valued, and
    /// **indeterminate collapses to false**: no fix agent is ever dispatched
    /// at a file whose validity we cannot vouch for. That makes `gap_files` a
    /// floor and mixes indeterminates into `noise_files`, so the full split
    /// is printed here and written to `oracle-verdicts.json` beside the
    /// corpus. A C gap number quoted without its indeterminate count is not
    /// a claim this crate makes.
    fn validate(&self, srcroot: &Path, paths: &[String]) -> Result<HashMap<String, bool>> {
        let oracle = Path::new("tools/c-oracle/c-oracle");
        if !oracle.exists() {
            eprintln!("oracle: building tools/c-oracle (libclang)");
            let ok = Command::new("tools/c-oracle/build.sh")
                .status()
                .context("run tools/c-oracle/build.sh — run from the repo root")?
                .success();
            anyhow::ensure!(ok, "tools/c-oracle/build.sh failed");
        }

        let mut child = Command::new(oracle)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .spawn()
            .context("spawn tools/c-oracle/c-oracle")?;

        let requests: Vec<String> = paths
            .iter()
            .map(|p| {
                let full = srcroot.join(p);
                let mut args = vec![
                    "-std=gnu17".to_string(),
                    "-ferror-limit=0".to_string(),
                    "-w".to_string(),
                ];
                args.extend(include_dirs(srcroot, p));
                format!("{}\t{}", full.display(), args.join("\t"))
            })
            .collect();

        let mut stdin = child.stdin.take().context("oracle stdin")?;
        let writer = std::thread::spawn(move || -> std::io::Result<()> {
            for line in &requests {
                writeln!(stdin, "{line}")?;
            }
            stdin.flush()
        });

        let mut verdicts: Vec<serde_json::Value> = Vec::new();
        let mut map = HashMap::new();
        let mut counts: HashMap<String, usize> = HashMap::new();
        {
            let stdout = child.stdout.take().context("oracle stdout")?;
            for line in BufReader::new(stdout).lines() {
                let line = line?;
                let v: serde_json::Value = serde_json::from_str(&line)
                    .with_context(|| format!("c-oracle emitted non-JSON: {line}"))?;
                let verdict = v["verdict"].as_str().unwrap_or("error").to_string();
                let rel = Path::new(v["path"].as_str().unwrap_or_default())
                    .strip_prefix(srcroot)
                    .map(|r| r.to_string_lossy().into_owned())
                    .unwrap_or_else(|_| v["path"].as_str().unwrap_or_default().to_string());
                *counts.entry(verdict.clone()).or_default() += 1;
                if let Some(cat) = v["unknown_category"].as_str() {
                    eprintln!("oracle: unrecognised clang category {cat:?} on {rel} — check ORACLE.md");
                }
                map.insert(rel, verdict == "valid");
                verdicts.push(v);
            }
        }
        let status = child.wait()?;
        let _ = writer.join().map_err(|_| anyhow::anyhow!("oracle stdin thread panicked"))?;
        anyhow::ensure!(status.success(), "c-oracle exited with {status}");

        let get = |k: &str| counts.get(k).copied().unwrap_or(0);
        eprintln!(
            "oracle: {} valid, {} invalid, {} indeterminate, {} error (of {} adjudicated)",
            get("valid"),
            get("invalid"),
            get("indeterminate"),
            get("error"),
            paths.len()
        );
        if get("indeterminate") > get("valid") {
            eprintln!(
                "oracle: WARNING — more files are unadjudicable than are known-valid. \
                 gap_files is a floor; read it with the indeterminate count."
            );
        }
        if let Some(corpus) = srcroot.parent() {
            let sidecar = corpus.join("oracle-verdicts.json");
            std::fs::write(
                &sidecar,
                serde_json::to_string_pretty(&serde_json::json!({
                    "oracle": "libclang, parse-only, category rule (see ORACLE.md)",
                    "flags": ["-std=gnu17", "-ferror-limit=0", "-w",
                              "-iquote<package header dirs>", "-I<package public dirs>"],
                    "counts": counts,
                    "files": verdicts,
                }))?,
            )?;
            eprintln!("oracle: verdict detail at {}", sidecar.display());
        }
        Ok(map)
    }
}

/// Every directory in a package that holds a header, plus their ancestors up
/// to the package root. Computed once per package.
///
/// The ancestors matter as much as the leaves: a package that writes
/// `#include "util/bitscan.h"` needs `src/` on the path, not `src/util/`.
/// Measured need for the whole approach — a first pass using only the
/// conventional dirs (`include/`, `src/`, …) left 12,555 of 13,144
/// indeterminate verdicts carrying an unresolved include, and the misses were
/// overwhelmingly ordinary package-internal headers: systemd's
/// `src/basic/alloc-util.h`, mesa's `src/compiler/nir/nir.h`, krb5's
/// `src/include/k5-int.h`.
static PKG_INCLUDES: LazyLock<std::sync::Mutex<HashMap<String, std::sync::Arc<Vec<String>>>>> =
    LazyLock::new(|| std::sync::Mutex::new(HashMap::new()));

fn package_includes(srcroot: &Path, pkgdir: &str) -> std::sync::Arc<Vec<String>> {
    if let Some(hit) = PKG_INCLUDES.lock().unwrap().get(pkgdir) {
        return hit.clone();
    }
    let root = srcroot.join(pkgdir);
    let mut header_dirs: std::collections::BTreeSet<std::path::PathBuf> =
        std::collections::BTreeSet::new();
    let mut stack = vec![root.clone()];
    while let Some(dir) = stack.pop() {
        let Ok(entries) = std::fs::read_dir(&dir) else { continue };
        for entry in entries.flatten() {
            let path = entry.path();
            match entry.file_type() {
                Ok(ft) if ft.is_dir() => {
                    if path.file_name().is_some_and(|n| n == ".git") {
                        continue;
                    }
                    stack.push(path);
                }
                Ok(_) => {
                    if path.extension().is_some_and(|e| e == "h") {
                        // the dir itself, then every ancestor up to the root,
                        // so prefixed includes ("util/bitscan.h") resolve too
                        let mut at = path.parent().map(|p| p.to_path_buf());
                        while let Some(d) = at {
                            let keep = d.starts_with(&root) || d == root;
                            if !keep {
                                break;
                            }
                            if !header_dirs.insert(d.clone()) {
                                break; // ancestors already recorded
                            }
                            if d == root {
                                break;
                            }
                            at = d.parent().map(|p| p.to_path_buf());
                        }
                    }
                }
                Err(_) => {}
            }
        }
    }
    let dirs: Vec<String> = header_dirs.iter().map(|d| d.display().to_string()).collect();
    let arc = std::sync::Arc::new(dirs);
    PKG_INCLUDES.lock().unwrap().insert(pkgdir.to_string(), arc.clone());
    arc
}

/// Include flags for one corpus file.
///
/// The package's own header dirs go on with **`-iquote`, not `-I`** — that is
/// load-bearing and was measured, not assumed. `-I` is searched for
/// `#include <...>` as well as `"..."`, so putting a package's internal dirs
/// there lets its private replacements for system headers shadow the real
/// ones: glibc's `string/string.h` answering `<string.h>`, mesa's `util/`
/// answering `<util/…>`. Those private copies are written for their own build
/// environment and do not stand alone, so resolution got *worse* the wider
/// the `-I` list grew. `-iquote` applies only to the quoted form, which is how
/// package-internal headers are included in practice.
///
/// Measured on a random 1,500 of the 17,868 failing files:
///
/// | include flags                   | valid | invalid | indeterminate |
/// |---------------------------------|-------|---------|---------------|
/// | conventional dirs, `-I`         |   389 |      13 |          1098 |
/// | every header dir, `-I`          |   346 |      14 |          1140 |
/// | every header dir, `-iquote`     |   483 |      20 |           997 |
///
/// **No build system is run** — no `./configure`, no `cmake` — so a generated
/// `config.h` is simply absent and its absence shows up as an indeterminate
/// verdict rather than a fabricated one. Resolving more would mean executing
/// arbitrary upstream build scripts.
fn include_dirs(srcroot: &Path, rel: &str) -> Vec<String> {
    let mut flags = Vec::new();
    let full = srcroot.join(rel);
    if let Some(own) = full.parent() {
        flags.push(format!("-iquote{}", own.display()));
    }
    let Some(pkgdir) = rel.split('/').next() else { return flags };
    for d in package_includes(srcroot, pkgdir).iter() {
        flags.push(format!("-iquote{d}"));
    }
    // The conventional dirs additionally go on as -I, so that a package's
    // *public* headers answer angle-bracket includes of its own API.
    let root = srcroot.join(pkgdir);
    for sub in ["", "include", "inc", "src", "lib"] {
        let d = if sub.is_empty() { root.clone() } else { root.join(sub) };
        if d.is_dir() {
            flags.push(format!("-I{}", d.display()));
        }
    }
    flags
}

const CXX_MARKERS: [&str; 9] = [
    "namespace ",
    "template<",
    "template <",
    "class ",
    "public:",
    "private:",
    "protected:",
    "using namespace ",
    "extern \"C++\"",
];

/// Comments and string literals blanked, newlines preserved so that line
/// starts still mean something. Both exclusions were measured needs, not
/// hygiene: a first version scanned raw text and dropped `glibc/elf/elf.h`
/// over the words "class declaration." at the end of a block comment, and
/// `malloc/obstack.h` over "namespace with <stddef.h>'s symbols" on a GNU
/// comment continuation line, which carries no `*` prefix to skip on.
fn strip_comments_and_strings(text: &str) -> String {
    let b = text.as_bytes();
    let mut out = String::with_capacity(text.len());
    let mut i = 0;
    while i < b.len() {
        if b[i..].starts_with(b"/*") {
            let end = text[i + 2..].find("*/").map(|j| i + 2 + j + 2).unwrap_or(b.len());
            out.extend(text[i..end].chars().filter(|c| *c == '\n'));
            i = end;
        } else if b[i..].starts_with(b"//") {
            let end = text[i..].find('\n').map(|j| i + j).unwrap_or(b.len());
            i = end;
        } else if b[i] == b'"' || b[i] == b'\'' {
            let quote = b[i];
            out.push(' ');
            i += 1;
            while i < b.len() && b[i] != quote {
                i += if b[i] == b'\\' { 2 } else { 1 };
            }
            i += 1;
        } else {
            out.push(b[i] as char);
            i += 1;
        }
    }
    out
}

/// Is this header C++ rather than C? Only **unguarded** C++ counts. A great
/// many C headers carry C++ sections behind `#ifdef __cplusplus` — glibc's
/// `math.h` has `extern "C++" { template <class __T> …}` — and those are C
/// headers, so anything inside a conditional whose condition mentions
/// `__cplusplus` is skipped, both branches of it.
///
/// Deliberately blunt: a corpus filter, not a language detector. Measured on
/// the 20-package pilot it drops 365 of 12,767 headers (2.9%) — `ncurses/c++/`,
/// krb5's Windows MFC classes, glibc's `template<>` test fixtures.
fn looks_like_cxx(content: &[u8]) -> bool {
    let raw = String::from_utf8_lossy(&content[..content.len().min(200_000)]);
    let text = strip_comments_and_strings(&raw);
    // one entry per open conditional: does it mention __cplusplus?
    let mut guards: Vec<bool> = Vec::new();
    for line in text.lines() {
        let l = line.trim_start();
        if let Some(directive) = l.strip_prefix('#') {
            let d = directive.trim_start();
            let word = d.split_whitespace().next().unwrap_or("");
            match word {
                "if" | "ifdef" | "ifndef" => guards.push(d.contains("__cplusplus")),
                "else" | "elif" => {
                    if let Some(top) = guards.last_mut() {
                        *top = *top || d.contains("__cplusplus");
                    }
                }
                "endif" => {
                    guards.pop();
                }
                _ => {}
            }
            continue;
        }
        if guards.iter().any(|g| *g) {
            continue;
        }
        if CXX_MARKERS.iter().any(|m| l.starts_with(m)) {
            return true;
        }
    }
    false
}

/// popcon's per-source install counts, filtered to the packages that are
/// actually C, newest-version pool coordinates recorded for `resolve()`.
fn rank_debian(db: &Path, k: usize) -> Result<Vec<RankedCrate>> {
    std::fs::create_dir_all(db)?;

    // 1. popcon: rank, source name, installs.
    eprintln!("rank: GET {POPCON}");
    let popcon = ureq::get(POPCON)
        .call()
        .with_context(|| format!("GET {POPCON}"))?
        .into_string()?;
    let mut ranked: Vec<(String, u64)> = Vec::new();
    for line in popcon.lines() {
        if line.starts_with('#') || line.starts_with("---") {
            continue;
        }
        let f: Vec<&str> = line.split_whitespace().collect();
        // "<rank> <name> <inst> <vote> <old> <recent> <no-files>"; the file
        // also carries a "Not in sid" pseudo-entry and a Total line.
        if f.len() < 3 || f[0].parse::<u64>().is_err() || f[1] == "Total" {
            continue;
        }
        let Ok(inst) = f[2].parse::<u64>() else { continue };
        ranked.push((f[1].to_string(), inst));
    }
    eprintln!("rank: popcon lists {} source packages", ranked.len());

    // 2. the Sources index, for versions and pool paths.
    let pool = load_sources(db)?;
    eprintln!("rank: sid index has {} sources with an orig tarball", pool.len());

    // 3. Walk popcon top-down, keeping the ones that are really C.
    let mut out = Vec::new();
    let mut index: HashMap<String, Pool> = HashMap::new();
    let (mut not_in_sid, mut not_c) = (0usize, 0usize);
    for (name, inst) in &ranked {
        if out.len() >= k {
            break;
        }
        let Some(p) = pool.get(name) else {
            not_in_sid += 1;
            continue;
        };
        match is_c_package(name, &p.version) {
            Ok(true) => {}
            Ok(false) => {
                not_c += 1;
                continue;
            }
            Err(e) => {
                eprintln!("rank: skipping {name}: {e:#}");
                continue;
            }
        }
        out.push(RankedCrate {
            rank: out.len() + 1,
            name: name.clone(),
            version: p.version.clone(),
            downloads: *inst,
        });
        index.insert(name.clone(), p.clone());
        if out.len() % 25 == 0 {
            eprintln!("rank: {} C sources kept (at popcon rank {})", out.len(), out.len() + not_in_sid + not_c);
        }
    }
    if out.is_empty() {
        bail!("debian rank list came out empty");
    }
    std::fs::write(db.join("index.json"), serde_json::to_string_pretty(&index)?)?;
    eprintln!(
        "rank: kept {} C sources; skipped {not_c} non-C and {not_in_sid} not in {SUITE}",
        out.len()
    );
    Ok(out)
}

/// Newest stanza per source package in `dists/<suite>/main/source/Sources`,
/// cached under `db/`. 16 MB gzipped, one download.
fn load_sources(db: &Path) -> Result<HashMap<String, Pool>> {
    let cached = db.join("Sources.gz");
    if !cached.exists() {
        eprintln!("rank: GET {SOURCES}");
        let mut body = Vec::new();
        ureq::get(SOURCES)
            .call()
            .with_context(|| format!("GET {SOURCES}"))?
            .into_reader()
            .read_to_end(&mut body)?;
        std::fs::write(&cached, &body)?;
    }
    let gz = flate2::read::GzDecoder::new(std::fs::File::open(&cached)?);
    let mut pool: HashMap<String, Pool> = HashMap::new();
    let (mut pkg, mut version, mut directory) = (None, None, None);
    let mut origs: Vec<String> = Vec::new();
    let finish = |pkg: &mut Option<String>,
                      version: &mut Option<String>,
                      directory: &mut Option<String>,
                      origs: &mut Vec<String>,
                      pool: &mut HashMap<String, Pool>| {
        if let (Some(n), Some(v), Some(d)) = (pkg.take(), version.take(), directory.take()) {
            // Shortest name wins for multi-tarball sources: that is the main
            // one, the rest are `.orig-<component>.tar.*` add-ons.
            origs.sort_by_key(|f| (f.len(), f.clone()));
            if let Some(file) = origs.first().cloned() {
                let newer = pool
                    .get(&n)
                    .is_none_or(|old: &Pool| deb_version_lt(&old.version, &v));
                if newer {
                    pool.insert(n, Pool { version: v, directory: d, file });
                }
            }
        }
        origs.clear();
    };
    for line in BufReader::new(gz).lines() {
        let line = line?;
        if let Some(v) = line.strip_prefix("Package: ") {
            pkg = Some(v.trim().to_string());
        } else if let Some(v) = line.strip_prefix("Version: ") {
            version = Some(v.trim().to_string());
        } else if let Some(v) = line.strip_prefix("Directory: ") {
            directory = Some(v.trim().to_string());
        } else if line.starts_with(' ') && line.contains(".orig.tar.") {
            if let Some(f) = line.split_whitespace().last() {
                if f.ends_with(".tar.xz") || f.ends_with(".tar.gz") || f.ends_with(".tar.bz2") {
                    origs.push(f.to_string());
                }
            }
        } else if line.is_empty() {
            finish(&mut pkg, &mut version, &mut directory, &mut origs, &mut pool);
        }
    }
    finish(&mut pkg, &mut version, &mut directory, &mut origs, &mut pool);
    if pool.is_empty() {
        bail!("parsed no sources out of {}", cached.display());
    }
    Ok(pool)
}

/// Numeric-chunk comparison. Not dpkg's algorithm (no epoch/tilde
/// subtleties); it only has to pick the newest of a package's own stanzas.
fn deb_version_lt(a: &str, b: &str) -> bool {
    fn chunks(s: &str) -> Vec<(u64, String)> {
        let mut out = Vec::new();
        let mut num = String::new();
        let mut txt = String::new();
        for ch in s.chars() {
            if ch.is_ascii_digit() {
                if !txt.is_empty() {
                    out.push((0, std::mem::take(&mut txt)));
                }
                num.push(ch);
            } else {
                if !num.is_empty() {
                    out.push((num.parse().unwrap_or(0), String::new()));
                    num.clear();
                }
                txt.push(ch);
            }
        }
        if !num.is_empty() {
            out.push((num.parse().unwrap_or(0), String::new()));
        }
        if !txt.is_empty() {
            out.push((0, txt));
        }
        out
    }
    chunks(a) < chunks(b)
}

/// Is this source package actually C? popcon ranks everything Debian ships,
/// so without this the top of the list spends its downloads on LibreOffice
/// (4.4M lines of C++, 34k of C) and gcc-16 (no C at all). sources.debian.org
/// publishes per-language SLOC, which answers it in one small request.
fn is_c_package(name: &str, version: &str) -> Result<bool> {
    let url = format!("https://sources.debian.org/api/info/package/{name}/{version}/");
    let doc: serde_json::Value = ureq::get(&url)
        .call()
        .with_context(|| format!("GET {url}"))?
        .into_json()?;
    let sloc = doc["pkg_infos"]["sloc"]
        .as_array()
        .with_context(|| format!("{name} {version}: no sloc in sources.debian.org info"))?;
    let mut ansic = 0i64;
    let mut cpp = 0i64;
    for pair in sloc {
        match (pair[0].as_str(), pair[1].as_i64()) {
            (Some("ansic"), Some(n)) => ansic = n,
            (Some("cpp"), Some(n)) => cpp = n,
            _ => {}
        }
    }
    // Enough C to be worth a download, and more C than C++ so that the
    // C++ giants (Qt, LibreOffice) do not enter on their C fringe.
    Ok(ansic >= 2000 && ansic >= cpp)
}
