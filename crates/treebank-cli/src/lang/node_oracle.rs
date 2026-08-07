//! Shared driver for the node-based reference parsers under `tools/`.
//!
//! Each oracle is a script that reads one path per line on stdin and writes
//! "<path>\tvalid|invalid" per line on stdout. Deps are installed with
//! `npm ci` on first use, from the lockfile committed next to the script.

use std::collections::HashMap;
use std::io::Write;
use std::path::Path;
use std::process::{Command, Stdio};

use anyhow::{Context, Result};

/// Run `tool/check.mjs` over `paths` (relative to `srcroot`), returning the
/// reference parser's verdict per path.
pub fn run(
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
    let mut child = Command::new("node")
        .args(node_args)
        .arg(&script)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .with_context(|| format!("spawn node {}", script.display()))?;

    // Feed stdin from a thread: a large batch's output would otherwise fill
    // the stdout pipe and deadlock us before we finish writing.
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
    // A closed pipe here just means the oracle exited early; the status
    // check below is the real error report.
    let _ = writer.join().map_err(|_| anyhow::anyhow!("oracle stdin thread panicked"))?;
    anyhow::ensure!(output.status.success(), "{} failed", script.display());

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
