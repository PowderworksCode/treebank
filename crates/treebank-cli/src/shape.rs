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
    /// Boundaries that agree while the kinds do not — the mapping is
    /// declared and the tree does not honour it.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub mismatches: Vec<ShapeCluster>,
    /// Oracle kinds the mapping never mentions. Holes, not defects.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub unmapped: Vec<ShapeCluster>,
    #[serde(default)]
    pub mismatched_nodes: usize,
    #[serde(default)]
    pub unmapped_nodes: usize,
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
    /// Kind pairings the mapping is allowed to fail on, as the full
    /// `"<OracleKind> => <our|kinds>"` signature the report prints. The whole
    /// signature, not the oracle kind alone: forgiving `Tuple` outright would
    /// forgive every Tuple confusion, where forgiving `Tuple => identifier`
    /// forgives only the case where an identifier was the ONLY thing we had.
    #[serde(default)]
    mismatch_ignore: Vec<Ignored>,
}

#[derive(Deserialize)]
struct Ignored {
    signature: String,
}

fn load_policy(grammar_dir: &Path) -> Result<(HashSet<String>, Option<usize>, HashSet<String>)> {
    let path = grammar_dir.join("shape_policy.json");
    if !path.exists() {
        return Ok((Default::default(), None, Default::default()));
    }
    let text = std::fs::read_to_string(&path)
        .with_context(|| format!("read {}", path.display()))?;
    let policy: ShapePolicy = serde_json::from_str(&text)
        .with_context(|| format!("parse {}", path.display()))?;
    Ok((
        policy.ignore.into_iter().map(|i| i.signature).collect(),
        policy.baseline_missed,
        policy.mismatch_ignore.into_iter().map(|i| i.signature).collect(),
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

/// The declared correspondence between the reference parser's node kinds and
/// ours: `"<OracleKind>": ["our_kind", ...]`.
///
/// This is the part a boundary comparison cannot do. Where the two parsers
/// agree on the bytes, the question left is whether they agree on WHAT is
/// there -- and they can disagree completely while covering identical spans.
/// `foo();` parsed as a bodyless function declaration occupies exactly the
/// bytes of the call it should be, so nothing about the boundaries is wrong;
/// only the names are, and only a table can say so.
///
/// Not required to be one-to-one. Several of our kinds may answer one oracle
/// kind (`Expr::Lit` is `integer`, `float`, `string`, `boolean_literal`, ...)
/// and one of ours may answer several oracle kinds. What it must be is
/// TOTAL and DECLARED: every oracle kind the corpus produces has an entry,
/// and every pair the corpus produces is in it. A mapping with holes is a
/// mapping nobody is checking.
fn load_node_map(grammar_dir: &Path) -> Result<(HashMap<String, HashSet<String>>, bool)> {
    #[derive(Deserialize, Default)]
    struct NodeMap {
        #[serde(default)]
        map: HashMap<String, Vec<String>>,
    }
    let path = grammar_dir.join("node_map.json");
    if !path.exists() {
        return Ok((Default::default(), false));
    }
    let text = std::fs::read_to_string(&path)
        .with_context(|| format!("read {}", path.display()))?;
    let m: NodeMap = serde_json::from_str(&text)
        .with_context(|| format!("parse {}", path.display()))?;
    // Present-but-empty is how the table is BOOTSTRAPPED: every oracle kind
    // then reports as unmapped, together with the kinds of ours actually
    // found at its span, which is the raw material for writing the entries.
    Ok((
        m.map.into_iter().map(|(k, v)| (k, v.into_iter().collect())).collect(),
        true,
    ))
}

/// One file's contribution, kept as a struct because there are now three
/// kinds of finding and a tuple of seven had stopped being readable.
struct FileResult {
    oracle_nodes: usize,
    missed: usize,
    had_miss: bool,
    skipped: bool,
    /// The oracle saw a boundary we have no node for.
    misses: Vec<Miss>,
    /// The boundary agrees and the KINDS do not.
    mismatches: Vec<Miss>,
    /// An oracle kind the mapping does not mention.
    unmapped: Vec<Miss>,
}

impl FileResult {
    fn skipped() -> Self {
        FileResult {
            oracle_nodes: 0,
            missed: 0,
            had_miss: false,
            skipped: true,
            misses: Vec::new(),
            mismatches: Vec::new(),
            unmapped: Vec::new(),
        }
    }
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
/// Span -> the kinds of every node of ours that occupies it.
///
/// A SET of kinds, not one, and that is not a compromise: a chain like
/// `expression_statement > call_expression > identifier` can have all three
/// nodes covering the same bytes, and the reference parser will name exactly
/// one of them. The question the mapping asks is "is the thing we built at
/// these bytes one of the things this oracle kind is allowed to be", and a
/// chain answers it honestly.
fn our_spans(root: tree_sitter::Node, src: &[u8]) -> HashMap<(usize, usize), Vec<&'static str>> {
    let mut out: HashMap<(usize, usize), Vec<&'static str>> = HashMap::new();
    let mut cursor = root.walk();
    let mut recurse = true;
    loop {
        if recurse {
            let n = cursor.node();
            let kind = n.kind();
            let mut add = |span: (usize, usize)| {
                let slot = out.entry(span).or_default();
                if !slot.contains(&kind) {
                    slot.push(kind);
                }
            };
            let (a, b) = (n.start_byte(), n.end_byte());
            add((a, b));
            add((a, trim_end(src, a, b.min(src.len()))));
            // ...and the same node with any trailing trivia it owns removed,
            // then separator-trimmed again, since the trivia may sit after a
            // terminator.
            let c = content_end(n);
            if c > a && c < b {
                add((a, c));
                add((a, trim_end(src, a, c.min(src.len()))));
            }
            // ...and the same node counting the trivia in FRONT of it as its
            // own, which is where the two parsers disagree about doc
            // comments.
            for lead in leading_starts(n) {
                if lead >= a {
                    continue;
                }
                add((lead, b));
                add((lead, trim_end(src, lead, b.min(src.len()))));
                if c > lead && c < b {
                    add((lead, c));
                    add((lead, trim_end(src, lead, c.min(src.len()))));
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

    let (ignore, baseline, mismatch_ignore) = load_policy(grammar_dir)?;
    let (node_map, has_map) = load_node_map(grammar_dir)?;
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
        mismatches: Vec::new(),
        unmapped: Vec::new(),
        mismatched_nodes: 0,
        unmapped_nodes: 0,
    };
    let mut by_sig: BTreeMap<String, Vec<Miss>> = BTreeMap::new();
    let mut by_mismatch: BTreeMap<String, Vec<Miss>> = BTreeMap::new();
    let mut by_unmapped: BTreeMap<String, Vec<Miss>> = BTreeMap::new();

    for chunk in files.chunks(BATCH) {
        let batch: Vec<String> = chunk.to_vec();
        let oracle_spans = oracle.spans(&corpus_src, &batch)?;

        let results: Vec<FileResult> = batch
            .par_iter()
            .map(|rel| -> Result<FileResult> {
                let Some(file) = oracle_spans.get(rel) else {
                    // The oracle must answer every path it was asked about;
                    // a missing answer is an oracle failure, not a pass.
                    anyhow::bail!("ts-oracle returned no span record for {rel}");
                };
                if file.skipped.is_some() {
                    return Ok(FileResult::skipped());
                }
                let src = std::fs::read(corpus_src.join(rel))?;
                let idx = dialect.get(rel.as_str()).copied().unwrap_or(0);
                let mut parser = Parser::new();
                parser.set_language(&langs[idx].0)?;
                let Some(tree) = parser.parse(&src, None) else {
                    return Ok(FileResult::skipped());
                };
                // A file we cannot parse is the SWEEP's business, not this
                // check's; comparing shapes against an error tree is noise.
                if tree.root_node().has_error() {
                    return Ok(FileResult::skipped());
                }
                let ours = our_spans(tree.root_node(), &src);
                let mut misses = Vec::new();
                let mut mismatches = Vec::new();
                let mut unmapped = Vec::new();
                for s in &file.spans {
                    // Two questions per oracle node, and the second only
                    // makes sense once the first is answered yes:
                    //   1. do we have a node with these bytes at all?
                    //   2. is it the KIND we said this oracle kind is?
                    let trimmed = trim_end(&src, s.start, s.end.min(src.len()));
                    let exact = ours.get(&(s.start, s.end));
                    let at = exact.or_else(|| ours.get(&(s.start, trimmed)));
                    if let Some(kinds) = at {
                        if let Some(expected) = node_map.get(&s.kind) {
                            // `"*"` marks a WRAPPER kind: one whose span
                            // coincides with its only child, so the child's
                            // own entry already carries the check and a
                            // second one here would say nothing. syn's
                            // `Stmt::Expr` is the case -- for a block's tail
                            // expression it spans exactly the expression, and
                            // we build no statement node at all. Narrow on
                            // purpose: it is a claim about the ORACLE's
                            // shape, not a way to silence a kind.
                            let agrees = expected.contains("*")
                                || kinds.iter().any(|k| expected.contains(*k));
                            // A trim-only match whose kinds do not line up is
                            // not a mismatch -- it is not a match at all. The
                            // separator trim exists to forgive punctuation
                            // between two parsers naming the SAME node; when
                            // the names say they are not the same node, the
                            // honest reading is that we have no counterpart
                            // here, and this belongs on the boundary side
                            // where the granularity policy can speak to it.
                            // `x[a,]` is the case: CPython's one-element
                            // Tuple trims to `a`, which is our identifier,
                            // and we build no tuple node there at all.
                            if !agrees && exact.is_none() {
                                // fall through to the boundary-miss path
                            } else if !agrees {
                                // The boundary agrees and the NAMES do not.
                                // This is the class a boundary check is blind
                                // to by construction: `foo();` parsed as a
                                // bodyless function declaration spans exactly
                                // the same bytes as the call it should be.
                                let mut got: Vec<&str> = kinds.to_vec();
                                got.sort_unstable();
                                let signature = format!("{} => {}", s.kind, got.join("|"));
                                if mismatch_ignore.contains(&signature) {
                                    continue;
                                }
                                let text = String::from_utf8_lossy(
                                    &src[s.start.min(src.len())..s.end.min(src.len())],
                                );
                                let text: String = text.chars().take(60).collect();
                                mismatches.push(Miss {
                                    path: rel.to_string(),
                                    kind: signature,
                                    start: s.start,
                                    end: s.end,
                                    text: text.replace('\n', "\\n"),
                                });
                                continue;
                            } else {
                                continue;
                            }
                        } else if has_map {
                            // An oracle kind with no entry at all. Not a
                            // parse defect -- a hole in the table, which is
                            // exactly as worth knowing.
                            let text = String::from_utf8_lossy(
                                &src[s.start.min(src.len())..s.end.min(src.len())],
                            );
                            let text: String = text.chars().take(60).collect();
                            let mut got: Vec<&str> = kinds.to_vec();
                            got.sort_unstable();
                            unmapped.push(Miss {
                                path: rel.to_string(),
                                kind: format!("{} => {}", s.kind, got.join("|")),
                                start: s.start,
                                end: s.end,
                                text: text.replace('\n', "\\n"),
                            });
                            continue;
                        } else {
                            continue;
                        }
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
                Ok(FileResult {
                    oracle_nodes: file.spans.len(),
                    missed: n,
                    had_miss: n > 0,
                    skipped: false,
                    misses,
                    mismatches,
                    unmapped,
                })
            })
            .collect::<Result<_>>()?;

        for r in results {
            report.oracle_nodes += r.oracle_nodes;
            report.missed_nodes += r.missed;
            if r.skipped {
                report.files_skipped += 1;
            } else {
                report.files_checked += 1;
            }
            if r.had_miss {
                report.files_with_misses += 1;
            }
            for m in r.misses {
                by_sig.entry(m.kind.clone()).or_default().push(m);
            }
            for m in r.mismatches {
                by_mismatch.entry(m.kind.clone()).or_default().push(m);
            }
            for m in r.unmapped {
                by_unmapped.entry(m.kind.clone()).or_default().push(m);
            }
        }
    }

    let cluster_of = |(signature, ms): (String, Vec<Miss>)| {
        let files: HashSet<&str> = ms.iter().map(|m| m.path.as_str()).collect();
        ShapeCluster {
            signature,
            count: ms.len(),
            files: files.len(),
            examples: ms.into_iter().take(4).collect(),
        }
    };
    report.mismatched_nodes = by_mismatch.values().map(|v| v.len()).sum();
    report.unmapped_nodes = by_unmapped.values().map(|v| v.len()).sum();
    report.mismatches = by_mismatch.into_iter().map(cluster_of).collect();
    report.mismatches.sort_by(|a, b| (b.files, b.count).cmp(&(a.files, a.count)));
    report.unmapped = by_unmapped.into_iter().map(cluster_of).collect();
    report.unmapped.sort_by(|a, b| (b.files, b.count).cmp(&(a.files, a.count)));

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
    if has_map {
        println!(
            "shape: mapping — {} kind mismatch(es) in {} cluster(s), {} unmapped oracle node(s) in {} kind(s)",
            report.mismatched_nodes,
            report.mismatches.len(),
            report.unmapped_nodes,
            report.unmapped.len(),
        );
        for c in report.mismatches.iter().take(12) {
            println!("  MISMATCH {:>5} files {:>7}x  {}", c.files, c.count, c.signature);
            if let Some(e) = c.examples.first() {
                println!("            e.g. {}  {:?}", e.path, e.text);
            }
        }
        for c in report.unmapped.iter().take(12) {
            println!("  UNMAPPED {:>5} files {:>7}x  {}", c.files, c.count, c.signature);
            if let Some(e) = c.examples.first() {
                println!("            e.g. {}  {:?}", e.path, e.text);
            }
        }
    }
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
