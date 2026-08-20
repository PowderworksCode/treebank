//! `treebank errors` — when we reject a file, do we reject it in the right
//! place?
//!
//! Every other check here is about which files we accept and what tree we
//! build for them. None of them looks at the REJECTIONS, and a grammar can
//! reject exactly the right files while pointing at a wildly wrong offset.
//! That costs twice: an editor's error recovery is only as good as the
//! position it is given, and every gap investigation starts by reading the
//! first ERROR node, so a misplaced one sends the reader to the wrong
//! construct.
//!
//! The corpus for this already exists and was being thrown away: the files
//! the sweep books as NOISE are exactly the ones both parsers reject.
//!
//! No claim is made that the offsets should be equal. Two parsers legitimately
//! notice a problem at different points -- ours reports where the parse table
//! ran out, CPython where its own recovery gave up -- and being a token or two
//! apart is normal. What is worth knowing is the DISTRIBUTION, and especially
//! the tail: a rejection hundreds of bytes from where the reference parser
//! looked is a rejection nobody can act on.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use anyhow::Result;
use rayon::prelude::*;
use serde::{Deserialize, Serialize};
use tree_sitter::Parser;

use treebank_corpus::fetch::Manifest;
use treebank_lang::LangName;

use crate::grammar;

#[derive(Serialize, Deserialize)]
pub struct Case {
    pub path: String,
    pub ours: usize,
    pub theirs: usize,
    pub delta: i64,
    pub context: String,
}

#[derive(Serialize, Deserialize)]
pub struct ErrReport {
    pub lang: String,
    pub files: usize,
    /// Files we reject and the reference parser rejects too.
    pub compared: usize,
    pub exact: usize,
    pub within_16: usize,
    pub within_128: usize,
    /// The tail: further than 128 bytes from where the oracle looked.
    pub far: usize,
    pub median_abs: i64,
    pub worst: Vec<Case>,
}

pub fn run(
    lang: LangName,
    grammar_dir: &Path,
    manifest_path: &Path,
    out: &Path,
    limit: Option<usize>,
) -> Result<()> {
    let oracle = treebank_oracle::spans_for(lang)
        .ok_or_else(|| anyhow::anyhow!("no span oracle for {lang}"))?;
    let manifest = Manifest::load(manifest_path)?;
    let corpus_src = manifest_path.parent().unwrap_or(Path::new(".")).join("src");
    let mut entries = manifest.files();
    entries.sort_by(|a, b| (&a.pkgdir, &a.rel).cmp(&(&b.pkgdir, &b.rel)));
    if let Some(n) = limit {
        entries.truncate(n);
    }

    let (language, _) = grammar::load(grammar_dir)?;

    // Only files WE reject are interesting; the rest have no error of ours
    // to place.
    let rejected: Vec<(String, usize)> = entries
        .par_iter()
        .filter_map(|f| {
            let rel = format!("{}/{}", f.pkgdir, f.rel);
            let src = std::fs::read(corpus_src.join(&rel)).ok()?;
            let mut parser = Parser::new();
            parser.set_language(&language).ok()?;
            let tree = parser.parse(&src, None)?;
            if !tree.root_node().has_error() {
                return None;
            }
            crate::sweep::first_error_offset(tree.root_node()).map(|at| (rel, at))
        })
        .collect();

    println!(
        "errors: {} files, {} rejected by the grammar — asking the oracle where it looked",
        entries.len(),
        rejected.len()
    );

    let mut cases: Vec<Case> = Vec::new();
    for chunk in rejected.chunks(400) {
        let paths: Vec<String> = chunk.iter().map(|(p, _)| p.clone()).collect();
        let spans = oracle.spans(&corpus_src, &paths)?;
        for (rel, ours) in chunk {
            let Some(file) = spans.get(rel) else { continue };
            // Only where the oracle ALSO rejected. Where it accepts, the
            // disagreement is a gap and the sweep already owns it.
            let Some(theirs) = file.error else { continue };
            let src = std::fs::read(corpus_src.join(rel)).unwrap_or_default();
            let lo = src[..(*ours).min(src.len())]
                .iter()
                .rposition(|b| *b == b'\n')
                .map(|i| i + 1)
                .unwrap_or(0);
            let hi = src[(*ours).min(src.len())..]
                .iter()
                .position(|b| *b == b'\n')
                .map(|i| ours + i)
                .unwrap_or(src.len());
            cases.push(Case {
                path: rel.clone(),
                ours: *ours,
                theirs,
                delta: *ours as i64 - theirs as i64,
                context: String::from_utf8_lossy(&src[lo..hi.min(src.len())])
                    .chars()
                    .take(80)
                    .collect(),
            });
        }
    }

    let mut deltas: Vec<i64> = cases.iter().map(|c| c.delta.abs()).collect();
    deltas.sort_unstable();
    let median = deltas.get(deltas.len() / 2).copied().unwrap_or(0);
    let mut buckets: BTreeMap<&str, usize> = BTreeMap::new();
    for d in &deltas {
        let k = match d {
            0 => "exact",
            1..=16 => "within_16",
            17..=128 => "within_128",
            _ => "far",
        };
        *buckets.entry(k).or_default() += 1;
    }
    cases.sort_by_key(|c| std::cmp::Reverse(c.delta.abs()));

    let report = ErrReport {
        lang: lang.to_string(),
        files: entries.len(),
        compared: cases.len(),
        exact: buckets.get("exact").copied().unwrap_or(0),
        within_16: buckets.get("within_16").copied().unwrap_or(0),
        within_128: buckets.get("within_128").copied().unwrap_or(0),
        far: buckets.get("far").copied().unwrap_or(0),
        median_abs: median,
        worst: cases.into_iter().take(10).collect(),
    };

    println!(
        "errors: {} comparable — {} exact, {} within 16 bytes, {} within 128, {} further; median |delta| {}",
        report.compared, report.exact, report.within_16, report.within_128, report.far, report.median_abs
    );
    for c in report.worst.iter().take(6) {
        println!("  {:>9} bytes  {}", c.delta, c.path);
        println!(
            "             ours@{} theirs@{}  {}",
            c.ours,
            c.theirs,
            c.context.trim()
        );
    }
    std::fs::create_dir_all(out.parent().unwrap_or(Path::new(".")))?;
    std::fs::write(out, serde_json::to_string_pretty(&report)?)?;
    println!("errors: report at {}", out.display());
    Ok(())
}

pub fn default_out(lang: LangName) -> PathBuf {
    PathBuf::from(format!("corpus/{lang}/reports/errors.json"))
}
