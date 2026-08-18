//! Reformatting must not change our tree (`treebank reformat`).
//!
//! A formatter preserves the program and rewrites its layout, so every node
//! we produce before must be there afterwards, in the same order. Anything
//! else is ours: a rule reading layout it should not, or a token that only
//! lexes when it happens to abut its neighbour.
//!
//! The comparison is a pre-order sequence of named node kinds with their
//! field names — deliberately NOT the spans, since every span moves and that
//! is the point. Comments are included: they are `extras`, and a formatter
//! that relocated one relative to the code around it would be worth knowing
//! about.
//!
//! **Only files the formatter changed in WHITESPACE ALONE are compared**,
//! and that restriction is the whole check rather than a detail of it. A
//! formatter is not tree-preserving: rustfmt rewrites `extern {` into
//! `extern "C" {`, adds a semicolon after a tail `return`, and collapses
//! `|x| { f() }` to `|x| f()`. Each is semantically neutral and
//! syntactically real, so the tree moves and nothing is wrong. Measured on
//! 600 rust files, those rewrites accounted for every divergence.
//!
//! Comparing everything and keeping a list of the formatter's known
//! rewrites was the alternative, and it is worse: the list is open-ended,
//! and each entry is a blanket that also silences a genuine finding wearing
//! the same node pair. Comparing the strings with all whitespace removed
//! costs a little yield — a file where the formatter added a trailing comma
//! is skipped — and buys a question with an unambiguous answer. Any
//! divergence that survives is caused by layout, which is ours by
//! construction.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use anyhow::Result;
use rayon::prelude::*;
use serde::Serialize;
use tree_sitter::{Node, Parser};

use treebank_lang::LangName;

use treebank_corpus::fetch::Manifest;
use crate::grammar;

/// Kinds and field names in pre-order. Positions are excluded on purpose.
fn shape(root: Node) -> Vec<String> {
    let mut out = Vec::new();
    let mut cursor = root.walk();
    let mut descend = true;
    loop {
        if descend {
            let n = cursor.node();
            if n.is_named() {
                match cursor.field_name() {
                    Some(f) => out.push(format!("{f}:{}", n.kind())),
                    None => out.push(n.kind().to_string()),
                }
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
                return out;
            }
            if cursor.goto_next_sibling() {
                descend = true;
                break;
            }
        }
    }
}

/// The first place the two sequences part company, as a readable pair.
fn first_divergence(before: &[String], after: &[String]) -> String {
    let at = before.iter().zip(after).position(|(a, b)| a != b);
    match at {
        Some(i) => {
            let ctx = i.saturating_sub(1);
            format!(
                "at node {i} (after `{}`): before `{}`, after `{}`",
                before.get(ctx).map(String::as_str).unwrap_or("<root>"),
                before.get(i).map(String::as_str).unwrap_or("<end>"),
                after.get(i).map(String::as_str).unwrap_or("<end>"),
            )
        }
        None => format!("same prefix, different length: {} before, {} after", before.len(), after.len()),
    }
}

#[derive(Serialize)]
pub struct Divergence {
    pub path: String,
    pub detail: String,
}

#[derive(Serialize)]
pub struct FmtReport {
    pub lang: String,
    pub tool: String,
    pub files: usize,
    /// Files the formatter actually changed. Only these carry any signal —
    /// a file already in the formatter's style proves nothing.
    pub reformatted: usize,
    /// Files not compared: the formatter declined them, or it changed more
    /// than layout, which puts them outside what this check can ask.
    pub skipped: usize,
    pub skip_reasons: BTreeMap<String, usize>,
    /// Reformatted files whose tree we could not build afterwards.
    pub unparsable: usize,
    pub diverged: usize,
    pub examples: Vec<Divergence>,
}

pub fn run(
    lang: LangName,
    grammar_dir: &Path,
    manifest_path: &Path,
    limit: Option<usize>,
    out_path: &Path,
) -> Result<()> {
    let Some(fmt) = treebank_oracle::reformatter_for(lang) else {
        anyhow::bail!(
            "no formatter for {lang}: this check needs the language's own formatter, \
             and stating that is better than substituting something else"
        );
    };
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

    println!(
        "reformat: {} files through {} — the tree must not move",
        entries.len(),
        fmt.tool()
    );

    let mut reformatted = 0usize;
    let mut skipped = 0usize;
    let mut unparsable = 0usize;
    let mut diverged = 0usize;
    let mut skip_reasons: BTreeMap<String, usize> = BTreeMap::new();
    let mut examples: Vec<Divergence> = Vec::new();

    for chunk in entries.chunks(200) {
        let paths: Vec<String> =
            chunk.iter().map(|f| format!("{}/{}", f.pkgdir, f.rel)).collect();
        let formatted = fmt.reformat(&corpus_src, &paths)?;

        let results: Vec<(bool, bool, bool, Option<String>, Option<Divergence>)> = chunk
            .par_iter()
            .zip(&paths)
            .map(|(f, rel)| -> Result<_> {
                let Some(r) = formatted.get(rel) else {
                    return Ok((false, false, false, Some("no record".into()), None));
                };
                let Some(after_src) = r.source.as_ref() else {
                    let why = r.skipped.clone().unwrap_or_else(|| "unstated".into());
                    return Ok((false, false, false, Some(why), None));
                };
                let before_src = std::fs::read(corpus_src.join(rel))?;
                if before_src == after_src.as_bytes() {
                    // Already in the formatter's style: no signal either way.
                    return Ok((false, false, false, None, None));
                }
                // Did the formatter change anything but layout? If so this
                // file cannot answer the question being asked.
                let squash = |b: &[u8]| -> Vec<u8> {
                    b.iter().copied().filter(|c| !c.is_ascii_whitespace()).collect()
                };
                if squash(&before_src) != squash(after_src.as_bytes()) {
                    return Ok((false, false, false, Some("formatter rewrote tokens, not only layout".into()), None));
                }
                let idx = crate::routing::route(lang, &f.dialect, &f.rel);
                let mut parser = Parser::new();
                parser.set_language(&langs[idx])?;
                let (Some(before), Some(after)) = (
                    parser.parse(&before_src, None),
                    parser.parse(after_src.as_bytes(), None),
                ) else {
                    return Ok((true, true, false, None, None));
                };
                // A file we already fail on cannot say anything about
                // invariance; the sweep owns that.
                if before.root_node().has_error() || after.root_node().has_error() {
                    return Ok((true, true, false, None, None));
                }
                let (a, b) = (shape(before.root_node()), shape(after.root_node()));
                if a == b {
                    return Ok((true, false, false, None, None));
                }
                Ok((
                    true,
                    false,
                    true,
                    None,
                    Some(Divergence { path: rel.clone(), detail: first_divergence(&a, &b) }),
                ))
            })
            .collect::<Result<_>>()?;

        for (changed, unp, div, why, ex) in results {
            if changed {
                reformatted += 1;
            }
            if unp {
                unparsable += 1;
            }
            if div {
                diverged += 1;
            }
            if let Some(w) = why {
                skipped += 1;
                *skip_reasons.entry(w.chars().take(80).collect()).or_insert(0) += 1;
            }
            if let Some(e) = ex {
                if examples.len() < 20 {
                    examples.push(e);
                }
            }
        }
    }

    let report = FmtReport {
        lang: lang.to_string(),
        tool: fmt.tool().to_string(),
        files: entries.len(),
        reformatted,
        skipped,
        skip_reasons,
        unparsable,
        diverged,
        examples,
    };

    println!(
        "reformat: {} reformatted ({} unchanged carry no signal), {} skipped, {} unparsable either side, {} DIVERGED",
        report.reformatted,
        report.files - report.reformatted - report.skipped,
        report.skipped,
        report.unparsable,
        report.diverged
    );
    let mut skips: Vec<(&String, &usize)> = report.skip_reasons.iter().collect();
    skips.sort_by_key(|(_, n)| std::cmp::Reverse(**n));
    for (why, n) in skips.iter().take(5) {
        println!("  skipped {n:>6}x  {why}");
    }
    for e in report.examples.iter().take(10) {
        println!("  {}  {}", e.path, e.detail);
    }

    if let Some(parent) = out_path.parent() {
        std::fs::create_dir_all(parent).ok();
    }
    std::fs::write(out_path, serde_json::to_string_pretty(&report)?)?;
    println!("reformat: report at {}", out_path.display());

    if report.diverged > 0 {
        anyhow::bail!("{} file(s) parse differently after reformatting", report.diverged);
    }
    Ok(())
}

pub fn default_out(lang: LangName) -> PathBuf {
    PathBuf::from(format!("corpus/{lang}/reports/reformat.json"))
}
