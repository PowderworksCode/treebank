//! How much does one missing token cost? (`treebank recovery`)
//!
//! Every other check judges the tree we build for text the language
//! accepts. Editors spend most of their time on text it does not: source is
//! broken between one keystroke and the next, and what an editor can do with
//! it depends entirely on how much structure survives. A parser that turns
//! one missing brace into a file-length ERROR is useless there while scoring
//! perfectly everywhere else in this repository.
//!
//! There is no oracle for this. CPython, syn and tsc all stop at the first
//! error and report a message; none of them produces the recovered tree
//! there is nothing to compare against. So this measures a PROPERTY instead
//! of checking an answer: take a file that parses cleanly, delete exactly
//! one token, and see how much of the file lands inside an ERROR.
//!
//! The unit is deliberately "one token" rather than "one byte". Deleting a
//! byte usually makes an identifier shorter, which is not a syntax error at
//! all; deleting a token is the smallest edit that reliably breaks the
//! parse, and it is what a half-typed line looks like.
//!
//! Blast radius is reported as a distribution rather than a single number,
//! because the shape is the point: a parser can have an excellent median and
//! still shred one file in fifty, and it is the tail an editor's user
//! notices.
//!
//! The "shredded" count applies a size floor, and the first run without one
//! showed why. Deleting `import` from `from a b` errors for two lines and
//! recovers — correct behaviour — but on a four-line file two lines is more
//! than half of it, so the deletion was counted as a shredding. A percentage
//! measures the file as much as the parser when the file is small. Only
//! files of at least `MIN_BYTES` are eligible; below that the radius is
//! still reported in the distribution, where it belongs.

use std::path::{Path, PathBuf};

use anyhow::Result;
use rayon::prelude::*;
use serde::Serialize;
use tree_sitter::{Node, Parser};

use treebank_corpus::fetch::Manifest;
use treebank_lang::LangName;

use crate::grammar;

/// Byte ranges of every leaf — our tokens, since the grammar decides what a
/// token is.
fn leaves(root: Node) -> Vec<(usize, usize)> {
    let mut out = Vec::new();
    let mut cursor = root.walk();
    let mut descend = true;
    loop {
        if descend {
            let n = cursor.node();
            if n.child_count() == 0 && n.end_byte() > n.start_byte() {
                out.push((n.start_byte(), n.end_byte()));
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

/// Bytes covered by the OUTERMOST error nodes. Outermost so nesting is not
/// double-counted: what is being measured is how much of the file the
/// parser gave up on, not how many times it said so.
fn error_bytes(root: Node) -> usize {
    let mut total = 0;
    let mut cursor = root.walk();
    let mut descend = true;
    loop {
        if descend {
            let n = cursor.node();
            if n.is_error() || n.is_missing() {
                total += n.end_byte() - n.start_byte();
                // Do not descend into an error we have already counted.
                if cursor.goto_next_sibling() {
                    continue;
                }
                loop {
                    if !cursor.goto_parent() {
                        return total;
                    }
                    if cursor.goto_next_sibling() {
                        break;
                    }
                }
                continue;
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
                return total;
            }
            if cursor.goto_next_sibling() {
                descend = true;
                break;
            }
        }
    }
}

#[derive(Serialize)]
pub struct Worst {
    pub path: String,
    pub deleted: String,
    pub at: usize,
    pub radius_pct: f64,
}

#[derive(Serialize)]
pub struct RecReport {
    pub lang: String,
    pub files: usize,
    /// One-token deletions that produced a parse error, i.e. the ones that
    /// have anything to say. A deletion the grammar still accepts is not a
    /// recovery question.
    pub breaking_deletions: usize,
    pub median_radius_pct: f64,
    pub p90_radius_pct: f64,
    pub p99_radius_pct: f64,
    /// Deletions where more than half the file ended up inside an error,
    /// counting only files of at least MIN_BYTES — see the module header.
    pub shredded: usize,
    pub shredded_min_bytes: usize,
    /// Which tokens do that, commonest first. A count on its own is not
    /// actionable: losing a quote and losing an identifier are the same
    /// number and completely different problems — an unterminated string
    /// genuinely does swallow the rest of the file, while an identifier
    /// should cost a line.
    pub shredded_by_token: Vec<(String, usize)>,
    pub worst: Vec<Worst>,
}

const PER_FILE: usize = 8;

/// Below this, a percentage says more about the file than about recovery.
const MIN_BYTES: usize = 1024;

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

    println!(
        "recovery: {} files, up to {PER_FILE} single-token deletions each — how far does one missing token spread?",
        entries.len()
    );

    let per_file: Vec<(usize, Vec<(f64, usize, Worst)>)> = entries
        .par_iter()
        .map(|f| -> Result<(usize, Vec<(f64, usize, Worst)>)> {
            let rel = format!("{}/{}", f.pkgdir, f.rel);
            let Ok(src) = std::fs::read(corpus_src.join(&rel)) else {
                return Ok((0, Vec::new()));
            };
            if src.is_empty() {
                return Ok((0, Vec::new()));
            }
            let idx = crate::routing::route(lang, &f.dialect, &f.rel);
            let mut parser = Parser::new();
            parser.set_language(&langs[idx])?;
            let Some(tree) = parser.parse(&src, None) else {
                return Ok((0, Vec::new()));
            };
            // Only files we already handle can say anything about recovery.
            if tree.root_node().has_error() {
                return Ok((0, Vec::new()));
            }
            let toks = leaves(tree.root_node());
            if toks.is_empty() {
                return Ok((0, Vec::new()));
            }
            let step = (toks.len() / PER_FILE).max(1);
            let mut out = Vec::new();
            for (start, end) in toks.iter().step_by(step).take(PER_FILE) {
                let mut damaged = Vec::with_capacity(src.len());
                damaged.extend_from_slice(&src[..*start]);
                damaged.extend_from_slice(&src[*end..]);
                let Some(dt) = parser.parse(&damaged, None) else {
                    continue;
                };
                if !dt.root_node().has_error() {
                    continue; // still valid; not a recovery question
                }
                let radius = if damaged.is_empty() {
                    0.0
                } else {
                    error_bytes(dt.root_node()) as f64 * 100.0 / damaged.len() as f64
                };
                out.push((
                    radius,
                    damaged.len(),
                    Worst {
                        path: rel.clone(),
                        deleted: String::from_utf8_lossy(&src[*start..*end])
                            .chars()
                            .take(24)
                            .collect(),
                        at: *start,
                        radius_pct: (radius * 10.0).round() / 10.0,
                    },
                ));
            }
            Ok((1, out))
        })
        .collect::<Result<_>>()?;

    let mut files = 0;
    let mut all: Vec<(f64, usize, Worst)> = Vec::new();
    for (f, mut v) in per_file {
        files += f;
        all.append(&mut v);
    }
    all.sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap_or(std::cmp::Ordering::Equal));

    let pct = |q: f64| -> f64 {
        if all.is_empty() {
            return 0.0;
        }
        let i = ((all.len() as f64 - 1.0) * q).round() as usize;
        (all[i].0 * 10.0).round() / 10.0
    };
    let is_shredded = |(r, n, _): &&(f64, usize, Worst)| *r > 50.0 && *n >= MIN_BYTES;
    let shredded = all.iter().filter(is_shredded).count();
    let mut by_token: std::collections::BTreeMap<String, usize> = Default::default();
    for (_, _, w) in all.iter().filter(is_shredded) {
        *by_token.entry(w.deleted.clone()).or_insert(0) += 1;
    }
    let mut shredded_by_token: Vec<(String, usize)> = by_token.into_iter().collect();
    shredded_by_token.sort_by_key(|(_, n)| std::cmp::Reverse(*n));
    shredded_by_token.truncate(15);
    let worst: Vec<Worst> = all
        .iter()
        .rev()
        .filter(|(_, n, _)| *n >= MIN_BYTES)
        .take(10)
        .map(|(_, _, w)| Worst {
            path: w.path.clone(),
            deleted: w.deleted.clone(),
            at: w.at,
            radius_pct: w.radius_pct,
        })
        .collect();

    let report = RecReport {
        lang: lang.to_string(),
        files,
        breaking_deletions: all.len(),
        median_radius_pct: pct(0.5),
        p90_radius_pct: pct(0.9),
        p99_radius_pct: pct(0.99),
        shredded,
        shredded_min_bytes: MIN_BYTES,
        shredded_by_token,
        worst,
    };

    println!(
        "recovery: {} files, {} breaking deletions — blast radius median {}%, p90 {}%, p99 {}%; {} shredded (>50% of a file of at least {} bytes inside an error)",
        report.files,
        report.breaking_deletions,
        report.median_radius_pct,
        report.p90_radius_pct,
        report.p99_radius_pct,
        report.shredded,
        MIN_BYTES
    );
    for (tok, n) in report.shredded_by_token.iter().take(8) {
        println!("  shreds {n:>5}x  deleting {tok:?}");
    }
    for w in report.worst.iter().take(3) {
        println!(
            "  {:>5}%  {}  deleting {:?} at {}",
            w.radius_pct, w.path, w.deleted, w.at
        );
    }

    if let Some(parent) = out_path.parent() {
        std::fs::create_dir_all(parent).ok();
    }
    std::fs::write(out_path, serde_json::to_string_pretty(&report)?)?;
    println!("recovery: report at {}", out_path.display());
    Ok(())
}

pub fn default_out(lang: LangName) -> PathBuf {
    PathBuf::from(format!("corpus/{lang}/reports/recovery.json"))
}
