//! Running a file through the language's own FORMATTER.
//!
//! This is the sibling of [`crate::unparse`] and asks a different question,
//! which is worth stating because the tools look interchangeable and are
//! not. A printer renders from the tree and never consults the original
//! bytes, so it answers "do we handle the canonical spelling". A formatter
//! is text-to-text: it reflows a token stream it never stopped holding, so
//! it keeps comments and keeps the author's spelling, and what it changes is
//! layout.
//!
//! That makes it useless for the round trip and ideal for a different
//! invariant: **reformatting must not change our tree**. A formatter
//! preserves the program, so every node we produce before must be there
//! after, in the same order. Anything else is our bug — a rule that reads
//! layout it should not, a token that only lexes when it happens to abut its
//! neighbour.
//!
//! One trap, found the hard way: rustfmt REORDERS `use` declarations by
//! default, which legitimately changes the tree and made 43 of 98 files look
//! like failures. Reordering is switched off rather than accommodated,
//! because a check whose disagreements are mostly expected is a check nobody
//! reads.

use std::collections::HashMap;
use std::path::Path;

use anyhow::{Context, Result};
use rayon::prelude::*;
use serde::Deserialize;

use crate::LangName;

pub struct Reformatted {
    pub source: Option<String>,
    pub skipped: Option<String>,
}

pub trait Reformatter: Sync {
    fn reformat(&self, srcroot: &Path, paths: &[String]) -> Result<HashMap<String, Reformatted>>;
    /// What ran, for the report — the reader should not have to guess which
    /// formatter's opinion they are looking at.
    fn tool(&self) -> &'static str;
    /// Is the tool actually on this machine? Separate from whether the
    /// language HAS a formatter, which is what `capabilities` declares:
    /// rustfmt ships with the toolchain, black does not.
    fn available(&self) -> bool {
        true
    }
}

/// `None` where no formatter is available for the language — either
/// because the language has none we can drive (declared, with reasons, in
/// [`crate::capabilities`]) or because the one it has is not installed
/// here.
///
/// Python's is `black`, which unlike rustfmt is not part of the toolchain,
/// so it is probed for. The probe is not hedging about whether to depend on
/// it — CI installs it and the check is expected to run — it is so that a
/// machine without it gets a sentence naming the missing tool instead of a
/// subprocess failure per file.
pub fn get(name: LangName) -> Option<&'static dyn Reformatter> {
    crate::capabilities::get(name)
        .reformat
        .filter(|r| r.available())
}

fn which(bin: &str) -> Option<String> {
    let out = std::process::Command::new("sh")
        .arg("-c")
        .arg(format!("command -v {bin}"))
        .output()
        .ok()?;
    out.status
        .success()
        .then(|| String::from_utf8_lossy(&out.stdout).trim().to_string())
}

pub(crate) struct RustFmt;
pub(crate) struct BlackFmt;
pub(crate) struct TypeScriptFmt;
pub(crate) struct ZigFmt;

#[derive(Deserialize)]
struct BatchFormatted {
    path: String,
    #[serde(default)]
    source: Option<String>,
    #[serde(default)]
    skipped: Option<String>,
}

fn format_json_lines(
    tool_dir: &Path,
    script: &str,
    srcroot: &Path,
    paths: &[String],
) -> Result<HashMap<String, Reformatted>> {
    let lines = crate::stdin_oracle::node_lines(tool_dir, script, &[], srcroot, paths)?;
    lines
        .into_iter()
        .filter(|line| !line.trim().is_empty())
        .map(|line| {
            let value: BatchFormatted = serde_json::from_str(&line)
                .with_context(|| format!("parse formatter output: {line:.200}"))?;
            let path = crate::stdin_oracle::relativize(&value.path, srcroot);
            Ok((
                path,
                Reformatted {
                    source: value.source,
                    skipped: value.skipped,
                },
            ))
        })
        .collect()
}

/// Run a formatter over each file's TEXT, on stdin, taking the result from
/// stdout.
///
/// Stdin rather than a copy on disk, because rustfmt in file mode resolves
/// `mod x;` declarations against the directory it finds the file in — hand
/// it a copy in a scratch directory and it declines roughly one rust file
/// in seven for a reason that has nothing to do with that file. Stdin has
/// no directory to resolve against. (`--skip-children` would also do it,
/// and is nightly-only.)
fn format_stdin(
    srcroot: &Path,
    paths: &[String],
    argv: &[&str],
) -> Result<HashMap<String, Reformatted>> {
    use std::io::Write;
    Ok(paths
        .par_iter()
        .map(|rel| {
            let result = (|| -> Result<Reformatted> {
                let src = std::fs::read_to_string(srcroot.join(rel))
                    .with_context(|| format!("read {rel}"))?;
                let mut child = std::process::Command::new(argv[0])
                    .args(&argv[1..])
                    .stdin(std::process::Stdio::piped())
                    .stdout(std::process::Stdio::piped())
                    .stderr(std::process::Stdio::piped())
                    .spawn()?;
                child.stdin.take().unwrap().write_all(src.as_bytes()).ok();
                let out = child.wait_with_output()?;
                if !out.status.success() {
                    let why = String::from_utf8_lossy(&out.stderr);
                    let head = why.lines().next().unwrap_or("declined").trim().to_string();
                    return Ok(Reformatted {
                        source: None,
                        skipped: Some(head),
                    });
                }
                Ok(Reformatted {
                    source: Some(String::from_utf8_lossy(&out.stdout).into_owned()),
                    skipped: None,
                })
            })();
            match result {
                Ok(r) => (rel.clone(), r),
                Err(e) => (
                    rel.clone(),
                    Reformatted {
                        source: None,
                        skipped: Some(e.to_string()),
                    },
                ),
            }
        })
        .collect())
}

#[allow(dead_code)]
fn format_in_place(
    srcroot: &Path,
    paths: &[String],
    ext: &str,
    argv: &[&str],
) -> Result<HashMap<String, Reformatted>> {
    let tmp = std::env::temp_dir().join(format!("treebank-fmt-{}", std::process::id()));
    std::fs::create_dir_all(&tmp)?;
    let out = paths
        .par_iter()
        .enumerate()
        .map(|(i, rel)| {
            let scratch = tmp.join(format!("f{i}.{ext}"));
            let result = (|| -> Result<Reformatted> {
                let src = std::fs::read_to_string(srcroot.join(rel))
                    .with_context(|| format!("read {rel}"))?;
                std::fs::write(&scratch, &src)?;
                let mut cmd = std::process::Command::new(argv[0]);
                cmd.args(&argv[1..]).arg(&scratch);
                let status = cmd.output()?;
                if !status.status.success() {
                    let why = String::from_utf8_lossy(&status.stderr);
                    let head = why.lines().next().unwrap_or("declined").trim().to_string();
                    return Ok(Reformatted {
                        source: None,
                        skipped: Some(head),
                    });
                }
                Ok(Reformatted {
                    source: Some(std::fs::read_to_string(&scratch)?),
                    skipped: None,
                })
            })();
            let _ = std::fs::remove_file(&scratch);
            match result {
                Ok(r) => (rel.clone(), r),
                Err(e) => (
                    rel.clone(),
                    Reformatted {
                        source: None,
                        skipped: Some(e.to_string()),
                    },
                ),
            }
        })
        .collect();
    let _ = std::fs::remove_dir_all(&tmp);
    Ok(out)
}

impl Reformatter for RustFmt {
    fn tool(&self) -> &'static str {
        "rustfmt (reordering and semicolon insertion off)"
    }

    fn reformat(&self, srcroot: &Path, paths: &[String]) -> Result<HashMap<String, Reformatted>> {
        // reorder_imports/reorder_modules genuinely change the program's
        // node ORDER, which is not what this check is asking about.
        format_stdin(
            srcroot,
            paths,
            &[
                "rustfmt",
                "--edition",
                "2021",
                "--config",
                "reorder_imports=false,reorder_modules=false,trailing_semicolon=false",
            ],
        )
    }
}

impl Reformatter for BlackFmt {
    fn tool(&self) -> &'static str {
        "black"
    }

    fn reformat(&self, srcroot: &Path, paths: &[String]) -> Result<HashMap<String, Reformatted>> {
        format_stdin(srcroot, paths, &["black", "-q", "--fast", "-"])
    }

    fn available(&self) -> bool {
        which("black").is_some()
    }
}

impl Reformatter for TypeScriptFmt {
    fn tool(&self) -> &'static str {
        "TypeScript language service formatter"
    }

    fn reformat(&self, srcroot: &Path, paths: &[String]) -> Result<HashMap<String, Reformatted>> {
        format_json_lines(&crate::tool("ts-oracle"), "format.mjs", srcroot, paths)
    }

    fn available(&self) -> bool {
        which("node").is_some() && which("npm").is_some()
    }
}

impl Reformatter for ZigFmt {
    fn tool(&self) -> &'static str {
        "zig fmt"
    }

    fn reformat(&self, srcroot: &Path, paths: &[String]) -> Result<HashMap<String, Reformatted>> {
        format_stdin(srcroot, paths, &["zig", "fmt", "--stdin"])
    }

    fn available(&self) -> bool {
        which("zig").is_some()
    }
}
