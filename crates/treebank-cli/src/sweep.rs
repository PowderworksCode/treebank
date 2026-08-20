use std::collections::{BTreeMap, HashMap};
use std::path::Path;

use anyhow::{bail, Context, Result};
use rayon::prelude::*;
use serde::{Deserialize, Serialize};
use tree_sitter::{Node, Parser};

use crate::grammar;
use treebank_corpus::fetch::Manifest;

#[derive(Serialize, Deserialize, Clone)]
pub struct Failure {
    pub package: String,
    pub path: String,
    pub line: usize,
    pub signature: String,
    pub snippet: String,
}

#[derive(Serialize, Deserialize)]
pub struct Cluster {
    pub signature: String,
    /// "gap" (valid code the grammar rejects — fix the grammar), "config"
    /// (valid code the grammar cannot represent AS WRITTEN, because a
    /// preprocessor conditional splits a construct; not a grammar bug — see
    /// `treebank_preprocessing`) or "noise" (the reference parser rejects
    /// these files too — ignore).
    pub verdict: String,
    pub count: usize,
    pub valid: usize,
    pub examples: Vec<Failure>,
    /// Failing files the reference parser says are VALID — the fix targets.
    pub valid_paths: Vec<String>,
    /// Valid files that parse cleanly once dead preprocessor branches are
    /// removed. Excluded from `valid_paths`: no grammar change fixes these.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub config_paths: Vec<String>,
    /// Gap files that parse cleanly once the package's macros are expanded.
    /// These ARE still gaps — a grammar could parse them — but the failure is
    /// caused by an unexpanded macro rather than by unsupported syntax.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub macro_paths: Vec<String>,
    /// Macros expanded at the error site, most common first: the shapes a fix
    /// has to support, and the ones to test it against.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub macros: Vec<String>,
    /// Valid-in-SOME-version files the grammar rejects ON PURPOSE, because
    /// `version_policy.toml` declares the construct rejected and the CURRENT
    /// version's oracle rejects it too (DESIGN.md §4.2). Excluded from
    /// `valid_paths`: no grammar change should fix these.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub version_paths: Vec<String>,
    pub paths: Vec<String>,
}

#[derive(Serialize, Deserialize)]
pub struct Report {
    pub lang: String,
    pub grammar: String,
    pub files: usize,
    pub passed: usize,
    pub failed: usize,
    pub gap_files: usize,
    /// Valid files whose only problem is that a preprocessor conditional
    /// splits a construct the grammar must see whole. Counted apart from
    /// both gaps and noise because neither name is true of them.
    #[serde(default)]
    pub config_files: usize,
    /// Files rejected by declared version policy. Counted apart from gaps
    /// because they are decisions, and apart from noise because the code is
    /// valid in a version the language once had.
    #[serde(default)]
    pub version_files: usize,
    /// Noise files that the PARSER alone would have accepted. The oracle
    /// judges with `compile`, which also runs the checks CPython performs
    /// after parsing, and its script's header states the cost of that: a
    /// file invalid for a post-parse reason AND holding a real grammar gap
    /// is recorded as noise. This is that cost, counted rather than assumed
    /// small. Every one of these is a gap the sweep cannot see.
    #[serde(default)]
    pub hidden_gap_files: usize,
    /// Their paths, so they can be read.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub hidden_gaps: Vec<String>,
    pub noise_files: usize,
    pub clusters: Vec<Cluster>,
}

/// Signatures a grammar declares it rejects on purpose, from
/// `version_policy.toml` (DESIGN.md §4.2). Absent file means no declarations,
/// which is the normal case; a malformed one is an error, because silently
/// treating it as empty would turn declared rejections back into gaps and
/// send a fix agent chasing decisions.
fn load_version_policy(grammar_dir: &Path) -> anyhow::Result<std::collections::HashSet<String>> {
    #[derive(Deserialize)]
    struct Rejection {
        signature: String,
    }
    #[derive(Deserialize)]
    struct Policy {
        #[serde(default)]
        rejections: Vec<Rejection>,
    }
    let path = grammar_dir.join("version_policy.toml");
    if !path.exists() {
        return Ok(Default::default());
    }
    let text =
        std::fs::read_to_string(&path).with_context(|| format!("read {}", path.display()))?;
    let policy: Policy =
        toml::from_str(&text).with_context(|| format!("parse {}", path.display()))?;
    Ok(policy.rejections.into_iter().map(|r| r.signature).collect())
}

/// Byte offset of the first ERROR or MISSING node, for the error-position
/// check. Same traversal `first_error` uses; exposed so `errpos` does not
/// need its own idea of where a rejection happened.
pub fn first_error_offset(root: Node) -> Option<usize> {
    first_error(root).map(|n| n.start_byte())
}

/// First ERROR or MISSING node in document order.
fn first_error<'a>(root: Node<'a>) -> Option<Node<'a>> {
    let mut cursor = root.walk();
    loop {
        let node = cursor.node();
        if node.is_error() || node.is_missing() {
            return Some(node);
        }
        // Descend only into subtrees that contain an error.
        if node.has_error() && cursor.goto_first_child() {
            continue;
        }
        loop {
            if cursor.goto_next_sibling() {
                break;
            }
            if !cursor.goto_parent() {
                return None;
            }
        }
    }
}

/// Signature, 1-based line and source line for a tree's first error, so a
/// round-trip failure clusters and reads exactly like a sweep gap.
pub fn error_signature(root: Node, text: &str) -> (String, usize, String) {
    let src = text.as_bytes();
    match first_error(root) {
        None => ("<no error>".into(), 0, String::new()),
        Some(node) => {
            let sig = signature_of(node, src);
            let at = node.start_byte().min(src.len());
            let lo = src[..at]
                .iter()
                .rposition(|b| *b == b'\n')
                .map(|i| i + 1)
                .unwrap_or(0);
            let hi = src[at..]
                .iter()
                .position(|b| *b == b'\n')
                .map(|i| at + i)
                .unwrap_or(src.len());
            let line = src[..at].iter().filter(|b| **b == b'\n').count() + 1;
            (
                sig,
                line,
                String::from_utf8_lossy(&src[lo..hi])
                    .chars()
                    .take(90)
                    .collect(),
            )
        }
    }
}

fn signature_of(node: Node, src: &[u8]) -> String {
    let parent = node
        .parent()
        .map(|p| p.kind().to_string())
        .unwrap_or_else(|| "<root>".into());
    if node.is_missing() {
        return format!("{parent} > MISSING {}", node.kind());
    }
    let mut kinds = Vec::new();
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        kinds.push(child.kind().to_string());
        if kinds.len() == 4 {
            kinds.push("…".into());
            break;
        }
    }
    if kinds.is_empty() {
        // Leaf ERROR: use the (normalized) unexpected text itself.
        let text = String::from_utf8_lossy(&src[node.byte_range()]);
        let text: String = text.split_whitespace().collect::<Vec<_>>().join(" ");
        let text: String = text.chars().take(20).collect();
        return format!("{parent} > ERROR '{text}'");
    }
    format!("{parent} > ERROR({})", kinds.join(" "))
}

/// The line number is counted in the RAW BYTES rather than in the lossy
/// string, and that is not a style choice. `from_utf8_lossy` replaces each
/// invalid byte with U+FFFD, which is three bytes, so every index past the
/// first bad byte means something different in the two strings — slicing the
/// lossy text at a byte offset taken from the source panicked outright
/// ("end byte index 15427 is not a char boundary") the first time a corpus
/// contained non-UTF-8. HTML is the language that found it: a repository
/// ships whatever encoding its author saved, and 26 files of the 132,492 in
/// this corpus are not UTF-8. Counting newlines in the bytes is both
/// panic-free and correct, because a newline is one byte in every encoding
/// this can meet.
fn snippet_at(src: &[u8], byte: usize) -> (usize, String) {
    let text = String::from_utf8_lossy(src);
    let line_no = src[..byte.min(src.len())]
        .iter()
        .filter(|b| **b == b'\n')
        .count()
        + 1;
    let line = text.lines().nth(line_no - 1).unwrap_or("");
    let line: String = line.trim().chars().take(160).collect();
    (line_no, line)
}

pub fn check_file(parser: &mut Parser, package: &str, rel: &str, src: &[u8]) -> Option<Failure> {
    let Some(tree) = parser.parse(src, None) else {
        return Some(Failure {
            package: package.into(),
            path: rel.into(),
            line: 0,
            signature: "<parse returned no tree>".into(),
            snippet: String::new(),
        });
    };
    let root = tree.root_node();
    if !root.has_error() {
        return None;
    }
    let node = first_error(root)?;
    let (line, snippet) = snippet_at(src, node.start_byte());
    Some(Failure {
        package: package.into(),
        path: rel.into(),
        line,
        signature: signature_of(node, src),
        snippet,
    })
}

#[derive(Serialize, Deserialize, Default)]
struct SweepCache {
    /// Fingerprint of the compiled grammar this cache is valid for.
    grammar: String,
    /// sha256 of every file that PASSED under that grammar. Failing files
    /// are always re-parsed (their diagnostics must stay fresh).
    passed_sha256: Vec<String>,
}

/// Resolve as many *split constructs* as a file has, one error at a time.
///
/// A conditional that splits a construct makes the grammar fail on code that
/// is valid in every configuration — see `treebank_preprocessing::branches`.
/// Files routinely contain several, so this walks: find the first error, force
/// the conditional enclosing it, keep whichever choice moves the error PAST
/// that line while leaving the line itself intact, and go again.
///
/// Returns the failure that branch forcing could not explain — `None` when the
/// whole file comes out clean — and how many splits were resolved on the way.
/// The returned failure is the honest one to cluster on: a file whose first
/// error is a split belongs with whatever its *next* problem is, not with the
/// split it was previously filed under.
fn resolve_splits(
    parser: &mut Parser,
    original: &Failure,
    source: &str,
) -> (Option<Failure>, usize) {
    const MAX_SPLITS: usize = 8;
    let mut text = source.to_string();
    let mut current = original.clone();
    let mut resolved = 0;
    for _ in 0..MAX_SPLITS {
        let Some(region) = treebank_preprocessing::innermost_containing(&text, current.line) else {
            break;
        };
        let mut progressed = false;
        for keep_if in [true, false] {
            let variant = treebank_preprocessing::force_branch(&text, &region, keep_if);
            // The guard: an error that vanished with its own line proves
            // nothing (every header is wrapped in an include guard).
            if !treebank_preprocessing::line_survives(&variant, current.line) {
                continue;
            }
            match check_file(parser, &current.package, &current.path, variant.as_bytes()) {
                None => return (None, resolved + 1),
                Some(next) if next.line > current.line => {
                    text = variant;
                    current = next;
                    resolved += 1;
                    progressed = true;
                    break;
                }
                Some(_) => {}
            }
        }
        if !progressed {
            break;
        }
    }
    (Some(current), resolved)
}

/// Rewrite the grammar's `[corpus.sweep]` block from THIS run. The ledger
/// is the evidence file, and transcribing measurements into it by hand is
/// how java's sat at 811 while the truth was 167 (issue #145). Only the
/// numbers move; everything else in the ledger stays prose, and a grammar
/// dir without a ledger (an upstream checkout under comparison) is left
/// alone.
fn write_ledger_block(grammar_dir: &Path, lang: treebank_lang::LangName, r: &Report) -> Result<()> {
    let path = grammar_dir.join("ledger.toml");
    let Ok(text) = std::fs::read_to_string(&path) else {
        return Ok(());
    };
    let block_body = format!(
        "\nfiles = {}\npassed = {}\nfailed = {}\ngap_files = {}\nnoise_files = {}\npass_rate = '{:.2}%'\n",
        r.files,
        r.passed,
        r.failed,
        r.gap_files,
        r.noise_files,
        100.0 * r.passed as f64 / r.files.max(1) as f64,
    );
    // One grammar may carry several corpora (typescript also sweeps the
    // javascript corpus), so a per-language block name is tried first.
    let per_lang = format!("[corpus.{lang}_sweep]");
    let (header, start) = if let Some(i) = text.find(per_lang.as_str()) {
        (per_lang.as_str(), i)
    } else if let Some(i) = text.find("[corpus.sweep]") {
        ("[corpus.sweep]", i)
    } else {
        return Ok(()); // no block declared; not ours to invent
    };
    // The block ends at the next section header or EOF.
    let after = &text[start + 1..];
    let end = after
        .find("\n[")
        .map(|i| start + 1 + i + 1)
        .unwrap_or(text.len());
    let mut new = String::new();
    new.push_str(&text[..start]);
    new.push_str(header);
    new.push_str(&block_body);
    if end < text.len() {
        new.push('\n');
        new.push_str(&text[end..]);
    }
    if new != text {
        std::fs::write(&path, new)?;
        println!("sweep: ledger {header} updated at {}", path.display());
    }
    Ok(())
}

pub fn run(
    lang: treebank_lang::LangName,
    grammar_dir: &Path,
    manifest_path: &Path,
    out: &Path,
) -> Result<()> {
    let (language, fingerprint) = grammar::load(grammar_dir)?;
    let manifest = Manifest::load(manifest_path)?;
    let corpus_root = manifest_path.parent().unwrap();
    let corpus_src = corpus_root.join("src");
    let files = manifest.files();

    // Incremental sweeps: files whose content already passed under this
    // exact grammar build are skipped. Any grammar change changes the
    // fingerprint and forces a full re-sweep.
    let cache_path = corpus_root.join("sweep-cache.json");
    let cache: SweepCache = std::fs::read_to_string(&cache_path)
        .ok()
        .and_then(|s| serde_json::from_str(&s).ok())
        .filter(|c: &SweepCache| c.grammar == fingerprint)
        .unwrap_or_default();
    let known_pass: std::collections::HashSet<&str> =
        cache.passed_sha256.iter().map(|s| s.as_str()).collect();
    let (skipped, work): (Vec<_>, Vec<_>) = files
        .iter()
        .partition(|f| known_pass.contains(f.sha256.as_str()));
    eprintln!(
        "sweep: {} files against {} ({} unchanged-and-passing, {} to parse)",
        files.len(),
        grammar_dir.display(),
        skipped.len(),
        work.len()
    );

    let results: Vec<(String, Option<Failure>)> = work
        .par_iter()
        .map_init(
            || {
                let mut p = Parser::new();
                p.set_language(&language)
                    .expect("language/runtime ABI mismatch");
                p
            },
            |parser, f| {
                let full = corpus_src.join(&f.pkgdir).join(&f.rel);
                let Ok(src) = std::fs::read(&full) else {
                    return None;
                };
                let package = f
                    .pkgdir
                    .rsplitn(2, '-')
                    .last()
                    .unwrap_or(&f.pkgdir)
                    .to_string();
                let failure =
                    check_file(parser, &package, &format!("{}/{}", f.pkgdir, f.rel), &src);
                Some((f.sha256.clone(), failure))
            },
        )
        .flatten()
        .collect();
    let failures: Vec<Failure> = results.iter().filter_map(|(_, f)| f.clone()).collect();

    // Persist the cache: previously-known passes (still present in the
    // corpus) plus this run's fresh passes.
    let mut passed_sha256: Vec<String> = skipped.iter().map(|f| f.sha256.clone()).collect();
    passed_sha256.extend(
        results
            .iter()
            .filter(|(_, f)| f.is_none())
            .map(|(sha, _)| sha.clone()),
    );
    passed_sha256.sort();
    passed_sha256.dedup();
    std::fs::write(
        &cache_path,
        serde_json::to_string(&SweepCache {
            grammar: fingerprint,
            passed_sha256,
        })?,
    )?;

    // Adjudicate every failing file with the language's reference parser so
    // the report separates grammar gaps from corpus noise.
    let failing_paths: Vec<String> = failures.iter().map(|f| f.path.clone()).collect();
    let validity = treebank_oracle::get(lang).validate(&corpus_src, &failing_paths)?;

    // The oracle must answer every question it was asked. A path with no
    // verdict reads as `false` at all three use sites below — that is, as
    // "the reference parser rejected it" — so it is filed as corpus noise
    // and gap_files drops, silently and in the direction that flatters the
    // grammar. The ways to get here are unglamorous and real: an oracle that
    // exits 0 after answering half a batch, or a path that does not survive
    // the round trip through the oracle and back (stdin_oracle re-derives
    // the key with strip_prefix, so a symlinked or otherwise renormalised
    // path silently fails to match). exec_oracle cannot hit this because it
    // builds its map from the input list; the stdin oracles can.
    let missing: Vec<&str> = failing_paths
        .iter()
        .filter(|p| !validity.contains_key(*p))
        .map(|p| p.as_str())
        .collect();
    anyhow::ensure!(
        missing.is_empty(),
        "oracle returned no verdict for {} of {} failing files. This is an \
         oracle failure, not a verdict: counting them invalid would file them \
         as corpus noise and understate gap_files. First few: {:?}",
        missing.len(),
        failing_paths.len(),
        &missing[..missing.len().min(5)],
    );

    // Declared version-policy rejections (DESIGN.md §4.2). TWO conditions,
    // both required: the cluster signature is declared in
    // `version_policy.toml`, AND the CURRENT version's oracle rejects the
    // file. The second is what keeps a declaration from becoming a
    // self-granted exemption — a policy entry can never suppress a failure on
    // code that is still valid today, so a real gap cannot hide behind one.
    let declared_versions = load_version_policy(grammar_dir)?;
    let version_only: std::collections::HashSet<String> = if declared_versions.is_empty() {
        Default::default()
    } else {
        // Only the files the UNION oracle called valid can be version-only;
        // the rest are noise and already classified.
        let valid_failing: Vec<String> = failing_paths
            .iter()
            .filter(|p| validity.get(*p).copied().unwrap_or(false))
            .cloned()
            .collect();
        let current = treebank_oracle::get(lang).validate_current(&corpus_src, &valid_failing)?;
        valid_failing
            .into_iter()
            .filter(|p| current.get(p).copied() == Some(false))
            .collect()
    };

    // A grammar sees every #if branch at once; a compiler sees only the live
    // ones. Where removing the branches a compiler would have dropped makes a
    // file parse cleanly, the rejection is a property of the preprocessor and
    // no grammar patch can fix it — so it must not be filed as a gap, where it
    // would sit at the top of the queue absorbing a fix agent's attempts.
    let mut config_inherent: std::collections::HashSet<String> =
        match crate::routing::preprocessing(lang) {
            None => Default::default(),
            Some(symbols) => {
                let hits: Vec<String> = failures
                    .par_iter()
                    .filter(|f| validity.get(&f.path).copied().unwrap_or(false))
                    .filter_map(|f| {
                        let src = std::fs::read_to_string(corpus_src.join(&f.path)).ok()?;
                        let reduced = treebank_preprocessing::reduce(&src, symbols);
                        if !reduced.changed() {
                            return None;
                        }
                        let mut parser = Parser::new();
                        parser.set_language(&language).ok()?;
                        let tree = parser.parse(reduced.text.as_bytes(), None)?;
                        (!tree.root_node().has_error()).then(|| f.path.clone())
                    })
                    .collect();
                if !hits.is_empty() {
                    eprintln!(
                        "preprocessing: {} valid file(s) parse cleanly once dead branches are \
                     removed — counted as configuration-inherent, not grammar gaps",
                        hits.len()
                    );
                }
                hits.into_iter().collect()
            }
        };

    // Conditionals whose symbols nobody declared can split a construct just
    // as `#ifdef __cplusplus` does — `#ifdef F_DUPFD_CLOEXEC` around one of
    // two spellings of a function signature. Nothing here needs to know what
    // the symbol means: if forcing either branch removes the error, the
    // grammar was failing only because it must see both at once.
    let mut split_resolved: HashMap<String, Option<Failure>> = HashMap::new();
    if crate::routing::preprocessing(lang).is_some() {
        let candidates: Vec<&Failure> = failures
            .iter()
            .filter(|f| validity.get(&f.path).copied().unwrap_or(false))
            .filter(|f| !config_inherent.contains(&f.path))
            .collect();
        let found: Vec<(String, Option<Failure>, usize)> = candidates
            .par_iter()
            .filter_map(|f| {
                let src = std::fs::read_to_string(corpus_src.join(&f.path)).ok()?;
                let mut parser = Parser::new();
                parser.set_language(&language).ok()?;
                let (remaining, resolved) = resolve_splits(&mut parser, f, &src);
                (resolved > 0).then(|| (f.path.clone(), remaining, resolved))
            })
            .collect();
        let (mut whole, mut partial) = (0usize, 0usize);
        for (path, remaining, _) in found {
            if remaining.is_none() {
                whole += 1;
                config_inherent.insert(path.clone());
            } else {
                partial += 1;
            }
            split_resolved.insert(path, remaining);
        }
        if whole + partial > 0 {
            eprintln!(
                "preprocessing: {whole} file(s) explained entirely by split constructs; \
                 {partial} more had a split ahead of their real problem and are \
                 re-clustered on that"
            );
        }
    }

    // Macro expansion, for diagnosis only. Unlike the conditional case above
    // this never changes a verdict: `THREAD_LOCAL int x;` is something a
    // grammar could parse, so it stays a gap. What it adds is WHICH macro,
    // which is what writing a minimal rule — and judging whether that rule
    // over-accepts — actually requires.
    let mut macro_clean: std::collections::HashSet<String> = Default::default();
    let mut macro_names: HashMap<String, Vec<String>> = HashMap::new();
    if crate::routing::preprocessing(lang).is_some() {
        // One macro census per package, from the files already in the corpus.
        let mut by_pkg: BTreeMap<String, Vec<&treebank_corpus::fetch::FileEntry>> = BTreeMap::new();
        for f in &files {
            by_pkg.entry(f.pkgdir.clone()).or_default().push(f);
        }
        let gap_by_pkg: BTreeMap<String, Vec<&Failure>> = failures
            .iter()
            .filter(|f| validity.get(&f.path).copied().unwrap_or(false))
            .filter(|f| !config_inherent.contains(&f.path))
            .fold(BTreeMap::new(), |mut acc, f| {
                let pkg = f.path.split('/').next().unwrap_or("").to_string();
                acc.entry(pkg).or_default().push(f);
                acc
            });
        for (pkg, gaps) in &gap_by_pkg {
            let Some(entries) = by_pkg.get(pkg) else {
                continue;
            };
            let mut macros = treebank_preprocessing::Macros::new();
            for e in entries {
                if let Ok(src) = std::fs::read_to_string(corpus_src.join(&e.pkgdir).join(&e.rel)) {
                    macros.add_source(&src);
                }
            }
            let found: Vec<(String, Vec<String>)> = gaps
                .par_iter()
                .filter_map(|f| {
                    let src = std::fs::read_to_string(corpus_src.join(&f.path)).ok()?;
                    let e = treebank_preprocessing::expand(&src, &macros);
                    if !e.changed() {
                        return None;
                    }
                    let mut parser = Parser::new();
                    parser.set_language(&language).ok()?;
                    let tree = parser.parse(e.text.as_bytes(), None)?;
                    if tree.root_node().has_error() {
                        return None;
                    }
                    let names: Vec<String> =
                        e.near(f.line).into_iter().map(str::to_string).collect();
                    Some((f.path.clone(), names))
                })
                .collect();
            let hit = found.len();
            for (path, names) in found {
                macro_clean.insert(path.clone());
                macro_names.insert(path, names);
            }
            eprintln!(
                "macros: {pkg} — {} definitions, {hit} of {} gap files parse once expanded",
                macros.len(),
                gaps.len()
            );
        }
    }

    // Cluster on the failure that survives split resolution, so a file is
    // filed under its real problem rather than under a conditional split that
    // happened to come first in the file.
    let mut by_sig: BTreeMap<String, Vec<Failure>> = BTreeMap::new();
    for f in &failures {
        let effective = match split_resolved.get(&f.path) {
            Some(Some(reclustered)) => reclustered.clone(),
            Some(None) => f.clone(), // fully explained; stays for the config bucket
            None => f.clone(),
        };
        by_sig
            .entry(effective.signature.clone())
            .or_default()
            .push(effective);
    }
    let mut clusters: Vec<Cluster> = by_sig
        .into_iter()
        .map(|(signature, fs)| {
            let (config_paths, rest): (Vec<String>, Vec<String>) = fs
                .iter()
                .filter(|f| validity.get(&f.path).copied().unwrap_or(false))
                .map(|f| f.path.clone())
                .partition(|p| config_inherent.contains(p));
            let declared = declared_versions.contains(&signature);
            let (version_paths, valid_paths): (Vec<String>, Vec<String>) = rest
                .into_iter()
                .partition(|p| declared && version_only.contains(p));
            let verdict = if !valid_paths.is_empty() {
                "gap"
            } else if !version_paths.is_empty() {
                "version"
            } else if !config_paths.is_empty() {
                "config"
            } else {
                "noise"
            };
            let macro_paths: Vec<String> = valid_paths
                .iter()
                .filter(|p| macro_clean.contains(*p))
                .cloned()
                .collect();
            let mut tally: BTreeMap<&str, usize> = BTreeMap::new();
            for p in &macro_paths {
                for name in macro_names.get(p).into_iter().flatten() {
                    *tally.entry(name.as_str()).or_default() += 1;
                }
            }
            let mut ranked: Vec<(&str, usize)> = tally.into_iter().collect();
            ranked.sort_by(|a, b| b.1.cmp(&a.1).then(a.0.cmp(b.0)));
            let macros: Vec<String> = ranked
                .into_iter()
                .take(8)
                .map(|(name, n)| format!("{name} ({n})"))
                .collect();
            Cluster {
                signature,
                verdict: verdict.into(),
                count: fs.len(),
                valid: valid_paths.len(),
                examples: fs.iter().take(5).cloned().collect(),
                valid_paths,
                config_paths,
                macro_paths,
                macros,
                version_paths,
                paths: fs.iter().map(|f| f.path.clone()).collect(),
            }
        })
        .collect();
    clusters.sort_by(|a, b| (b.valid, b.count).cmp(&(a.valid, a.count)));

    let gap_files: usize = clusters.iter().map(|c| c.valid).sum();
    let config_files: usize = clusters.iter().map(|c| c.config_paths.len()).sum();
    let version_files: usize = clusters.iter().map(|c| c.version_paths.len()).sum();
    let noise_files = failures.len() - gap_files - config_files - version_files;

    // Measure the oracle's documented blind spot rather than assuming it is
    // small. Among the files booked as noise, ask the PARSER alone: the ones
    // it accepts are syntactically valid, so our rejection of them is a
    // grammar gap that `compile`'s extra strictness hid.
    let noise_paths: Vec<String> = failures
        .iter()
        .map(|f| f.path.clone())
        .filter(|p| !validity.get(p).copied().unwrap_or(false))
        .collect();
    let mut hidden_gaps: Vec<String> = Vec::new();
    if !noise_paths.is_empty() {
        if let Some(syntax) =
            treebank_oracle::get(lang).validate_syntax_only(&corpus_src, &noise_paths)?
        {
            for p in &noise_paths {
                if syntax.get(p).copied() == Some(true) {
                    hidden_gaps.push(p.clone());
                }
            }
        }
    }
    hidden_gaps.sort();

    // A declared rejection that matches nothing is stale: either the corpus
    // no longer contains it or the signature drifted when the grammar
    // changed. Loud, because a policy file that quietly stops describing
    // reality is worse than no policy file.
    let matched: std::collections::HashSet<&str> = clusters
        .iter()
        .filter(|c| !c.version_paths.is_empty())
        .map(|c| c.signature.as_str())
        .collect();
    for sig in &declared_versions {
        if !matched.contains(sig.as_str()) {
            eprintln!(
                "sweep: WARNING version_policy.toml declares `{sig}` rejected, but no \
                 failing file matches it. Stale entry, or the signature drifted."
            );
        }
    }
    let report = Report {
        lang: lang.to_string(),
        grammar: grammar_dir.display().to_string(),
        files: files.len(),
        passed: files.len() - failures.len(),
        failed: failures.len(),
        gap_files,
        config_files,
        version_files,
        noise_files,
        hidden_gap_files: hidden_gaps.len(),
        hidden_gaps,
        clusters,
    };
    std::fs::create_dir_all(out.parent().unwrap_or(Path::new(".")))?;
    std::fs::write(out, serde_json::to_string_pretty(&report)?)?;
    write_ledger_block(grammar_dir, lang, &report)?;
    let report_md = out.with_file_name("REPORT.md");
    std::fs::write(&report_md, markdown(&report, corpus_root))?;

    println!(
        "sweep: {} files — {} passed, {} failed ({} grammar-gap, {} config-inherent, \
         {} version-policy, {} noise), {} clusters",
        report.files,
        report.passed,
        report.failed,
        report.gap_files,
        report.config_files,
        report.version_files,
        report.noise_files,
        report.clusters.len()
    );
    if report.hidden_gap_files > 0 {
        println!(
            "sweep: {} of the noise files are SYNTACTICALLY valid — the oracle judges with \
             compile(), which also runs CPython's post-parse checks, so these are gaps it \
             cannot see. First few: {:?}",
            report.hidden_gap_files,
            &report.hidden_gaps[..report.hidden_gaps.len().min(3)],
        );
    }
    for c in report.clusters.iter().take(10) {
        println!("  {:>5} {:>6}  {}", c.verdict, c.count, c.signature);
        if let Some(e) = c.examples.first() {
            println!("               e.g. {}:{}  {}", e.path, e.line, e.snippet);
        }
    }
    println!("sweep: agent-ready report at {}", report_md.display());
    Ok(())
}

/// The hand-to-an-agent report: what's broken, where, and how to verify a fix.
fn markdown(report: &Report, corpus_root: &Path) -> String {
    use std::fmt::Write;
    let mut md = String::new();
    let _ = write!(
        md,
        "# Grammar sweep report — {}\n\n\
         - Grammar: `{}`\n\
         - Corpus: `{}` ({} files)\n\
         - Result: **{} passed, {} failed** — {} files are valid {} the \
         grammar rejects (grammar gaps), {} are invalid (corpus noise, \
         ignore){}\n\n",
        report.lang,
        report.grammar,
        corpus_root.display(),
        report.files,
        report.passed,
        report.failed,
        report.gap_files,
        report.lang,
        report.noise_files,
        if report.config_files > 0 || report.version_files > 0 {
            let mut extra = String::new();
            if report.config_files > 0 {
                extra.push_str(&format!(
                    ", and {} are valid but **cannot be represented as written** \
                     because a preprocessor conditional splits a construct — see \
                     the note at the end; do not try to fix those",
                    report.config_files
                ));
            }
            if report.version_files > 0 {
                extra.push_str(&format!(
                    ", and {} are valid only in an OLDER version of the language \
                     and are rejected **on purpose** — see `version_policy.toml`; \
                     those are decisions, not bugs, do not try to fix them",
                    report.version_files
                ));
            }
            extra
        } else {
            String::new()
        },
    );
    let mut config: Vec<&Cluster> = report
        .clusters
        .iter()
        .filter(|c| c.verdict == "config")
        .collect();
    // Report order is by gap size; these have none, so order them by the
    // count this section actually prints.
    config.sort_by_key(|c| std::cmp::Reverse(c.config_paths.len()));
    let config_note = |md: &mut String| {
        if config.is_empty() {
            return;
        }
        let _ = write!(
            md,
            "\n## Not grammar bugs: {} file(s) the preprocessor splits\n\n\
             These are valid {}, and the grammar rejects them, but **no grammar \
             change can fix them and you should not try**. Each one parses \
             cleanly once the branches a compiler would have dropped are \
             removed, so the rejection is a property of conditional \
             compilation, not of the parser.\n\n\
             The canonical case is a C header opening `extern \"C\" {{` inside \
             one `#ifdef __cplusplus` and closing `}}` inside another: the \
             braces cross conditional boundaries, so no single tree can \
             represent both configurations. Making the grammar accept it would \
             mean accepting unbalanced braces generally, which is how a parser \
             starts accepting broken code.\n\n\
             Clusters, largest first:\n\n",
            report.config_files, report.lang,
        );
        for c in &config {
            let _ = write!(
                md,
                "- `{}` — {} file(s)\n",
                c.signature,
                c.config_paths.len()
            );
        }
    };

    let mut versions: Vec<&Cluster> = report
        .clusters
        .iter()
        .filter(|c| c.verdict == "version")
        .collect();
    versions.sort_by_key(|c| std::cmp::Reverse(c.version_paths.len()));
    let version_note = |md: &mut String| {
        if versions.is_empty() {
            return;
        }
        let _ = write!(
            md,
            "\n## Not grammar bugs: {} file(s) rejected by version policy\n\n\
             These are valid in an OLDER version of {} and the grammar rejects \
             them **on purpose**. Do not try to fix them.\n\n\
             Where a construct is valid only in an older version AND admitting \
             it would change how CURRENT code parses, the current language wins \
             (DESIGN.md §4.2). In a GLR grammar an admitted old form is not a \
             quiet extra reading — it is a fork at every occurrence of the \
             token, and forks can win. Each construct below is declared in \
             `version_policy.toml` with its reasoning, and has a file in \
             `test/negative/` so the rejection is a gate rather than a note.\n\n\
             Both conditions are required to land here: the signature is \
             declared, AND the CURRENT version's oracle also rejects the file. \
             A declaration alone cannot suppress a failure on code that is \
             still valid today.\n\n\
             Clusters, largest first:\n\n",
            report.version_files, report.lang,
        );
        for c in &versions {
            let _ = write!(
                md,
                "- `{}` — {} file(s)\n",
                c.signature,
                c.version_paths.len()
            );
        }
    };

    let gaps: Vec<&Cluster> = report
        .clusters
        .iter()
        .filter(|c| c.verdict == "gap")
        .collect();
    if gaps.is_empty() {
        md.push_str("No grammar gaps — nothing to fix.\n");
        config_note(&mut md);
        version_note(&mut md);
        return md;
    }
    md.push_str("## Grammar gaps, largest first\n");
    for (i, c) in gaps.iter().enumerate() {
        let _ = write!(
            md,
            "\n### {}. `{}` — {} valid file(s)\n{}\nExamples:\n",
            i + 1,
            c.signature,
            c.valid,
            if c.macro_paths.is_empty() {
                String::new()
            } else {
                format!(
                    "\n**{} of these parse cleanly once macros are expanded**, so the \
                     grammar is meeting an unexpanded macro rather than unfamiliar \
                     syntax. Most common at the error site: {}. Expand one to see the \
                     shape a rule has to accept — and use the others to check it does \
                     not accept more than that.\n",
                    c.macro_paths.len(),
                    c.macros.join(", ")
                )
            }
        );
        for e in c
            .examples
            .iter()
            .filter(|e| c.valid_paths.contains(&e.path))
            .take(3)
        {
            let _ = write!(md, "- `{}:{}`  `{}`\n", e.path, e.line, e.snippet);
        }
        md.push_str("\nFiles that must pass after the fix:\n");
        for p in &c.valid_paths {
            let _ = write!(md, "- `{}/src/{}`\n", corpus_root.display(), p);
        }
    }
    let _ = write!(
        md,
        "\n## How to fix (one cluster at a time)\n\n\
         Work in `{grammar}`. For each cluster:\n\n\
         1. Reproduce: write the smallest failing repro and confirm it \
         is valid {lang} with the oracle.\n\
         2. Fix the grammar source (smallest change, mirror existing idioms; \
         scanner only for external-token issues). Add a corpus test in \
         `test/corpus/`.\n\
         3. Regenerate with the pinned tree-sitter CLI, run the corpus \
         tests, re-run this sweep (pass count must beat {passed}), and \
         re-run the negative corpus.\n\
         4. Record the change and its before/after sweep numbers in the \
         grammar's ledger — see `DESIGN.md` at the repo root.\n",
        grammar = report.grammar,
        lang = report.lang,
        passed = report.passed,
    );
    config_note(&mut md);
    version_note(&mut md);
    md
}

/// Negative corpus: every file must FAIL to parse.
pub fn negative(grammar_dir: &Path, dir: &Path) -> Result<()> {
    negative_inner(grammar_dir, dir, false)
}

/// `quiet` suppresses the success line so `verify` can format its own.
pub fn negative_quiet(grammar_dir: &Path, dir: &Path) -> Result<()> {
    negative_inner(grammar_dir, dir, true)
}

fn negative_inner(grammar_dir: &Path, dir: &Path, quiet: bool) -> Result<()> {
    let (language, _) = grammar::load(grammar_dir)?;
    let mut parser = Parser::new();
    parser.set_language(&language)?;
    let mut wrongly_accepted = Vec::new();
    let mut total = 0;
    for entry in std::fs::read_dir(dir)? {
        let path = entry?.path();
        if !path.is_file()
            || path
                .file_name()
                .is_some_and(|n| n.to_string_lossy().starts_with('.'))
        {
            continue;
        }
        total += 1;
        let src = std::fs::read(&path)?;
        let tree = parser.parse(&src, None);
        let ok = tree.map(|t| !t.root_node().has_error()).unwrap_or(false);
        if ok {
            wrongly_accepted.push(path);
        }
    }
    if !wrongly_accepted.is_empty() {
        for p in &wrongly_accepted {
            eprintln!("negative: ACCEPTED (should reject): {}", p.display());
        }
        bail!(
            "{} of {} negative files were accepted",
            wrongly_accepted.len(),
            total
        );
    }
    if !quiet {
        println!("negative: all {total} files correctly rejected");
    }
    Ok(())
}
