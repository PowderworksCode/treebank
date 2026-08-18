//! `treebank shape` — does our tree agree with the reference parser about
//! where the node boundaries fall?
//!
//! The sweep can only catch the grammar REJECTING valid code. It is
//! structurally blind to the grammar ACCEPTING code and building the wrong
//! tree for it: those files parse cleanly, sweep cleanly, and ship. Every
//! silent mis-parse found here so far was found by accident, from an
//! adjacent file where the wrong reading happened to be illegal.
//!
//! The property checked, over every clean-parsing file in the corpus:
//!
//!   for every node the reference parser reports, our tree has a node with
//!   exactly that byte span.
//!
//! One-directional on purpose. Our tree may have nodes the oracle does not —
//! finer granularity is fine. What it may not do is fail to see a boundary
//! the reference parser sees, because that means we grouped the code
//! differently, and one of the two groupings is wrong.

use std::collections::{BTreeMap, HashMap, HashSet};
use std::path::Path;

use anyhow::{Context, Result};
use rayon::prelude::*;
use serde::{Deserialize, Serialize};
use tree_sitter::Parser;

use treebank_lang::LangName;

use crate::grammar;

#[derive(Serialize, Deserialize)]
pub struct Miss {
    pub path: String,
    pub kind: String,
    pub start: usize,
    pub end: usize,
    /// The source text at the boundary the oracle saw and we did not,
    /// clipped. This is the whole diagnostic: it names the construct.
    pub text: String,
}

#[derive(Serialize, Deserialize)]
pub struct ShapeCluster {
    /// Oracle node kind plus the innermost node of ours that straddles the
    /// boundary — the pair says both what was expected and what we built.
    pub signature: String,
    pub count: usize,
    pub files: usize,
    pub examples: Vec<Miss>,
}

#[derive(Serialize, Deserialize)]
pub struct ShapeReport {
    pub lang: String,
    pub grammar: String,
    pub files_checked: usize,
    pub files_skipped: usize,
    pub oracle_nodes: usize,
    pub missed_nodes: usize,
    pub files_with_misses: usize,
    pub clusters: Vec<ShapeCluster>,
}

/// Boundaries this grammar deliberately does not mirror, as
/// `"<OracleKind> <- <our_kind>"` pairs — the same signature the report
/// prints. Pairs rather than bare oracle kinds so that allowlisting a
/// granularity difference cannot also silence a real disagreement about the
/// same kind somewhere else.
#[derive(Deserialize, Default)]
struct ShapePolicy {
    #[serde(default)]
    ignore: Vec<Ignored>,
    /// The ratchet. Misses above this fail the command, so a change that
    /// silently regroups the tree cannot land unnoticed -- which is not
    /// hypothetical: raising the type operators above `PREC.cast` to fix
    /// `x as A & B` also lifted them above `type_operator`, and
    /// `readonly string[] | undefined` quietly became
    /// `readonly (string[] | undefined)` in 119 files. A report nobody reads
    /// would not have caught that; a ratchet does.
    ///
    /// Absent means no ceiling, which is right for a grammar that has not
    /// been through this yet.
    #[serde(default)]
    baseline_missed: Option<usize>,
}

#[derive(Deserialize)]
struct Ignored {
    signature: String,
}

fn load_policy(grammar_dir: &Path) -> Result<(HashSet<String>, Option<usize>)> {
    let path = grammar_dir.join("shape_policy.json");
    if !path.exists() {
        return Ok((Default::default(), None));
    }
    let text = std::fs::read_to_string(&path)
        .with_context(|| format!("read {}", path.display()))?;
    let policy: ShapePolicy = serde_json::from_str(&text)
        .with_context(|| format!("parse {}", path.display()))?;
    Ok((
        policy.ignore.into_iter().map(|i| i.signature).collect(),
        policy.baseline_missed,
    ))
}

/// Where a span ends once trailing separators and whitespace are dropped.
///
/// Two parsers can agree completely about the structure and still disagree
/// about which side of a `;` or `,` the boundary falls on: tsc puts the
/// terminator INSIDE a `PropertySignature` and OUTSIDE a
/// `VariableDeclarationList`, and we do the opposite in both. That is
/// punctuation bookkeeping, not shape, and there are thousands of it.
///
/// Handled as a RULE rather than an allowlist entry, and applied to both
/// sides so it works whichever parser is the longer one. An allowlist would
/// have had to name `PropertySignature`, and would then have hidden a real
/// `PropertySignature` disagreement too.
fn trim_end(src: &[u8], start: usize, mut end: usize) -> usize {
    while end > start {
        match src[end - 1] {
            b';' | b',' | b' ' | b'\t' | b'\n' | b'\r' => end -= 1,
            _ => break,
        }
    }
    end
}

/// Where a node's CONTENT ends, ignoring trailing trivia it happens to own.
///
/// A trailing comment has to belong to somebody, and the two parsers do not
/// have to agree on whom. `return  # No prctl.` is the last statement of a
/// function body; CPython ends the FunctionDef at `return`, and we end the
/// block after the comment, because tree-sitter attaches an extra to
/// whatever node is open when it is consumed. Neither is wrong, and there
/// are thousands of it.
///
/// Handled as a RULE, like the separator trim, and a language-agnostic one:
/// tree-sitter marks extras itself, so this walks back past trailing extras
/// without knowing what a comment looks like in any particular language.
fn content_end(node: tree_sitter::Node) -> usize {
    let mut cursor = node.walk();
    let mut last = None;
    for child in node.children(&mut cursor) {
        if !child.is_extra() {
            last = Some(child);
        }
    }
    match last {
        Some(child) => content_end(child),
        None => node.end_byte(),
    }
}

/// Where a node's span begins once the trivia immediately in front of it is
/// counted as its own.
///
/// The mirror of `content_end`, and Rust is what forces it: `syn` turns a
/// `///` doc comment into a `#[doc]` attribute, which is part of the item,
/// while we keep it as a comment extra in front of the item. Neither is
/// wrong -- one parser thinks the documentation is part of the thing
/// documented and the other thinks it is trivia -- and it is the single
/// largest source of disagreement in the Rust corpus.
///
/// Language-agnostic for the same reason as `content_end`: tree-sitter
/// marks extras itself.
/// Every prefix of that run, not just the longest. A file that opens with a
/// `//!` module comment, then a blank line, then a `///` doc comment on the
/// item, has TWO contiguous extras in front of the item, and syn takes only
/// the second -- the one that is the item's documentation. Taking only the
/// longest extension would miss it by exactly one comment.
fn leading_starts(node: tree_sitter::Node) -> Vec<usize> {
    let mut out = Vec::new();
    let mut prev = node.prev_sibling();
    while let Some(p) = prev {
        if !p.is_extra() {
            break;
        }
        out.push(p.start_byte());
        prev = p.prev_sibling();
    }
    out
}

/// Every byte span in our tree, named and anonymous alike, plus each one's
/// separator-trimmed form. Anonymous nodes count: the oracle reports keyword
/// and punctuation nodes too, and a keyword we tokenise identically is
/// agreement, not a finer grouping.
fn our_spans(root: tree_sitter::Node, src: &[u8]) -> HashSet<(usize, usize)> {
    let mut out = HashSet::new();
    let mut cursor = root.walk();
    let mut recurse = true;
    loop {
        if recurse {
            let n = cursor.node();
            let (a, b) = (n.start_byte(), n.end_byte());
            out.insert((a, b));
            out.insert((a, trim_end(src, a, b.min(src.len()))));
            // ...and the same node with any trailing trivia it owns removed,
            // then separator-trimmed again, since the trivia may sit after a
            // terminator.
            let c = content_end(n);
            if c > a && c < b {
                out.insert((a, c));
                out.insert((a, trim_end(src, a, c.min(src.len()))));
            }
            // ...and the same node counting the trivia in FRONT of it as its
            // own, which is where the two parsers disagree about doc
            // comments.
            for lead in leading_starts(n) {
                if lead >= a {
                    continue;
                }
                out.insert((lead, b));
                out.insert((lead, trim_end(src, lead, b.min(src.len()))));
                if c > lead && c < b {
                    out.insert((lead, c));
                    out.insert((lead, trim_end(src, lead, c.min(src.len()))));
                }
            }
        }
        if recurse && cursor.goto_first_child() {
            continue;
        }
        if cursor.goto_next_sibling() {
            recurse = true;
            continue;
        }
        if !cursor.goto_parent() {
            break;
        }
        recurse = false;
    }
    out
}

/// The innermost node of ours containing the oracle's span. Naming it is
/// what turns "we are missing a boundary" into "we built THIS instead".
fn straddler(root: tree_sitter::Node, start: usize, end: usize) -> String {
    let mut best = root;
    let mut node = root;
    loop {
        let mut found = None;
        let mut cursor = node.walk();
        if cursor.goto_first_child() {
            loop {
                let c = cursor.node();
                if c.start_byte() <= start && c.end_byte() >= end {
                    found = Some(c);
                    break;
                }
                if !cursor.goto_next_sibling() {
                    break;
                }
            }
        }
        match found {
            Some(c) => {
                best = c;
                node = c;
            }
            None => break,
        }
    }
    best.kind().to_string()
}

pub fn run(
    lang: LangName,
    grammar_dir: &Path,
    manifest_path: &Path,
    out: &Path,
    limit: Option<usize>,
    dir: Option<&Path>,
) -> Result<()> {
    let oracle = treebank_oracle::spans_for(lang).with_context(|| {
        format!("no span oracle for {lang}: shape checking needs a reference parser that can report node boundaries")
    })?;

    // Two sources. The corpus is where findings come from; a committed
    // fixture directory is how they stay fixed, because CI has no corpus and
    // a check that only ever runs by hand is not a gate.
    let (corpus_src, files, dialect) = match dir {
        Some(d) => {
            let mut files = Vec::new();
            collect(d, d, extensions(lang), &mut files)?;
            files.sort();
            anyhow::ensure!(!files.is_empty(), "no source files under {}", d.display());
            let dialect = files
                .iter()
                .map(|f| (f.clone(), crate::routing::route(lang, &None, f)))
                .collect();
            (d.to_path_buf(), files, dialect)
        }
        None => {
            let manifest: treebank_corpus::fetch::Manifest = serde_json::from_str(
                &std::fs::read_to_string(manifest_path)
                    .with_context(|| format!("read {}", manifest_path.display()))?,
            )?;
            let corpus_src = manifest_path
                .parent()
                .unwrap_or(Path::new("."))
                .join("src");
            let mut entries = manifest.files();
            entries.sort_by(|a, b| (&a.pkgdir, &a.rel).cmp(&(&b.pkgdir, &b.rel)));
            if let Some(n) = limit {
                entries.truncate(n);
            }
            let files: Vec<String> = entries
                .iter()
                .map(|f| format!("{}/{}", f.pkgdir, f.rel))
                .collect();
            let dialect: HashMap<String, usize> = entries
                .iter()
                .map(|f| {
                    (
                        format!("{}/{}", f.pkgdir, f.rel),
                        crate::routing::route(lang, &f.dialect, &f.rel),
                    )
                })
                .collect();
            (corpus_src, files, dialect)
        }
    };

    let (ignore, baseline) = load_policy(grammar_dir)?;
    let dirs = crate::routing::grammar_dirs(lang);
    let langs: Vec<(tree_sitter::Language, String)> = dirs
        .iter()
        .map(|d| grammar::load(&grammar_dir.join(d)))
        .collect::<Result<_>>()?;

    println!(
        "shape: {} files against {} ({} declared granularity difference(s))",
        files.len(),
        grammar_dir.display(),
        ignore.len()
    );

    // Batched so the oracle process is not handed the whole corpus at once
    // and so a failure surfaces early.
    const BATCH: usize = 400;
    let mut report = ShapeReport {
        lang: lang.to_string(),
        grammar: grammar_dir.display().to_string(),
        files_checked: 0,
        files_skipped: 0,
        oracle_nodes: 0,
        missed_nodes: 0,
        files_with_misses: 0,
        clusters: Vec::new(),
    };
    let mut by_sig: BTreeMap<String, Vec<Miss>> = BTreeMap::new();

    for chunk in files.chunks(BATCH) {
        let batch: Vec<String> = chunk.to_vec();
        let oracle_spans = oracle.spans(&corpus_src, &batch)?;

        let results: Vec<(usize, usize, bool, bool, Vec<Miss>)> = batch
            .par_iter()
            .map(|rel| -> Result<(usize, usize, bool, bool, Vec<Miss>)> {
                let Some(file) = oracle_spans.get(rel) else {
                    // The oracle must answer every path it was asked about;
                    // a missing answer is an oracle failure, not a pass.
                    anyhow::bail!("ts-oracle returned no span record for {rel}");
                };
                if file.skipped.is_some() {
                    return Ok((0, 0, false, true, Vec::new()));
                }
                let src = std::fs::read(corpus_src.join(rel))?;
                let idx = dialect.get(rel.as_str()).copied().unwrap_or(0);
                let mut parser = Parser::new();
                parser.set_language(&langs[idx].0)?;
                let Some(tree) = parser.parse(&src, None) else {
                    return Ok((0, 0, false, true, Vec::new()));
                };
                // A file we cannot parse is the SWEEP's business, not this
                // check's; comparing shapes against an error tree is noise.
                if tree.root_node().has_error() {
                    return Ok((0, 0, false, true, Vec::new()));
                }
                let ours = our_spans(tree.root_node(), &src);
                let mut misses = Vec::new();
                for s in &file.spans {
                    if ours.contains(&(s.start, s.end)) {
                        continue;
                    }
                    let trimmed = trim_end(&src, s.start, s.end.min(src.len()));
                    if ours.contains(&(s.start, trimmed)) {
                        continue;
                    }
                    // Name what we built instead. The PAIR is the signature,
                    // and the pair is what the allowlist matches on: ignoring
                    // a bare kind would also ignore a real disagreement about
                    // that kind, which is how a check like this stops working
                    // without anyone noticing.
                    let built = straddler(tree.root_node(), s.start, s.end);
                    let signature = format!("{} <- {}", s.kind, built);
                    if ignore.contains(&signature) {
                        continue;
                    }
                    let text = String::from_utf8_lossy(
                        &src[s.start.min(src.len())..s.end.min(src.len())],
                    );
                    let text: String = text.chars().take(60).collect();
                    misses.push(Miss {
                        path: rel.to_string(),
                        kind: signature,
                        start: s.start,
                        end: s.end,
                        text: text.replace('\n', "\\n"),
                    });
                }
                let n = misses.len();
                Ok((file.spans.len(), n, n > 0, false, misses))
            })
            .collect::<Result<_>>()?;

        for (nodes, missed, had, skipped, misses) in results {
            report.oracle_nodes += nodes;
            report.missed_nodes += missed;
            if skipped {
                report.files_skipped += 1;
            } else {
                report.files_checked += 1;
            }
            if had {
                report.files_with_misses += 1;
            }
            for m in misses {
                by_sig.entry(m.kind.clone()).or_default().push(m);
            }
        }
    }

    report.clusters = by_sig
        .into_iter()
        .map(|(signature, ms)| {
            let files: HashSet<&str> = ms.iter().map(|m| m.path.as_str()).collect();
            ShapeCluster {
                signature,
                count: ms.len(),
                files: files.len(),
                examples: ms.into_iter().take(4).collect(),
            }
        })
        .collect();
    report.clusters.sort_by(|a, b| (b.files, b.count).cmp(&(a.files, a.count)));

    println!(
        "shape: {} files checked ({} skipped), {} oracle node(s), {} missed in {} file(s), {} cluster(s)",
        report.files_checked,
        report.files_skipped,
        report.oracle_nodes,
        report.missed_nodes,
        report.files_with_misses,
        report.clusters.len(),
    );
    for c in report.clusters.iter().take(12) {
        println!("  {:>6} files {:>7}x  {}", c.files, c.count, c.signature);
        if let Some(e) = c.examples.first() {
            println!("            e.g. {}  {:?}", e.path, e.text);
        }
    }

    std::fs::create_dir_all(out.parent().unwrap_or(Path::new(".")))?;
    std::fs::write(out, serde_json::to_string_pretty(&report)?)?;
    println!("shape: report at {}", out.display());

    // A fixture directory is a ZERO ratchet -- every file in it is there
    // because a specific mis-parse was fixed, so any miss is that mis-parse
    // coming back.
    let ceiling = if dir.is_some() { Some(0) } else { baseline };
    // Only meaningful over the whole set; a --limit run checks a prefix and
    // would trip the ratchet for no reason.
    if let (Some(max), None) = (ceiling, limit) {
        anyhow::ensure!(
            report.missed_nodes <= max,
            "shape: {} missed boundaries, baseline is {}. Either a change \
             regrouped the tree -- read the clusters above, they name what \
             was built instead -- or the corpus grew and the baseline in \
             {}/shape_policy.json needs raising DELIBERATELY.",
            report.missed_nodes,
            max,
            grammar_dir.display(),
        );
        println!("shape: {} <= baseline {}", report.missed_nodes, max);
    }
    Ok(())
}

/// The source extensions a language's fixture directory may hold.
fn extensions(lang: LangName) -> &'static [&'static str] {
    match lang {
        LangName::Python => &["py", "pyi"],
        LangName::Rust => &["rs"],
        LangName::Typescript | LangName::Javascript => {
            &["ts", "tsx", "mts", "cts", "js", "jsx", "mjs", "cjs"]
        }
    }
}

/// Source files under a fixture directory, as paths relative to it.
fn collect(root: &Path, dir: &Path, exts: &[&str], out: &mut Vec<String>) -> Result<()> {
    for entry in std::fs::read_dir(dir).with_context(|| format!("read {}", dir.display()))? {
        let path = entry?.path();
        if path.is_dir() {
            collect(root, &path, exts, out)?;
        } else if path
            .extension()
            .and_then(|e| e.to_str())
            .is_some_and(|e| exts.contains(&e))
        {
            out.push(
                path.strip_prefix(root)
                    .unwrap_or(&path)
                    .to_string_lossy()
                    .into_owned(),
            );
        }
    }
    Ok(())
}
