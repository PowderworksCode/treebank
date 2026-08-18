//! Re-rendering a file from the reference parser's own tree.
//!
//! Every other check reads the corpus as written. This one asks a question
//! the corpus cannot: whether we handle each construct in the form the
//! language's own tools EMIT, rather than only in the form its authors
//! happened to write.
//!
//! `ast.unparse` and `ts.createPrinter` both print in one canonical spelling
//! — no comments, normalised quotes and spacing, parentheses only where the
//! tree needs them. A construct we parse in its common spelling and not in
//! its canonical one is a real gap that no amount of real source will show,
//! because real source is written by people who write it the usual way.

use std::collections::HashMap;
use std::path::Path;

use anyhow::{Context, Result};
use serde::Deserialize;

use crate::{stdin_oracle, LangName};

pub struct Rendered {
    pub source: Option<String>,
    pub skipped: Option<String>,
}

pub trait Unparser: Sync {
    fn unparse(&self, srcroot: &Path, paths: &[String]) -> Result<HashMap<String, Rendered>>;
}

/// `None` where the language's toolchain has no printer we can drive.
pub fn get(name: LangName) -> Option<&'static dyn Unparser> {
    static PY: PythonUnparser = PythonUnparser;
    static TS: TypeScriptUnparser = TypeScriptUnparser;
    match name {
        LangName::Python => Some(&PY),
        LangName::Typescript | LangName::Javascript => Some(&TS),
        LangName::Rust => None,
    }
}

struct PythonUnparser;
struct TypeScriptUnparser;

#[derive(Deserialize)]
struct RawRendered {
    path: String,
    #[serde(default)]
    source: Option<String>,
    #[serde(default)]
    skipped: Option<String>,
}

fn decode(lines: &[String], srcroot: &Path) -> Result<HashMap<String, Rendered>> {
    let mut out = HashMap::new();
    for line in lines {
        if line.trim().is_empty() {
            continue;
        }
        let raw: RawRendered = serde_json::from_str(line)
            .with_context(|| format!("parse unparse output: {line:.200}"))?;
        out.insert(
            stdin_oracle::relativize(&raw.path, srcroot),
            Rendered { source: raw.source, skipped: raw.skipped },
        );
    }
    Ok(out)
}

impl Unparser for PythonUnparser {
    fn unparse(&self, srcroot: &Path, paths: &[String]) -> Result<HashMap<String, Rendered>> {
        let script = crate::tool("py-oracle/unparse.py");
        let lines = stdin_oracle::run_lines(
            "python3",
            &[script.to_string_lossy().as_ref()],
            "python3 tools/py-oracle/unparse.py — is python3 installed?",
            srcroot,
            paths,
        )?;
        decode(&lines, srcroot)
    }
}

impl Unparser for TypeScriptUnparser {
    fn unparse(&self, srcroot: &Path, paths: &[String]) -> Result<HashMap<String, Rendered>> {
        let lines = stdin_oracle::node_lines(
            &crate::tool("ts-oracle"),
            "unparse.mjs",
            &[],
            srcroot,
            paths,
        )?;
        decode(&lines, srcroot)
    }
}
