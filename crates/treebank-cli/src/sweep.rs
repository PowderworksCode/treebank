use std::collections::BTreeMap;
use std::path::Path;

use anyhow::{bail, Result};
use rayon::prelude::*;
use serde::{Deserialize, Serialize};
use tree_sitter::{Node, Parser};

use crate::fetch::Manifest;
use crate::grammar;

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
    /// "gap" (valid code the grammar rejects — fix the grammar) or "noise"
    /// (the reference parser rejects these files too — ignore).
    pub verdict: String,
    pub count: usize,
    pub valid: usize,
    pub examples: Vec<Failure>,
    /// Failing files the reference parser says are VALID — the fix targets.
    pub valid_paths: Vec<String>,
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
    pub noise_files: usize,
    pub clusters: Vec<Cluster>,
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

fn signature_of(node: Node, src: &[u8]) -> String {
    let parent = node.parent().map(|p| p.kind().to_string()).unwrap_or_else(|| "<root>".into());
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

fn snippet_at(src: &[u8], byte: usize) -> (usize, String) {
    let text = String::from_utf8_lossy(src);
    let upto = &text[..text.len().min(byte.min(text.len()))];
    let line_no = upto.matches('\n').count() + 1;
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
    /// Combined fingerprint of the compiled grammars this cache is valid for.
    grammar: String,
    /// sha256 of every file that PASSED under that grammar. Failing files
    /// are always re-parsed (their diagnostics must stay fresh).
    passed_sha256: Vec<String>,
}

pub fn run(lang: &dyn crate::lang::Lang, grammar_dir: &Path, manifest_path: &Path, out: &Path) -> Result<()> {
    let loaded: Vec<(tree_sitter::Language, String)> = lang
        .grammar_dirs()
        .iter()
        .map(|d| grammar::load(&grammar_dir.join(d)))
        .collect::<Result<_>>()?;
    let fingerprint = loaded.iter().map(|(_, f)| f.as_str()).collect::<Vec<_>>().join("+");
    let languages: Vec<tree_sitter::Language> = loaded.into_iter().map(|(l, _)| l).collect();
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
    let (skipped, work): (Vec<_>, Vec<_>) =
        files.iter().partition(|f| known_pass.contains(f.sha256.as_str()));
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
                languages
                    .iter()
                    .map(|l| {
                        let mut p = Parser::new();
                        p.set_language(l).expect("language/runtime ABI mismatch");
                        p
                    })
                    .collect::<Vec<_>>()
            },
            |parsers, f| {
                let full = corpus_src.join(&f.pkgdir).join(&f.rel);
                let Ok(src) = std::fs::read(&full) else {
                    return None;
                };
                let package = f.pkgdir.rsplitn(2, '-').last().unwrap_or(&f.pkgdir).to_string();
                let parser = &mut parsers[lang.route(&f.dialect, &f.rel)];
                let failure =
                    check_file(parser, &package, &format!("{}/{}", f.pkgdir, f.rel), &src);
                Some((f.sha256.clone(), failure))
            },
        )
        .flatten()
        .collect();
    let failures: Vec<Failure> =
        results.iter().filter_map(|(_, f)| f.clone()).collect();

    // Persist the cache: previously-known passes (still present in the
    // corpus) plus this run's fresh passes.
    let mut passed_sha256: Vec<String> =
        skipped.iter().map(|f| f.sha256.clone()).collect();
    passed_sha256.extend(
        results.iter().filter(|(_, f)| f.is_none()).map(|(sha, _)| sha.clone()),
    );
    passed_sha256.sort();
    passed_sha256.dedup();
    std::fs::write(
        &cache_path,
        serde_json::to_string(&SweepCache { grammar: fingerprint, passed_sha256 })?,
    )?;

    // Adjudicate every failing file with the language's reference parser so
    // the report separates grammar gaps from corpus noise.
    let failing_paths: Vec<String> = failures.iter().map(|f| f.path.clone()).collect();
    let validity = lang.validate(&corpus_src, &failing_paths)?;

    let mut by_sig: BTreeMap<String, Vec<Failure>> = BTreeMap::new();
    for f in &failures {
        by_sig.entry(f.signature.clone()).or_default().push(f.clone());
    }
    let mut clusters: Vec<Cluster> = by_sig
        .into_iter()
        .map(|(signature, fs)| {
            let valid_paths: Vec<String> = fs
                .iter()
                .filter(|f| validity.get(&f.path).copied().unwrap_or(false))
                .map(|f| f.path.clone())
                .collect();
            Cluster {
                signature,
                verdict: if valid_paths.is_empty() { "noise" } else { "gap" }.into(),
                count: fs.len(),
                valid: valid_paths.len(),
                examples: fs.iter().take(5).cloned().collect(),
                valid_paths,
                paths: fs.iter().map(|f| f.path.clone()).collect(),
            }
        })
        .collect();
    clusters.sort_by(|a, b| (b.valid, b.count).cmp(&(a.valid, a.count)));

    let gap_files = clusters.iter().map(|c| c.valid).sum();
    let noise_files = failures.len() - gap_files;
    let report = Report {
        lang: lang.name().to_string(),
        grammar: grammar_dir.display().to_string(),
        files: files.len(),
        passed: files.len() - failures.len(),
        failed: failures.len(),
        gap_files,
        noise_files,
        clusters,
    };
    std::fs::create_dir_all(out.parent().unwrap_or(Path::new(".")))?;
    std::fs::write(out, serde_json::to_string_pretty(&report)?)?;
    let report_md = out.with_file_name("REPORT.md");
    std::fs::write(&report_md, markdown(&report, corpus_root))?;

    println!(
        "sweep: {} files — {} passed, {} failed ({} grammar-gap, {} noise), {} clusters",
        report.files,
        report.passed,
        report.failed,
        report.gap_files,
        report.noise_files,
        report.clusters.len()
    );
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
         grammar rejects (grammar gaps), {} are invalid (corpus noise, ignore)\n\n",
        report.lang,
        report.grammar,
        corpus_root.display(),
        report.files,
        report.passed,
        report.failed,
        report.gap_files,
        report.lang,
        report.noise_files,
    );
    let gaps: Vec<&Cluster> = report.clusters.iter().filter(|c| c.verdict == "gap").collect();
    if gaps.is_empty() {
        md.push_str("No grammar gaps — nothing to fix.\n");
        return md;
    }
    md.push_str("## Grammar gaps, largest first\n");
    for (i, c) in gaps.iter().enumerate() {
        let _ = write!(
            md,
            "\n### {}. `{}` — {} valid file(s)\n\nExamples:\n",
            i + 1,
            c.signature,
            c.valid
        );
        for e in c.examples.iter().filter(|e| c.valid_paths.contains(&e.path)).take(3) {
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
         1. Reproduce: `../../scripts/parse.sh <failing file>` (run from the \
         grammar dir), then write the smallest failing repro and confirm it \
         is valid {lang}.\n\
         2. Fix the grammar source (smallest change, mirror existing idioms; \
         scanner only for external-token issues). Add a corpus test in \
         `test/corpus/`.\n\
         3. Run `../../scripts/check.sh` until it prints `CHECK OK` — it \
         regenerates with the pinned CLI and runs corpus tests, this sweep \
         (pass count must beat {passed}), and the negative corpus.\n\
         4. Capture the change as `patches/NNNN-*.patch` (source-of-truth \
         files only, never generated files) with a ledger entry — see \
         `GRAMMARS.md` at the repo root.\n",
        grammar = report.grammar,
        lang = report.lang,
        passed = report.passed,
    );
    md
}

/// Negative corpus: every file must FAIL to parse.
pub fn negative(grammar_dir: &Path, dir: &Path) -> Result<()> {
    let (language, _) = grammar::load(grammar_dir)?;
    let mut parser = Parser::new();
    parser.set_language(&language)?;
    let mut wrongly_accepted = Vec::new();
    let mut total = 0;
    for entry in std::fs::read_dir(dir)? {
        let path = entry?.path();
        if !path.is_file() || path.file_name().is_some_and(|n| n.to_string_lossy().starts_with('.')) {
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
        bail!("{} of {} negative files were accepted", wrongly_accepted.len(), total);
    }
    println!("negative: all {total} files correctly rejected");
    Ok(())
}
