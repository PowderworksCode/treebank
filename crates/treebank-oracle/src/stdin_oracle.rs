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
    let out = node_lines(tool, "check.mjs", node_args, srcroot, paths)?;
    Ok(verdicts(&out, srcroot))
}

/// Run any script in a node oracle's tool directory and return its raw
/// stdout lines. The verdict oracles answer in `path\tvalid` pairs; the
/// span oracle answers in JSON, so the shared part stops at the lines.
pub fn node_lines(
    tool: &Path,
    script_name: &str,
    node_args: &[&str],
    srcroot: &Path,
    paths: &[String],
) -> Result<Vec<String>> {
    ensure_node_modules(tool)?;
    let script = tool.join(script_name);
    let mut args: Vec<&str> = node_args.to_vec();
    let script_str = script.to_string_lossy().into_owned();
    args.push(&script_str);
    run_lines(
        "node",
        &args,
        &format!("spawn node {}", script.display()),
        srcroot,
        paths,
    )
}

fn ensure_node_modules(tool: &Path) -> Result<()> {
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
    Ok(())
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
    let lines = run_lines(program, args, hint, srcroot, paths)?;
    Ok(verdicts(&lines, srcroot))
}

/// As `run`, but returns the oracle's raw stdout lines.
pub fn run_lines(
    program: &str,
    args: &[&str],
    hint: &str,
    srcroot: &Path,
    paths: &[String],
) -> Result<Vec<String>> {
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
    // stderr is inherited rather than piped, so the oracle's own diagnostics
    // have already reached the terminal; only the status is news here.
    anyhow::ensure!(output.status.success(), "{program} oracle exited with {}", output.status);

    Ok(String::from_utf8_lossy(&output.stdout)
        .lines()
        .map(|l| l.to_string())
        .collect())
}

/// Corpus-relative key for an absolute path the oracle echoed back.
pub fn relativize(path: &str, srcroot: &Path) -> String {
    Path::new(path)
        .strip_prefix(srcroot)
        .map(|r| r.to_string_lossy().into_owned())
        .unwrap_or_else(|_| path.to_string())
}

fn verdicts(lines: &[String], srcroot: &Path) -> HashMap<String, bool> {
    let mut map = HashMap::new();
    for line in lines {
        if let Some((path, verdict)) = line.rsplit_once('\t') {
            map.insert(relativize(path, srcroot), verdict == "valid");
        }
    }
    map
}

/// A long-lived oracle process, for callers that ask many small questions.
///
/// The batch helpers above spawn a process per call, which is right for the
/// sweep — one launch amortised over hundreds of thousands of files. It is
/// wrong for `fuzz`, which asks about one program at a time and then asks
/// again for every step of shrinking. Measured on java: 0.57s of fixed cost
/// per launch against 1.2ms per file, so a run spends its time starting
/// JVMs rather than parsing.
///
/// The protocol is a sentinel line in each direction. The caller writes
/// paths, then the sentinel; the oracle answers, then echoes the sentinel.
/// Paths are written from a separate thread because a batch can exceed the
/// pipe buffer, and a caller that writes everything before reading anything
/// deadlocks against an oracle doing the same.
pub struct Persistent {
    child: std::process::Child,
    stdin: Option<std::process::ChildStdin>,
    stdout: std::io::BufReader<std::process::ChildStdout>,
}

/// Chosen because no path contains it.
pub const SENTINEL: &str = "\u{0}--end--";

impl Persistent {
    pub fn spawn(program: &str, args: &[&str], hint: &str) -> Result<Persistent> {
        let mut child = Command::new(program)
            .args(args)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .spawn()
            .with_context(|| hint.to_string())?;
        let stdin = child.stdin.take().context("oracle stdin")?;
        let stdout =
            std::io::BufReader::new(child.stdout.take().context("oracle stdout")?);
        Ok(Persistent { child, stdin: Some(stdin), stdout })
    }

    pub fn ask(&mut self, srcroot: &Path, paths: &[String]) -> Result<HashMap<String, bool>> {
        use std::io::{BufRead, Write};

        let lines: Vec<String> = paths
            .iter()
            .map(|p| srcroot.join(p).display().to_string())
            .collect();
        let mut stdin = self.stdin.take().context("oracle stdin already closed")?;
        let writer = std::thread::spawn(move || -> std::io::Result<std::process::ChildStdin> {
            for line in &lines {
                writeln!(stdin, "{line}")?;
            }
            writeln!(stdin, "{SENTINEL}")?;
            stdin.flush()?;
            Ok(stdin)
        });

        let mut out = Vec::new();
        loop {
            let mut line = String::new();
            let n = self.stdout.read_line(&mut line)?;
            if n == 0 {
                anyhow::bail!("oracle exited mid-batch");
            }
            let line = line.trim_end_matches(['\n', '\r']).to_string();
            if line == SENTINEL {
                break;
            }
            out.push(line);
        }

        self.stdin = Some(
            writer
                .join()
                .map_err(|_| anyhow::anyhow!("oracle stdin thread panicked"))??,
        );
        Ok(verdicts(&out, srcroot))
    }
}

impl Drop for Persistent {
    fn drop(&mut self) {
        drop(self.stdin.take());
        let _ = self.child.wait();
    }
}

/// A named, lazily-started persistent oracle.
///
/// Every subprocess oracle here has the same shape and the same problem:
/// startup dominates when the caller asks small questions. This keeps one
/// process per (program, args) for the life of the run.
pub fn persistent(
    key: &'static str,
    program: &str,
    args: &[&str],
    hint: &str,
    srcroot: &Path,
    paths: &[String],
) -> Result<HashMap<String, bool>> {
    use std::sync::{Mutex, OnceLock};
    static POOL: OnceLock<Mutex<HashMap<&'static str, Persistent>>> = OnceLock::new();
    let pool = POOL.get_or_init(|| Mutex::new(HashMap::new()));
    let mut pool = pool.lock().map_err(|_| anyhow::anyhow!("oracle pool poisoned"))?;
    if !pool.contains_key(key) {
        pool.insert(key, Persistent::spawn(program, args, hint)?);
    }
    pool.get_mut(key).expect("just inserted").ask(srcroot, paths)
}
