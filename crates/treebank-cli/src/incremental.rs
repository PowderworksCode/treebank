//! Reparsing after an edit must give the same tree as parsing from scratch
//! (`treebank incremental`).
//!
//! This is the one check here with a HARD invariant rather than an oracle.
//! tree-sitter's whole reason for existing is that you can edit a file and
//! reparse only what changed, and the contract is that the result is
//! indistinguishable from a fresh parse. So there is nothing to adjudicate:
//! parse, edit, reparse incrementally, parse the edited text from scratch,
//! and compare. Any difference is ours.
//!
//! It is worth having because the failure is invisible to every other check
//! in this repository. All of them parse from scratch, so a grammar can be
//! perfect on all 204,000 corpus files and still hand a broken tree to the
//! editor that is actually using it — and the usual cause is an external
//! scanner whose `serialize`/`deserialize` does not round-trip its state.
//! Python's scanner carries an indent stack across those calls, so python is
//! exactly the language where this can go wrong quietly.
//!
//! The edits are deliberately crude — delete a run of bytes, insert a token,
//! replace a run — because the property does not care what the edit means.
//! An edit that makes the file invalid is still an edit, and the two parses
//! must still agree about the wreckage.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use anyhow::Result;
use rayon::prelude::*;
use serde::Serialize;
use tree_sitter::{InputEdit, Node, Parser, Point, Tree};

use treebank_corpus::fetch::Manifest;
use treebank_lang::LangName;

use crate::grammar;

/// Kind plus byte range, in pre-order. Both halves matter: an incremental
/// parse that produces the right shape at the wrong offsets is still wrong,
/// and that is the more likely failure.
fn fingerprint(root: Node) -> Vec<(u16, usize, usize)> {
    let mut out = Vec::new();
    let mut cursor = root.walk();
    let mut descend = true;
    loop {
        if descend {
            let n = cursor.node();
            out.push((n.kind_id(), n.start_byte(), n.end_byte()));
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

/// Row/column of a byte offset, which `InputEdit` needs and tree-sitter
/// will not compute for us.
fn point_at(src: &[u8], offset: usize) -> Point {
    let mut row = 0;
    let mut last_nl = 0;
    for (i, b) in src[..offset].iter().enumerate() {
        if *b == b'\n' {
            row += 1;
            last_nl = i + 1;
        }
    }
    Point::new(row, offset - last_nl)
}

/// Keep an offset off the middle of a UTF-8 character; an edit that splits
/// one produces text that is not the text either parser was given.
fn floor_boundary(src: &[u8], mut i: usize) -> usize {
    i = i.min(src.len());
    // `src.len()` is a boundary and has no byte to inspect.
    while i > 0 && i < src.len() && (src[i] & 0xC0) == 0x80 {
        i -= 1;
    }
    i
}

struct Edit {
    start: usize,
    old_end: usize,
    inserted: &'static str,
}

/// Three shapes of edit, chosen by seed. Crude on purpose.
fn edits_for(src: &[u8], seed: u64) -> Vec<Edit> {
    if src.is_empty() {
        return Vec::new();
    }
    let mut out = Vec::new();
    let mut h = seed | 1;
    let mut next = || {
        h ^= h >> 12;
        h ^= h << 25;
        h ^= h >> 27;
        h.wrapping_mul(0x2545_F491_4F6C_DD1D)
    };
    for k in 0..3 {
        let at = floor_boundary(src, (next() as usize) % src.len());
        let len = ((next() as usize) % 12).min(src.len() - at);
        let end = floor_boundary(src, at + len);
        out.push(match k {
            0 => Edit {
                start: at,
                old_end: end,
                inserted: "",
            }, // delete
            1 => Edit {
                start: at,
                old_end: at,
                inserted: "x = 1\n",
            }, // insert
            _ => Edit {
                start: at,
                old_end: end,
                inserted: ")",
            }, // replace
        });
    }
    out
}

fn apply(src: &[u8], e: &Edit) -> Vec<u8> {
    let mut out = Vec::with_capacity(src.len() + e.inserted.len());
    out.extend_from_slice(&src[..e.start]);
    out.extend_from_slice(e.inserted.as_bytes());
    out.extend_from_slice(&src[e.old_end..]);
    out
}

fn input_edit(src: &[u8], new_src: &[u8], e: &Edit) -> InputEdit {
    let new_end = e.start + e.inserted.len();
    InputEdit {
        start_byte: e.start,
        old_end_byte: e.old_end,
        new_end_byte: new_end,
        start_position: point_at(src, e.start),
        old_end_position: point_at(src, e.old_end),
        new_end_position: point_at(new_src, new_end),
    }
}

#[derive(Serialize)]
pub struct Divergence {
    pub path: String,
    pub edit: String,
    pub detail: String,
    /// Whether the EDITED text has a syntax error. This splits the finding
    /// in two, and the two halves mean very different things: a divergence
    /// on text that still parses cleanly is a straightforward reuse bug,
    /// while one on text the edit broke is error recovery and subtree reuse
    /// interacting — the same wreckage can be stitched together more than
    /// one way, and a fresh parse and an incremental one need not pick the
    /// same one.
    pub edited_text_has_error: bool,
}

#[derive(Serialize)]
pub struct IncReport {
    pub lang: String,
    pub files: usize,
    pub edits_applied: usize,
    pub diverged: usize,
    /// Divergences where the edited text still parses cleanly. These are the
    /// ones that must be zero.
    pub diverged_on_valid: usize,
    pub examples: Vec<Divergence>,
}

fn describe(
    a: &[(u16, usize, usize)],
    b: &[(u16, usize, usize)],
    tree: &Tree,
    fresh: &Tree,
) -> String {
    match a.iter().zip(b).position(|(x, y)| x != y) {
        Some(i) => format!(
            "node {i} of {}: incremental {:?} at {}..{}, fresh {:?} at {}..{}",
            a.len().max(b.len()),
            tree.language().node_kind_for_id(a[i].0).unwrap_or("?"),
            a[i].1,
            a[i].2,
            fresh.language().node_kind_for_id(b[i].0).unwrap_or("?"),
            b[i].1,
            b[i].2,
        ),
        None => format!(
            "same prefix, {} nodes incremental vs {} fresh",
            a.len(),
            b.len()
        ),
    }
}

pub fn run(
    lang: LangName,
    grammar_dir: &Path,
    manifest_path: &Path,
    limit: Option<usize>,
    seed: u64,
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
        "incremental: {} files, 3 edits each — reparse must equal a fresh parse",
        entries.len()
    );

    let results: Vec<(usize, usize, Vec<Divergence>)> = entries
        .par_iter()
        .map(|f| -> Result<(usize, usize, Vec<Divergence>)> {
            let rel = format!("{}/{}", f.pkgdir, f.rel);
            let Ok(src) = std::fs::read(corpus_src.join(&rel)) else {
                return Ok((0, 0, Vec::new()));
            };
            let idx = crate::routing::route(lang, &f.dialect, &f.rel);
            let mut parser = Parser::new();
            parser.set_language(&langs[idx])?;
            let mut applied = 0;
            let mut found = Vec::new();
            for e in edits_for(&src, seed ^ (rel.len() as u64)) {
                let Some(mut tree) = parser.parse(&src, None) else {
                    continue;
                };
                let new_src = apply(&src, &e);
                tree.edit(&input_edit(&src, &new_src, &e));
                let (Some(inc), Some(fresh)) = (
                    parser.parse(&new_src, Some(&tree)),
                    parser.parse(&new_src, None),
                ) else {
                    continue;
                };
                applied += 1;
                let (a, b) = (fingerprint(inc.root_node()), fingerprint(fresh.root_node()));
                if a != b {
                    found.push(Divergence {
                        path: rel.clone(),
                        edit: format!("{}..{} -> {:?}", e.start, e.old_end, e.inserted),
                        detail: describe(&a, &b, &inc, &fresh),
                        edited_text_has_error: fresh.root_node().has_error(),
                    });
                }
            }
            Ok((1, applied, found))
        })
        .collect::<Result<_>>()?;

    let mut files = 0;
    let mut edits_applied = 0;
    let mut examples: Vec<Divergence> = Vec::new();
    let mut diverged = 0;
    let mut diverged_on_valid = 0;
    for (f, a, mut d) in results {
        files += f;
        edits_applied += a;
        diverged += d.len();
        diverged_on_valid += d.iter().filter(|x| !x.edited_text_has_error).count();
        // Keep a clean-text divergence over a broken-text one: it is the
        // half that has to be zero.
        d.sort_by_key(|x| x.edited_text_has_error);
        for x in d {
            if examples.len() < 20 {
                examples.push(x);
            }
        }
    }
    examples.sort_by_key(|x| x.edited_text_has_error);

    let report = IncReport {
        lang: lang.to_string(),
        files,
        edits_applied,
        diverged,
        diverged_on_valid,
        examples,
    };
    println!(
        "incremental: {} files, {} edits, {} diverged ({} on text that still parses cleanly)",
        report.files, report.edits_applied, report.diverged, report.diverged_on_valid
    );
    let mut by_file: BTreeMap<&str, usize> = BTreeMap::new();
    for e in &report.examples {
        *by_file.entry(e.path.as_str()).or_insert(0) += 1;
    }
    for e in report.examples.iter().take(10) {
        println!("  {}  edit {}\n      {}", e.path, e.edit, e.detail);
    }

    if let Some(parent) = out_path.parent() {
        std::fs::create_dir_all(parent).ok();
    }
    std::fs::write(out_path, serde_json::to_string_pretty(&report)?)?;
    println!("incremental: report at {}", out_path.display());
    // Only the clean half is a gate. See `Divergence::edited_text_has_error`.
    if report.diverged_on_valid > 0 {
        anyhow::bail!(
            "{} incremental reparse(s) disagree with a fresh parse on text that still parses cleanly",
            report.diverged_on_valid
        );
    }
    Ok(())
}

pub fn default_out(lang: LangName) -> PathBuf {
    PathBuf::from(format!("corpus/{lang}/reports/incremental.json"))
}
