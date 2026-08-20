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

/// One labelled parent -> child edge.
///
/// Spans say what is there; edges say how it is CONNECTED. Two trees can
/// agree on every node and still attach the children under different names,
/// and the names are what a consumer reads -- `orelse` versus `body` is the
/// difference between a program and its opposite, with every span and every
/// kind identical.
#[derive(Debug, Clone)]
pub struct Edge {
    pub parent: (usize, usize),
    pub parent_kind: String,
    pub field: String,
    pub child: (usize, usize),
}

/// What the oracle saw in one file.
#[derive(Debug, Clone, Default)]
pub struct FileSpans {
    pub spans: Vec<Span>,
    /// Empty when the oracle cannot report field names at all, which is not
    /// the same as a file having none — `has_edges` says which.
    pub edges: Vec<Edge>,
    /// Whether this oracle reports edges. `syn` does not: it has no generic
    /// field reflection, so a Rust node's children are positional to us.
    pub has_edges: bool,
    /// Token extents from a LEXICAL oracle, where one exists. CPython ships
    /// `tokenize` alongside `ast`; tsc and syn expose no separate token
    /// stream with positions, so they report none.
    pub tokens: Vec<(usize, usize)>,
    pub has_tokens: bool,
    /// Byte offset where the reference parser reported its FIRST error, when
    /// it rejected the file. Rejecting the right files at the wrong offset
    /// makes error recovery useless downstream, and nothing checks it.
    pub error: Option<usize>,
    /// Set when the oracle declined to report boundaries — its own parse
    /// errored, or it threw. Never silently an empty span list: an empty
    /// list means "this file has no nodes", which would pass the check
    /// vacuously and hide whatever went wrong.
    pub skipped: Option<String>,
}

pub trait SpanOracle: Sync {
    fn spans(&self, srcroot: &Path, paths: &[String]) -> Result<HashMap<String, FileSpans>>;
}

/// The languages whose oracle can report boundaries. Every one of them
/// has one now; the `Option` stays because the next language added will
/// not, and `None` is an honest answer where a silent no-op would not be.
///
/// Which languages those are is declared in [`crate::capabilities`],
/// alongside the other two things a toolchain may or may not be able to
/// do, so that a new language answers all three in one place.
pub fn get(name: LangName) -> Option<&'static dyn SpanOracle> {
    crate::capabilities::get(name).spans
}

pub(crate) struct TypeScriptSpans;
pub(crate) struct PythonSpans;
pub(crate) struct JavaSpans;
pub(crate) struct BashSpans;

#[derive(Deserialize)]
struct RawFile {
    path: String,
    #[serde(default)]
    spans: Vec<(usize, usize, String)>,
    #[serde(default)]
    edges: Vec<(usize, usize, String, String, usize, usize)>,
    /// Whether this oracle reports edges AT ALL. Absent means yes, which
    /// the two original span oracles both do; javac's TreeScanner has no
    /// generic field reflection, so its script says `false` explicitly
    /// rather than letting an empty list claim every file has no edges.
    #[serde(default)]
    has_edges: Option<bool>,
    #[serde(default)]
    tokens: Option<Vec<(usize, usize)>>,
    #[serde(default)]
    error: Option<usize>,
    #[serde(default)]
    skipped: Option<String>,
}

impl SpanOracle for TypeScriptSpans {
    fn spans(&self, srcroot: &Path, paths: &[String]) -> Result<HashMap<String, FileSpans>> {
        let lines =
            stdin_oracle::node_lines(&crate::tool("ts-oracle"), "spans.mjs", &[], srcroot, paths)?;
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

impl SpanOracle for JavaSpans {
    /// javac's own parser, via `Trees.getSourcePositions()` over
    /// `JavacTask.parse()`. Parse only — no analyze — so nothing synthetic
    /// exists and every reported node is something the file spells.
    fn spans(&self, srcroot: &Path, paths: &[String]) -> Result<HashMap<String, FileSpans>> {
        let script = crate::tool("java-oracle/Spans.java");
        let lines = stdin_oracle::run_lines(
            "java",
            &[script.to_string_lossy().as_ref()],
            "java tools/java-oracle/Spans.java — is a JDK (not just a JRE) installed?",
            srcroot,
            paths,
        )?;
        parse_jsonl(&lines, srcroot)
    }
}

impl SpanOracle for BashSpans {
    /// NOT the reference implementation. bash has no AST to ask for --
    /// `bash -n` is a verdict and nothing else -- so boundaries come from
    /// mvdan.cc/sh, an independent reimplementation, and this check is
    /// differential against a PEER. A disagreement is a place to look, not
    /// automatically our bug; the oracle program's header carries the full
    /// authority statement, and validity verdicts still come from bash.
    fn spans(&self, srcroot: &Path, paths: &[String]) -> Result<HashMap<String, FileSpans>> {
        // Built once per session rather than `go run` per batch: `go -C`
        // would also change the working directory, and the paths on stdin
        // are relative to ours.
        let dir = crate::tool("bash-oracle/spans");
        let bin = std::env::temp_dir().join("treebank-bash-spans");
        {
            static BUILT: std::sync::Once = std::sync::Once::new();
            BUILT.call_once(|| {
                let out = std::process::Command::new("go")
                    .args(["-C", dir.to_string_lossy().as_ref(), "build", "-o"])
                    .arg(&bin)
                    .arg(".")
                    .output()
                    .expect("run go build — is a Go toolchain installed?");
                if !out.status.success() {
                    panic!(
                        "go build tools/bash-oracle/spans failed:\n{}",
                        String::from_utf8_lossy(&out.stderr)
                    );
                }
            });
        }
        let lines = stdin_oracle::run_lines(
            bin.to_string_lossy().as_ref(),
            &[],
            "tools/bash-oracle/spans — go build produced no binary?",
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
                edges: raw
                    .edges
                    .into_iter()
                    .map(|(ps, pe, pk, field, cs, ce)| Edge {
                        parent: (ps, pe),
                        parent_kind: pk,
                        field,
                        child: (cs, ce),
                    })
                    .collect(),
                has_edges: raw.has_edges.unwrap_or(true),
                has_tokens: raw.tokens.is_some(),
                tokens: raw.tokens.unwrap_or_default(),
                error: raw.error,
                skipped: raw.skipped,
            },
        );
    }
    Ok(out)
}
