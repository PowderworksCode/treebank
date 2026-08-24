//! The C and C++ oracle: one libclang program, two dialects.
//!
//! It is the only THREE-VALUED oracle here — valid / invalid /
//! indeterminate — and the reason is C's, not ours. `foo * bar;` is a
//! declaration or a multiplication depending on a typedef that arrives
//! through `#include`, so validity in isolation is not a question C has an
//! answer to. The verdict is relative to an include environment, and the
//! environment is part of the recorded evidence.
//!
//! `tools/c-oracle/ORACLE.md` carries the full argument, including the two
//! measured reasons `gcc -fsyntax-only` cannot do this job. The short
//! version: gcc terminates on a missing include and parses nothing after
//! it, and an undefined macro produces a genuine SYNTAX-class diagnostic
//! there, indistinguishable from a real one. libclang keeps going and
//! reports clang's own diagnostic CATEGORY, which is data rather than
//! prose.

use std::collections::{HashMap, HashSet};
use std::io::{BufRead, BufReader, Write};
use std::path::Path;
use std::process::{Command, Stdio};
use std::sync::{Arc, LazyLock, Mutex};

use anyhow::{Context, Result};

use crate::{LangName, Oracle};

pub struct C;
pub struct Cpp;

impl Oracle for C {
    fn name(&self) -> LangName {
        LangName::C
    }

    fn validate(&self, srcroot: &Path, paths: &[String]) -> Result<HashMap<String, bool>> {
        run(LangName::C, &["-x", "c", "-std=gnu17"], srcroot, paths)
    }
}

impl Oracle for Cpp {
    fn name(&self) -> LangName {
        LangName::Cpp
    }

    /// `gnu++17` rather than the newest standard clang knows, and that is a
    /// decision. C++20 and C++23 change what is *valid* in ways that reject
    /// older code — `throw()` exception specifications were removed in
    /// C++20 and are all over the corpus — so the newest dialect would book
    /// working library headers as invalid. C++17 is the dialect a
    /// distribution's C++ is actually written against.
    fn validate(&self, srcroot: &Path, paths: &[String]) -> Result<HashMap<String, bool>> {
        run(
            LangName::Cpp,
            &["-x", "c++", "-std=gnu++17"],
            srcroot,
            paths,
        )
    }
}

/// Ask the oracle about a batch, and report the three-valued split.
///
/// **Indeterminate collapses to `false`.** `Oracle::validate` is
/// two-valued and this is where the information is lost, deliberately: no
/// file whose validity we cannot vouch for is ever booked as a grammar gap.
/// The consequence is that `gap_files` is a FLOOR and `noise_files` mixes
/// "the reference rejected it" with "we could not tell", so the full split
/// is printed here and written beside the corpus. A C gap number quoted
/// without its indeterminate count is not a claim this crate makes.
fn run(
    lang: LangName,
    dialect: &[&str],
    srcroot: &Path,
    paths: &[String],
) -> Result<HashMap<String, bool>> {
    let oracle = ensure_oracle()?;

    let requests: Vec<String> = paths
        .iter()
        .map(|p| {
            let full = srcroot.join(p);
            let mut args: Vec<String> = dialect.iter().map(|s| s.to_string()).collect();
            args.push("-ferror-limit=0".to_string());
            args.push("-w".to_string());
            args.extend(include_dirs(srcroot, p));
            format!("{}\t{}", full.display(), args.join("\t"))
        })
        .collect();

    let verdicts = collect_verdicts(&oracle, &requests)?;

    let mut map = HashMap::new();
    let mut counts: HashMap<String, usize> = HashMap::new();
    for v in &verdicts {
        let verdict = v["verdict"].as_str().unwrap_or("error").to_string();
        let rel = crate::stdin_oracle::relativize(v["path"].as_str().unwrap_or_default(), srcroot);
        *counts.entry(verdict.clone()).or_default() += 1;
        if let Some(cat) = v["unknown_category"].as_str() {
            eprintln!("oracle: unrecognised clang category {cat:?} on {rel} — check ORACLE.md");
        }
        map.insert(rel, verdict == "valid");
    }

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
                "language": lang.as_str(),
                "oracle": "libclang, parse-only, category rule (see ORACLE.md)",
                "dialect": dialect,
                "counts": counts,
                "files": verdicts,
            }))?,
        )?;
        eprintln!("oracle: verdict detail at {}", sidecar.display());
    }
    Ok(map)
}

fn collect_verdicts(oracle: &Path, requests: &[String]) -> Result<Vec<serde_json::Value>> {
    let mut verdicts = Vec::new();
    let mut next = 0usize;
    let mut crash_retries = HashSet::new();
    while next < requests.len() {
        let mut child = Command::new(oracle)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .spawn()
            .with_context(|| format!("spawn {}", oracle.display()))?;

        let mut stdin = child.stdin.take().context("oracle stdin")?;
        let batch = requests[next..].to_vec();
        let writer = std::thread::spawn(move || -> std::io::Result<()> {
            for line in &batch {
                writeln!(stdin, "{line}")?;
            }
            stdin.flush()
        });

        let before = verdicts.len();
        {
            let stdout = child.stdout.take().context("oracle stdout")?;
            for line in BufReader::new(stdout).lines() {
                let line = line?;
                let v: serde_json::Value = serde_json::from_str(&line)
                    .with_context(|| format!("c-oracle emitted non-JSON: {line}"))?;
                verdicts.push(v);
            }
        }
        let status = child.wait()?;
        let writer_result = writer
            .join()
            .map_err(|_| anyhow::anyhow!("oracle stdin thread panicked"))?;
        let answered = verdicts.len() - before;
        next += answered;

        if status.success() {
            writer_result?;
            anyhow::ensure!(
                next == requests.len(),
                "c-oracle exited successfully after answering {next} of {} requests",
                requests.len()
            );
            break;
        }

        // libclang is native code operating on adversarially varied source.
        // A crash in one translation unit must not discard hours of prior
        // verdicts or silently bless that file. Output is ordered and flushed
        // per request, so the first unanswered request is the crash site.
        anyhow::ensure!(
            next < requests.len(),
            "c-oracle exited with {status} after answering every request"
        );
        let path = requests[next].split('\t').next().unwrap_or_default();
        if crash_retries.insert(path.to_string()) {
            eprintln!(
                "oracle: c-oracle exited with {status} while adjudicating {path}; \
                 retrying that file once in a fresh process"
            );
            continue;
        }
        eprintln!(
            "oracle: c-oracle exited with {status} again on {path}; \
             recording that file as indeterminate and continuing"
        );
        verdicts.push(serde_json::json!({
            "path": path,
            "verdict": "indeterminate",
            "oracle_crash": true,
            "detail": format!("c-oracle exited with {status} twice"),
        }));
        next += 1;
        // A closed stdin pipe is expected after a native crash.
        let _ = writer_result;
    }
    Ok(verdicts)
}

fn ensure_oracle() -> Result<std::path::PathBuf> {
    let oracle = crate::tool("c-oracle/c-oracle");
    let source = crate::tool("c-oracle/oracle.c");
    let build = crate::tool("c-oracle/build.sh");
    let stale = || -> Result<bool> {
        if !oracle.exists() {
            return Ok(true);
        }
        let built = oracle.metadata()?.modified()?;
        Ok(source.metadata()?.modified()? > built || build.metadata()?.modified()? > built)
    };
    if stale()? {
        eprintln!("oracle: building {} (libclang)", build.display());
        let ok = Command::new(&build)
            .status()
            .with_context(|| format!("run {} — is libclang installed?", build.display()))?
            .success();
        anyhow::ensure!(ok, "{} failed", build.display());
    }
    Ok(oracle)
}

pub(crate) fn span_lines(lang: LangName, srcroot: &Path, paths: &[String]) -> Result<Vec<String>> {
    let dialect: &[&str] = match lang {
        LangName::C => &["-x", "c", "-std=gnu17"],
        LangName::Cpp => &["-x", "c++", "-std=gnu++17"],
        _ => anyhow::bail!("libclang spans requested for {lang}"),
    };
    let oracle = ensure_oracle()?;
    let requests: Vec<String> = paths
        .iter()
        .map(|p| {
            let full = srcroot.join(p);
            let mut args: Vec<String> = dialect.iter().map(|s| s.to_string()).collect();
            args.push("-ferror-limit=0".to_string());
            args.push("-w".to_string());
            args.extend(include_dirs(srcroot, p));
            format!("{}\t{}", full.display(), args.join("\t"))
        })
        .collect();
    crate::stdin_oracle::run_lines(
        oracle.to_string_lossy().as_ref(),
        &["--spans"],
        "c-oracle --spans — is libclang installed?",
        Path::new(""),
        &requests,
    )
}

/// Every directory in a package that holds a header, plus their ancestors
/// up to the package root. Computed once per package.
///
/// The ancestors matter as much as the leaves: a package that writes
/// `#include "util/bitscan.h"` needs `src/` on the path, not `src/util/`.
/// Measured need for the whole approach — a first pass using only the
/// conventional dirs (`include/`, `src/`, …) left 12,555 of 13,144
/// indeterminate verdicts carrying an unresolved include, and the misses
/// were overwhelmingly ordinary package-internal headers: systemd's
/// `src/basic/alloc-util.h`, mesa's `src/compiler/nir/nir.h`, krb5's
/// `src/include/k5-int.h`.
static PKG_INCLUDES: LazyLock<Mutex<HashMap<String, Arc<Vec<String>>>>> =
    LazyLock::new(|| Mutex::new(HashMap::new()));

fn package_includes(srcroot: &Path, pkgdir: &str) -> Arc<Vec<String>> {
    if let Some(hit) = PKG_INCLUDES.lock().unwrap().get(pkgdir) {
        return hit.clone();
    }
    let root = srcroot.join(pkgdir);
    let mut header_dirs: std::collections::BTreeSet<std::path::PathBuf> =
        std::collections::BTreeSet::new();
    let mut stack = vec![root.clone()];
    while let Some(dir) = stack.pop() {
        let Ok(entries) = std::fs::read_dir(&dir) else {
            continue;
        };
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
                    if is_header(&path) {
                        // the dir itself, then every ancestor up to the
                        // root, so prefixed includes ("util/bitscan.h")
                        // resolve too
                        let mut at = path.parent().map(|p| p.to_path_buf());
                        while let Some(d) = at {
                            if !(d.starts_with(&root) || d == root) {
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
    let dirs: Vec<String> = header_dirs
        .iter()
        .map(|d| d.display().to_string())
        .collect();
    let arc = Arc::new(dirs);
    PKG_INCLUDES
        .lock()
        .unwrap()
        .insert(pkgdir.to_string(), arc.clone());
    arc
}

/// C++ headers are as often extensionless (`<vector>`) as suffixed, so the
/// test is broader than `.h` — but an extensionless file is only taken as a
/// header if it sits in a directory that already holds one, which the
/// directory walk gives for free.
fn is_header(path: &Path) -> bool {
    match path.extension().and_then(|e| e.to_str()) {
        Some(e) => matches!(
            e,
            "h" | "hpp" | "hh" | "hxx" | "h++" | "inl" | "ipp" | "tcc"
        ),
        None => false,
    }
}

#[cfg(all(test, unix))]
mod tests {
    use super::collect_verdicts;
    use std::os::unix::fs::PermissionsExt;

    #[test]
    fn a_repeat_native_crash_is_one_indeterminate_verdict_not_a_lost_batch() {
        let dir = std::env::temp_dir().join(format!(
            "treebank-c-oracle-crash-test-{}",
            std::process::id()
        ));
        std::fs::create_dir_all(&dir).unwrap();
        let oracle = dir.join("fake-oracle.sh");
        std::fs::write(
            &oracle,
            r#"#!/usr/bin/env bash
while IFS=$'\t' read -r path _; do
  if [[ "$path" == crash ]]; then
    kill -s SEGV "$$"
  fi
  printf '{"path":"%s","verdict":"valid"}\n' "$path"
done
"#,
        )
        .unwrap();
        let mut permissions = std::fs::metadata(&oracle).unwrap().permissions();
        permissions.set_mode(0o755);
        std::fs::set_permissions(&oracle, permissions).unwrap();

        let requests = vec!["before".into(), "crash".into(), "after".into()];
        let verdicts = collect_verdicts(&oracle, &requests).unwrap();
        assert_eq!(verdicts.len(), 3);
        assert_eq!(verdicts[0]["path"], "before");
        assert_eq!(verdicts[0]["verdict"], "valid");
        assert_eq!(verdicts[1]["path"], "crash");
        assert_eq!(verdicts[1]["verdict"], "indeterminate");
        assert_eq!(verdicts[1]["oracle_crash"], true);
        assert_eq!(verdicts[2]["path"], "after");
        assert_eq!(verdicts[2]["verdict"], "valid");

        std::fs::remove_dir_all(dir).unwrap();
    }
}

/// Include flags for one corpus file.
///
/// The package's own header dirs go on with **`-iquote`, not `-I`** — that
/// is load-bearing and was measured, not assumed. `-I` is searched for
/// `#include <...>` as well as `"..."`, so putting a package's internal
/// dirs there lets its private replacements for system headers shadow the
/// real ones: glibc's `string/string.h` answering `<string.h>`, mesa's
/// `util/` answering `<util/…>`. Those private copies are written for their
/// own build environment and do not stand alone, so resolution got *worse*
/// the wider the `-I` list grew. `-iquote` applies only to the quoted form,
/// which is how package-internal headers are included in practice.
///
/// Measured on a random 1,500 of 17,868 failing files:
///
/// | include flags                     | valid | invalid | indet. |
/// |-----------------------------------|-------|---------|--------|
/// | conventional dirs, `-I`           |   389 |      13 |   1098 |
/// | every header dir, `-I`            |   346 |      14 |   1140 |
/// | every header dir, `-iquote`       |   483 |      20 |    997 |
/// | `-iquote` + conventional `-I`     |   372 |      11 |   1117 |
/// | the same + `-idirafter` (what we do) | 453 |   37 |   1010 |
///
/// **No build system is run** — no `./configure`, no `cmake` — so a
/// generated `config.h` is simply absent, and its absence shows up as an
/// indeterminate verdict rather than a fabricated one. Resolving more would
/// mean executing arbitrary upstream build scripts.
fn include_dirs(srcroot: &Path, rel: &str) -> Vec<String> {
    let mut flags = Vec::new();
    let full = srcroot.join(rel);
    if let Some(own) = full.parent() {
        flags.push(format!("-iquote{}", own.display()));
    }
    let Some(pkgdir) = rel.split('/').next() else {
        return flags;
    };
    for d in package_includes(srcroot, pkgdir).iter() {
        flags.push(format!("-iquote{d}"));
    }
    // The conventional dirs additionally go on as -I, so that a package's
    // *public* headers answer angle-bracket includes of its own API.
    let root = srcroot.join(pkgdir);
    for sub in ["", "include", "inc", "src", "lib"] {
        let d = if sub.is_empty() {
            root.clone()
        } else {
            root.join(sub)
        };
        if d.is_dir() {
            flags.push(format!("-I{}", d.display()));
        }
    }
    // Packages also include their own *internal* headers with angle
    // brackets — glibc's `#include <sigsetops.h>`, which `-iquote` will not
    // answer. `-idirafter` is searched AFTER the system directories, so it
    // supplies only headers the system does not have: `<string.h>` still
    // resolves to the real one, `<sigsetops.h>` to glibc's sysdeps copy.
    // This is the flag that makes the wide list safe; plain `-I` is not.
    for d in package_includes(srcroot, pkgdir).iter() {
        flags.push(format!("-idirafter{d}"));
    }
    flags
}
