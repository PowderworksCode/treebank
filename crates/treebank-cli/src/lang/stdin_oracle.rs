//! Shared driver for the line-oriented reference parsers under `tools/`.
//!
//! Each oracle is a script that reads one path per line on stdin and writes
//! "<path>\tvalid|invalid" per line on stdout. That contract is the same
//! whichever interpreter runs it, so the pumping — feed stdin from a thread
//! so a large batch cannot deadlock against a full stdout pipe, then parse
//! the verdicts back — lives here once.
//!
//! `node` oracles additionally need their deps installed with `npm ci` on
//! first use, from the lockfile committed next to the script; `run_node`
//! wraps that. `run` is the bare form, for interpreters that need nothing
//! installed.

use std::collections::HashMap;
use std::io::Write;
use std::path::Path;
use std::process::{Command, Stdio};

use anyhow::{Context, Result};

/// Run `tool/check.mjs` under node, installing its deps on first use.
pub fn run_node(
    tool: &Path,
    node_args: &[&str],
    srcroot: &Path,
    paths: &[String],
) -> Result<HashMap<String, bool>> {
    if !tool.join("node_modules").exists() {
        eprintln!("oracle: installing {} deps (npm ci)", tool.display());
        let ok = Command::new("npm")
            .args(["ci", "--no-audit", "--no-fund"])
            .current_dir(tool)
            .status()
            .with_context(|| format!("run npm ci in {}", tool.display()))?
            .success();
        anyhow::ensure!(ok, "npm ci failed in {}", tool.display());
    }
    let script = tool.join("check.mjs");
    let mut args: Vec<&str> = node_args.to_vec();
    let script_str = script.to_string_lossy().into_owned();
    args.push(&script_str);
    run(
        "node",
        &args,
        &format!("spawn node {}", script.display()),
        srcroot,
        paths,
    )
}

/// Run `program args...` as an oracle over `paths` (relative to `srcroot`),
/// returning the reference parser's verdict per path. `hint` is the context
/// shown if the process cannot be spawned at all, which is where a missing
/// interpreter surfaces.
pub fn run(
    program: &str,
    args: &[&str],
    hint: &str,
    srcroot: &Path,
    paths: &[String],
) -> Result<HashMap<String, bool>> {
    run_configured(program, args, hint, srcroot, paths, |_| Vec::new())
}

/// The same, for a reference parser whose answer depends on configuration
/// that is not in the file.
///
/// `configure` returns the flags for one corpus-relative path, and they go
/// on the request line after a tab: `<path>\t<flag>\t<flag>…`. The reply is
/// unchanged (`<path>\tvalid|invalid`), so everything downstream is the same.
///
/// This exists because that class of language is not rare, and each member
/// of it was about to grow its own copy of this plumbing:
///
/// - **C** already has one, inline in `c.rs`: a file's validity depends on
///   the include paths it is compiled with, so it sends `-iquote…`/`-I…` per
///   file. It keeps its own copy for now because its oracle is three-valued
///   and speaks JSON rather than this two-valued line protocol.
/// - **Haskell** needs it because GHC's parser is configured by `LANGUAGE`
///   extensions that real packages declare in the `.cabal` file rather than
///   in the source. Measured on 5,631 files from the top 40 Hackage
///   packages: 575 of them (10.2%) change verdict when their package's
///   configuration is applied, and every one of those changes is
///   invalid → valid.
/// - **Scala** is next: scalameta requires the dialect (2.13 vs 3) to be
///   declared per file, and nothing in the path tells you which.
///
/// The configuration itself is per LANGUAGE and belongs in that language's
/// module; what is shared is only this — the request shape, and the rule
/// that a per-file flag list is derived from the package, memoized per
/// package, and never guessed from the file's own text.
pub fn run_configured(
    program: &str,
    args: &[&str],
    hint: &str,
    srcroot: &Path,
    paths: &[String],
    configure: impl Fn(&str) -> Vec<String>,
) -> Result<HashMap<String, bool>> {
    let mut child = Command::new(program)
        .args(args)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .with_context(|| hint.to_string())?;

    // Feed stdin from a thread: a large batch's output would otherwise fill
    // the stdout pipe and deadlock us before we finish writing.
    let mut stdin = child.stdin.take().context("oracle stdin")?;
    let lines: Vec<String> = paths
        .iter()
        .map(|p| {
            let mut line = srcroot.join(p).display().to_string();
            for flag in configure(p) {
                line.push('\t');
                line.push_str(&flag);
            }
            line
        })
        .collect();
    let writer = std::thread::spawn(move || -> std::io::Result<()> {
        for line in &lines {
            writeln!(stdin, "{line}")?;
        }
        stdin.flush()
    });

    let output = child.wait_with_output()?;
    // A closed pipe here just means the oracle exited early; the status
    // check below is the real error report.
    let _ = writer.join().map_err(|_| anyhow::anyhow!("oracle stdin thread panicked"))?;
    // stderr is inherited rather than piped, so the oracle's own diagnostics
    // have already reached the terminal; only the status is news here.
    anyhow::ensure!(output.status.success(), "{program} oracle exited with {}", output.status);

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
