//! `treebank roundtrip` — do we parse the language's own canonical spelling?
//!
//! The corpus is written by people, and people write a construct the usual
//! way. A grammar can handle every spelling that appears in 139,205 files and
//! still miss the one the language's own printer emits — parentheses dropped
//! where the tree does not need them, quotes and spacing normalised, a
//! trailing comma gone.
//!
//! Re-rendering every file through `ast.unparse` or `ts.createPrinter` and
//! parsing the result costs one pass and doubles the corpus with source no
//! human wrote. What it finds is a real gap; what it does not find is a
//! construct we handle in both spellings, which is the answer we want.

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
pub struct Failure {
    pub path: String,
    pub line: usize,
    pub snippet: String,
}

#[derive(Serialize, Deserialize)]
pub struct RtCluster {
    pub signature: String,
    pub count: usize,
    pub examples: Vec<Failure>,
}

#[derive(Serialize, Deserialize)]
pub struct RtReport {
    pub lang: String,
    pub files: usize,
    /// Files the printer produced a rendering for.
    pub rendered: usize,
    /// Files it declined — its own parse failed, or it gave up.
    pub skipped: usize,
    /// Why it declined, by reason, commonest first. A bare count invites
    /// the assumption that a skip is the corpus's fault; naming them shows
    /// when it is the printer's, which is a different thing to fix.
    pub skip_reasons: BTreeMap<String, usize>,
    pub reparsed: usize,
    pub failed: usize,
    pub clusters: Vec<RtCluster>,
}

/// Collapse a printer's message to the class of thing that went wrong.
/// Raw messages carry the offending source inline, so a thousand distinct
/// strings would describe one problem.
fn skip_kind(reason: &str) -> String {
    let head = reason.split(['`', '\n']).next().unwrap_or(reason).trim();
    let head = head.strip_suffix(':').unwrap_or(head);
    head.chars().take(80).collect()
}

pub fn run(
    lang: LangName,
    grammar_dir: &Path,
    manifest_path: &Path,
    out: &Path,
    limit: Option<usize>,
) -> Result<()> {
    let printer = treebank_oracle::unparser_for(lang).ok_or_else(|| {
        anyhow::anyhow!("no printer for {lang}: round-tripping needs the toolchain to render its own tree")
    })?;
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

    println!("roundtrip: {} files through the {lang} printer", entries.len());

    let mut rendered = 0usize;
    let mut skipped = 0usize;
    let mut reparsed = 0usize;
    let mut by_sig: BTreeMap<String, Vec<Failure>> = BTreeMap::new();
    let mut skip_reasons: BTreeMap<String, usize> = BTreeMap::new();

    for chunk in entries.chunks(400) {
        let paths: Vec<String> = chunk.iter().map(|f| format!("{}/{}", f.pkgdir, f.rel)).collect();
        let out_map = printer.unparse(&corpus_src, &paths)?;
        let results: Vec<(bool, bool, Option<(String, Failure)>, Option<String>)> = chunk
            .par_iter()
            .zip(&paths)
            .map(|(f, rel)| -> Result<(bool, bool, Option<(String, Failure)>, Option<String>)> {
                let Some(r) = out_map.get(rel) else {
                    return Ok((false, false, None, Some("printer returned no record".into())));
                };
                let Some(text) = r.source.as_ref() else {
                    let why = r.skipped.clone().unwrap_or_else(|| "unstated".into());
                    return Ok((false, false, None, Some(skip_kind(&why))));
                };
                let idx = crate::routing::route(lang, &f.dialect, &f.rel);
                let mut parser = Parser::new();
                parser.set_language(&langs[idx])?;
                let Some(tree) = parser.parse(text.as_bytes(), None) else {
                    return Ok((true, false, None, None));
                };
                if !tree.root_node().has_error() {
                    return Ok((true, true, None, None));
                }
                // Cluster by the same signature the sweep uses, so a
                // round-trip failure reads like a gap and can be chased the
                // same way.
                let (sig, line, snippet) = crate::sweep::error_signature(tree.root_node(), text);
                Ok((true, true, Some((sig, Failure { path: rel.clone(), line, snippet })), None))
            })
            .collect::<Result<_>>()?;
        for (r, p, fail, why) in results {
            if r { rendered += 1 } else { skipped += 1 }
            if p { reparsed += 1 }
            if let Some((sig, f)) = fail {
                by_sig.entry(sig).or_default().push(f);
            }
            if let Some(w) = why {
                *skip_reasons.entry(w).or_insert(0) += 1;
            }
        }
    }

    let mut clusters: Vec<RtCluster> = by_sig
        .into_iter()
        .map(|(signature, fs)| RtCluster {
            signature,
            count: fs.len(),
            examples: fs.into_iter().take(3).collect(),
        })
        .collect();
    clusters.sort_by_key(|c| std::cmp::Reverse(c.count));
    let failed: usize = clusters.iter().map(|c| c.count).sum();

    let report = RtReport {
        lang: lang.to_string(),
        files: entries.len(),
        rendered,
        skipped,
        reparsed,
        failed,
        clusters,
        skip_reasons,
    };
    println!(
        "roundtrip: {} rendered ({} skipped by the printer), {} reparsed, {} FAILED in {} cluster(s)",
        report.rendered, report.skipped, report.reparsed, report.failed, report.clusters.len()
    );
    let mut skips: Vec<(&String, &usize)> = report.skip_reasons.iter().collect();
    skips.sort_by_key(|(_, n)| std::cmp::Reverse(**n));
    for (why, n) in skips.iter().take(6) {
        println!("  skipped {n:>6}x  {why}");
    }
    for c in report.clusters.iter().take(12) {
        println!("  {:>6}x  {}", c.count, c.signature);
        if let Some(e) = c.examples.first() {
            println!("           {}:{}  {}", e.path, e.line, e.snippet.trim());
        }
    }
    std::fs::create_dir_all(out.parent().unwrap_or(Path::new(".")))?;
    std::fs::write(out, serde_json::to_string_pretty(&report)?)?;
    println!("roundtrip: report at {}", out.display());
    Ok(())
}

pub fn default_out(lang: LangName) -> PathBuf {
    PathBuf::from(format!("corpus/{lang}/reports/roundtrip.json"))
}
