//! Node BOUNDARIES from a reference parser.
//!
//! The sweep asks its oracles one question — "is this file valid?" — and a
//! yes/no answer can only ever catch one kind of defect: the grammar
//! rejecting code it should accept. It is structurally blind to the other
//! kind, where the grammar accepts a file and builds the WRONG TREE for it.
//! Those parse cleanly, sweep cleanly, and ship.
//!
//! Every silent mis-parse found in this repository so far was found by
//! accident, from an adjacent file where the wrong reading happened to be
//! illegal: `x as A & B` parsed as `(x as A) & B` corpus-wide and surfaced
//! only because `x as A & { c?: B }` put a `?` where an object literal
//! cannot have one.
//!
//! This is the systematic version. The reference parser already builds a
//! tree; the sweep throws it away. Keep the node BOUNDARIES from it and one
//! property becomes checkable over the whole corpus:
//!
//!   for every node the oracle reports, our tree has a node with exactly
//!   that byte span.
//!
//! Deliberately about boundaries and not NAMES. Comparing names needs a
//! correspondence table per language, which is where this kind of check
//! usually dies — the table is large, subjective, and rots. Boundaries need
//! no table at all: if tsc says something spans 15..20 and we have no node
//! there, we disagree about the shape of the code, whatever either of us
//! calls it.
//!
//! The check is one-directional on purpose. Our tree may have nodes the
//! oracle does not (finer granularity is fine, and normal). What it may not
//! do is fail to see a boundary the reference parser sees.

use std::collections::HashMap;
use std::path::Path;

use anyhow::{Context, Result};
use serde::Deserialize;

use crate::{stdin_oracle, LangName};

/// One node boundary, in BYTES — tsc counts UTF-16 code units and
/// tree-sitter counts bytes, so the conversion happens in the oracle script
/// where the string is already decoded.
#[derive(Debug, Clone, Deserialize)]
pub struct Span {
    pub start: usize,
    pub end: usize,
    pub kind: String,
}

/// What the oracle saw in one file.
#[derive(Debug, Clone, Default)]
pub struct FileSpans {
    pub spans: Vec<Span>,
    /// Set when the oracle declined to report boundaries — its own parse
    /// errored, or it threw. Never silently an empty span list: an empty
    /// list means "this file has no nodes", which would pass the check
    /// vacuously and hide whatever went wrong.
    pub skipped: Option<String>,
}

pub trait SpanOracle: Sync {
    fn spans(&self, srcroot: &Path, paths: &[String]) -> Result<HashMap<String, FileSpans>>;
}

/// The languages whose oracle can report boundaries. Every one of them has
/// one now; the `Option` stays because the next language added will not,
/// and `None` is an honest answer where a silent no-op would not be.
pub fn get(name: LangName) -> Option<&'static dyn SpanOracle> {
    static TS: TypeScriptSpans = TypeScriptSpans;
    static PY: PythonSpans = PythonSpans;
    static RS: crate::rust_spans::RustSpans = crate::rust_spans::RustSpans;
    match name {
        LangName::Typescript | LangName::Javascript => Some(&TS),
        LangName::Python => Some(&PY),
        LangName::Rust => Some(&RS),
    }
}

struct TypeScriptSpans;
struct PythonSpans;

#[derive(Deserialize)]
struct RawFile {
    path: String,
    #[serde(default)]
    spans: Vec<(usize, usize, String)>,
    #[serde(default)]
    skipped: Option<String>,
}

impl SpanOracle for TypeScriptSpans {
    fn spans(&self, srcroot: &Path, paths: &[String]) -> Result<HashMap<String, FileSpans>> {
        let lines = stdin_oracle::node_lines(
            &crate::tool("ts-oracle"),
            "spans.mjs",
            &[],
            srcroot,
            paths,
        )?;
        parse_jsonl(&lines, srcroot)
    }
}

impl SpanOracle for PythonSpans {
    /// CPython 3 only. `validate` unions py3 with py2.7 because a union
    /// grammar must be judged against every version it claims, but py2 has
    /// no `ast` we can ask for positions and a py2-only file has no py3
    /// tree to compare against — those come back `skipped`, which is
    /// honest, rather than counted as agreement.
    fn spans(&self, srcroot: &Path, paths: &[String]) -> Result<HashMap<String, FileSpans>> {
        let script = crate::tool("py-oracle/spans.py");
        let lines = stdin_oracle::run_lines(
            "python3",
            &[script.to_string_lossy().as_ref()],
            "python3 tools/py-oracle/spans.py — is python3 installed?",
            srcroot,
            paths,
        )?;
        parse_jsonl(&lines, srcroot)
    }
}

/// Both span oracles answer in the same JSON-lines shape, so the decoding is
/// shared: one object per file, spans as `[start, end, kind]` triples.
fn parse_jsonl(lines: &[String], srcroot: &Path) -> Result<HashMap<String, FileSpans>> {
    let mut out = HashMap::new();
    for line in lines {
        if line.trim().is_empty() {
            continue;
        }
        let raw: RawFile = serde_json::from_str(line)
            .with_context(|| format!("parse span oracle output: {line:.200}"))?;
        out.insert(
            stdin_oracle::relativize(&raw.path, srcroot),
            FileSpans {
                spans: raw
                    .spans
                    .into_iter()
                    .map(|(start, end, kind)| Span { start, end, kind })
                    .collect(),
                skipped: raw.skipped,
            },
        );
    }
    Ok(out)
}
