//! What the corpus never shows us (`treebank kinds`).
//!
//! The sweep is only as good as the code it reads. A construct no corpus
//! file contains is a construct the oracle has never been asked about, so a
//! bug there is invisible to every check that starts from real source — the
//! sweep, `mutate`, `roundtrip` and `reformat` alike. Fuzzing is the only
//! thing that can reach it.
//!
//! That makes "which node kinds does real code never produce" the question
//! worth asking, and it is cheap to answer: parse the corpus with our own
//! grammar and count. No oracle is involved, because this measures the
//! CORPUS rather than the grammar.
//!
//! The output is a budget, not a score. Kinds with millions of occurrences
//! are thoroughly checked already and fuzzing them again buys nothing;
//! kinds with none are where the marginal value of a generated program is.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use anyhow::Result;
use rayon::prelude::*;
use serde::Serialize;
use tree_sitter::{Node, Parser};

use treebank_corpus::fetch::Manifest;
use treebank_lang::LangName;

use crate::grammar;

fn count_kinds(root: Node, out: &mut BTreeMap<u16, u64>) {
    let mut cursor = root.walk();
    let mut descend = true;
    loop {
        if descend {
            let n = cursor.node();
            if n.is_named() {
                *out.entry(n.kind_id()).or_insert(0) += 1;
            }
        }
        if descend && cursor.goto_first_child() {
            continue;
        }
        if cursor.goto_next_sibling() {
            descend = true;
            continue;
        }
        loop {
            if !cursor.goto_parent() {
                return;
            }
            if cursor.goto_next_sibling() {
                descend = true;
                break;
            }
        }
    }
}

#[derive(Serialize)]
pub struct KindsReport {
    pub lang: String,
    pub files_parsed: usize,
    pub named_kinds_total: usize,
    pub named_kinds_seen: usize,
    /// Node kinds the grammar can produce that the corpus never did. These
    /// are the blind spot: no oracle has ever been asked about them.
    pub never_seen: Vec<String>,
    /// Kinds seen fewer than `THIN` times — checked, but barely.
    pub thin: Vec<(String, u64)>,
    /// The whole table, for anything that wants to weight by it.
    pub counts: BTreeMap<String, u64>,
}

/// Below this a kind is "thin": present, but on so few files that one
/// unusual spelling of it could still be unchecked.
const THIN: u64 = 20;

pub fn run(
    lang: LangName,
    grammar_dir: &Path,
    manifest_path: &Path,
    limit: Option<usize>,
    out_path: &Path,
) -> Result<()> {
    let manifest = Manifest::load(manifest_path)?;
    let corpus_src = manifest_path.parent().unwrap_or(Path::new(".")).join("src");
    let mut entries = manifest.files();
    entries.sort_by(|a, b| (&a.pkgdir, &a.rel).cmp(&(&b.pkgdir, &b.rel)));
    if let Some(n) = limit {
        entries.truncate(n);
    }

    let dirs = crate::routing::grammar_dirs(lang);
    let langs: Vec<tree_sitter::Language> = dirs
        .iter()
        .map(|d| grammar::load(&grammar_dir.join(d)).map(|(l, _)| l))
        .collect::<Result<_>>()?;

    println!("kinds: {} files — what does real {lang} never contain?", entries.len());

    let per_file: Vec<(usize, BTreeMap<u16, u64>)> = entries
        .par_iter()
        .map(|f| -> Result<(usize, BTreeMap<u16, u64>)> {
            let rel = format!("{}/{}", f.pkgdir, f.rel);
            let Ok(src) = std::fs::read(corpus_src.join(&rel)) else {
                return Ok((0, BTreeMap::new()));
            };
            let idx = crate::routing::route(lang, &f.dialect, &f.rel);
            let mut parser = Parser::new();
            parser.set_language(&langs[idx])?;
            let Some(tree) = parser.parse(&src, None) else {
                return Ok((0, BTreeMap::new()));
            };
            let mut counts = BTreeMap::new();
            count_kinds(tree.root_node(), &mut counts);
            Ok((1, counts))
        })
        .collect::<Result<_>>()?;

    let mut files_parsed = 0;
    let mut totals: BTreeMap<u16, u64> = BTreeMap::new();
    for (n, counts) in per_file {
        files_parsed += n;
        for (k, v) in counts {
            *totals.entry(k).or_insert(0) += v;
        }
    }

    // Every named kind the grammar can produce, from the language itself
    // rather than from node-types.json, so the two cannot drift.
    let ts_lang = &langs[0];
    let mut named: Vec<(u16, String)> = Vec::new();
    for id in 0..ts_lang.node_kind_count() as u16 {
        if ts_lang.node_kind_is_named(id) {
            if let Some(name) = ts_lang.node_kind_for_id(id) {
                if !name.starts_with('_') {
                    named.push((id, name.to_string()));
                }
            }
        }
    }

    // SUM across every id sharing a name. tree-sitter gives an aliased
    // node its own symbol, so one name can have several ids — and looking
    // up a single one reported `identifier` as absent from 61,801 rust
    // files, which is how this bug announced itself.
    let mut counts: BTreeMap<String, u64> = BTreeMap::new();
    for (id, name) in &named {
        *counts.entry(name.clone()).or_insert(0) += totals.get(id).copied().unwrap_or(0);
    }
    let never_seen: Vec<String> =
        counts.iter().filter(|(_, n)| **n == 0).map(|(k, _)| k.clone()).collect();
    let mut thin: Vec<(String, u64)> = counts
        .iter()
        .filter(|(_, n)| **n > 0 && **n < THIN)
        .map(|(k, n)| (k.clone(), *n))
        .collect();
    thin.sort_by_key(|(k, n)| (*n, k.clone()));

    let report = KindsReport {
        lang: lang.to_string(),
        files_parsed,
        named_kinds_total: counts.len(),
        named_kinds_seen: counts.len() - never_seen.len(),
        never_seen,
        thin,
        counts,
    };

    println!(
        "kinds: {} of {} named kinds appear in {} files — {} never do, {} appear fewer than {THIN} times",
        report.named_kinds_seen,
        report.named_kinds_total,
        report.files_parsed,
        report.never_seen.len(),
        report.thin.len(),
    );
    if !report.never_seen.is_empty() {
        println!("  never in the corpus: {}", report.never_seen.join(", "));
    }
    for (k, n) in report.thin.iter().take(12) {
        println!("  thin {n:>4}x  {k}");
    }

    if let Some(parent) = out_path.parent() {
        std::fs::create_dir_all(parent).ok();
    }
    std::fs::write(out_path, serde_json::to_string_pretty(&report)?)?;
    println!("kinds: report at {}", out_path.display());
    Ok(())
}

pub fn default_out(lang: LangName) -> PathBuf {
    PathBuf::from(format!("corpus/{lang}/reports/kinds.json"))
}
