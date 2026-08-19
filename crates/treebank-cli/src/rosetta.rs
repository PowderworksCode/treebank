//! The rosetta gate (DESIGN.md §5.4): the same program, written in every
//! owned language, must yield the same role counts.
//!
//! This is the executable form of the promise that the shared vocabulary
//! means the same thing everywhere. It is the only check that catches a
//! role threaded in one grammar and forgotten in another — supertype
//! matching is derivation-based, so a missed thread produces no error,
//! just silence.
//!
//! Facet queries are expanded through the grammar's own `roles.json`
//! before running, so `(_callable)` is testable here exactly as a
//! consumer would use it through treebank-core.

use std::collections::BTreeMap;
use std::path::Path;

use anyhow::{bail, Context, Result};
use serde::Deserialize;

#[derive(Deserialize)]
struct Expected {
    #[serde(default)]
    note: String,
    queries: BTreeMap<String, usize>,
}

/// One language's participation in a rosetta case: which grammar crate
/// parses it, and what extension its program carries.
const LANGUAGES: &[(&str, &str)] = &[("python", "py"), ("rust", "rs"), ("typescript", "ts")];

pub fn run(dir: &Path, crates_dir: &Path) -> Result<()> {
    run_inner(dir, crates_dir, false)
}

/// `quiet` suppresses the summary line so `verify` can format its own.
pub fn run_quiet(dir: &Path, crates_dir: &Path) -> Result<()> {
    run_inner(dir, crates_dir, true)
}

fn run_inner(dir: &Path, crates_dir: &Path, quiet: bool) -> Result<()> {
    let mut cases: Vec<_> = std::fs::read_dir(dir)
        .with_context(|| format!("read {}", dir.display()))?
        .filter_map(|e| e.ok())
        .map(|e| e.path())
        .filter(|p| p.is_dir())
        .collect();
    cases.sort();
    if cases.is_empty() {
        bail!("no rosetta cases under {}", dir.display());
    }

    let mut failures = Vec::new();
    let mut checked = 0usize;

    for case in &cases {
        let name = case.file_name().unwrap().to_string_lossy().to_string();
        let expected: Expected = serde_json::from_str(
            &std::fs::read_to_string(case.join("expected.json"))
                .with_context(|| format!("read {}/expected.json", name))?,
        )
        .with_context(|| format!("parse {}/expected.json", name))?;
        let _ = &expected.note;

        for (lang, ext) in LANGUAGES {
            let program = case.join(format!("program.{ext}"));
            if !program.exists() {
                failures.push(format!(
                    "{name}: no program.{ext} — every owned language must \
                     participate in every case, or the case proves nothing"
                ));
                continue;
            }
            let grammar_dir = crates_dir.join(format!("treebank-{lang}"));
            let (language, _) = crate::grammar::load(&grammar_dir)?;
            let roles = treebank_core::roles::RolesManifest::load(&grammar_dir.join("roles.json"))?;
            let facets: BTreeMap<String, Vec<String>> = roles.facets.into_iter().collect();

            let source = std::fs::read_to_string(&program)?;
            let tree = {
                let mut parser = tree_sitter::Parser::new();
                parser.set_language(&language)?;
                parser
                    .parse(&source, None)
                    .with_context(|| format!("parse {}", program.display()))?
            };
            if tree.root_node().has_error() {
                failures.push(format!("{name}/{lang}: program does not parse cleanly"));
                continue;
            }

            for (query_src, want) in &expected.queries {
                let expanded = treebank_core::expand::expand(query_src, &facets)?;
                let query = tree_sitter::Query::new(&language, &expanded)
                    .with_context(|| format!("{name}/{lang}: bad query `{query_src}`"))?;
                let mut cursor = tree_sitter::QueryCursor::new();
                let got = {
                    use tree_sitter::StreamingIterator;
                    let mut matches = cursor.matches(&query, tree.root_node(), source.as_bytes());
                    let mut n = 0usize;
                    while let Some(m) = matches.next() {
                        n += m.captures.len();
                    }
                    n
                };
                checked += 1;
                if got != *want {
                    failures.push(format!(
                        "{name}/{lang}: `{query_src}` matched {got}, expected {want}"
                    ));
                }
            }
        }
    }

    for f in &failures {
        eprintln!("rosetta: {f}");
    }
    if !failures.is_empty() {
        bail!("{} rosetta assertion(s) failed", failures.len());
    }
    if !quiet {
        println!(
            "rosetta OK: {} case(s) × {} languages, {checked} assertions",
            cases.len(),
            LANGUAGES.len()
        );
    }
    Ok(())
}
