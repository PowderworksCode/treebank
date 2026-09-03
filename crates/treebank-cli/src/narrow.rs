//! `treebank narrow` — the rung-1 narrowing gate and scanner
//! (notes/DESIGN.md §4.2, "Narrowing a shared table").
//!
//! A row that shares its family's parse table accepts less than that table
//! does, and `narrowing.json` is where it says which constructs it does not
//! claim. This module does the two things that turn the manifest from a
//! comment into a gate:
//!
//! - **check** — every pattern compiles against the shared grammar, and
//!   every pattern matches the fixture it names. The first refuses a
//!   pattern the table can never produce; the second refuses a narrowing
//!   nobody can trip, which is the `roles` liveness rule (§3.3 rule 5)
//!   applied to the same failure mode.
//! - **scan** — parse a file with the shared grammar and report every
//!   out-of-row occurrence in it, which is what a narrowed parse is: the
//!   tree comes back carrying its out-of-row occurrences, or the call
//!   refuses the file.
//!
//! What the scan deliberately does NOT do is decide which row a file
//! belongs to. An occurrence outside a row is a fact about that
//! occurrence; a py2-only construct nested inside a py3-only one is valid
//! in neither version, and the measured cost of forgetting that is in
//! `narrowing.rs`. The file's row is the oracle's question (§4.3).

use std::path::{Path, PathBuf};

use anyhow::{bail, Context, Result};
use treebank::narrowing::{Entry, NarrowingManifest};

/// One out-of-row occurrence found in a file.
pub struct Occurrence {
    pub construct: String,
    pub valid_in: String,
    pub line: usize,
    pub column: usize,
    pub text: String,
}

impl std::fmt::Display for Occurrence {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{}:{}: {} ({}) — {}",
            self.line,
            self.column,
            self.construct,
            self.valid_in,
            self.text.replace('\n', "\\n"),
        )
    }
}

fn manifest_path(grammar_dir: &Path) -> PathBuf {
    grammar_dir.join("narrowing.json")
}

/// Load the manifest for a family crate, if it ships one. A crate without
/// `narrowing.json` has no rung-1 rows and is not a failure.
pub fn load(grammar_dir: &Path) -> Result<Option<NarrowingManifest>> {
    let path = manifest_path(grammar_dir);
    if !path.exists() {
        return Ok(None);
    }
    Ok(Some(NarrowingManifest::load(&path)?))
}

/// Compile one entry's pattern against the shared grammar. A pattern that
/// names a node the grammar cannot produce fails here rather than matching
/// nothing forever.
fn compile(language: &tree_sitter::Language, entry: &Entry) -> Result<tree_sitter::Query> {
    tree_sitter::Query::new(language, &entry.pattern)
        .with_context(|| format!("`{}`: pattern does not compile", entry.construct))
}

/// Every occurrence of `query` in `source`.
fn matches(
    query: &tree_sitter::Query,
    tree: &tree_sitter::Tree,
    source: &str,
    entry: &Entry,
) -> Vec<Occurrence> {
    use tree_sitter::StreamingIterator;
    let mut cursor = tree_sitter::QueryCursor::new();
    let mut found = Vec::new();
    let mut it = cursor.matches(query, tree.root_node(), source.as_bytes());
    while let Some(m) = it.next() {
        for c in m.captures {
            let start = c.node.start_position();
            found.push(Occurrence {
                construct: entry.construct.clone(),
                valid_in: entry.valid_in.clone(),
                line: start.row + 1,
                column: start.column + 1,
                text: c.node.utf8_text(source.as_bytes()).unwrap_or("").to_string(),
            });
        }
    }
    found
}

/// Check a family crate's manifest: patterns compile, and each one matches
/// the fixture it names. Returns a one-line summary for `verify`.
pub fn check(grammar_dir: &Path) -> Result<String> {
    let Some(manifest) = load(grammar_dir)? else {
        return Ok("no narrowing.json; this crate has no rung-1 rows".to_string());
    };

    let vocab = treebank::vocabulary();
    if manifest.vocabulary != vocab.version {
        bail!(
            "narrowing.json targets vocabulary {} but treebank carries {}",
            manifest.vocabulary,
            vocab.version
        );
    }

    let (language, _) = crate::grammar::load(grammar_dir)?;
    let mut parser = tree_sitter::Parser::new();
    parser
        .set_language(&language)
        .context("setting the shared grammar")?;

    let mut findings: Vec<String> = Vec::new();
    let mut entries = 0usize;

    for (row, spec) in &manifest.rows {
        if spec.out_of_row.is_empty() {
            findings.push(format!(
                "{row}: no out-of-row entries, so this row narrows nothing"
            ));
        }
        for entry in &spec.out_of_row {
            entries += 1;

            // Rule 1: the pattern compiles against the shared grammar.
            let query = match compile(&language, entry) {
                Ok(q) => q,
                Err(e) => {
                    findings.push(format!("{row}: {e:#}"));
                    continue;
                }
            };

            // Rule 2 (liveness): the pattern matches the fixture it names.
            let fixture = grammar_dir.join(&entry.fixture);
            let Ok(source) = std::fs::read_to_string(&fixture) else {
                findings.push(format!(
                    "{row}: `{}` names fixture {}, which does not exist",
                    entry.construct, entry.fixture
                ));
                continue;
            };
            let Some(tree) = parser.parse(&source, None) else {
                findings.push(format!("{row}: could not parse {}", entry.fixture));
                continue;
            };
            let hits = matches(&query, &tree, &source, entry);
            if hits.is_empty() {
                findings.push(format!(
                    "{row}: `{}` matches nothing in its own fixture {} — a narrowing nobody can trip",
                    entry.construct, entry.fixture
                ));
            }
        }
    }

    for f in &findings {
        eprintln!("narrowing: {f}");
    }
    if !findings.is_empty() {
        bail!("{} narrowing finding(s)", findings.len());
    }

    let rows: Vec<&str> = manifest.rows.keys().map(String::as_str).collect();
    let residue: usize = manifest.rows.values().map(|r| r.residue.len()).sum();
    Ok(format!(
        "{} row(s) over {} [{}], {entries} entr(ies), {residue} declared residue",
        manifest.rows.len(),
        manifest.grammar,
        rows.join(", "),
    ))
}

/// Scan files for one row's out-of-row occurrences.
pub fn scan(grammar_dir: &Path, row: &str, files: &[PathBuf]) -> Result<Vec<(PathBuf, Occurrence)>> {
    let manifest =
        load(grammar_dir)?.with_context(|| format!("{} ships no narrowing.json", grammar_dir.display()))?;
    let spec = manifest.rows.get(row).with_context(|| {
        let known: Vec<&str> = manifest.rows.keys().map(String::as_str).collect();
        format!("no row named {row}; this crate has {}", known.join(", "))
    })?;

    let (language, _) = crate::grammar::load(grammar_dir)?;
    let mut parser = tree_sitter::Parser::new();
    parser.set_language(&language)?;

    let compiled: Vec<(&Entry, tree_sitter::Query)> = spec
        .out_of_row
        .iter()
        .map(|e| compile(&language, e).map(|q| (e, q)))
        .collect::<Result<_>>()?;

    let mut out = Vec::new();
    for path in files {
        let source = std::fs::read_to_string(path)
            .with_context(|| format!("read {}", path.display()))?;
        let tree = parser
            .parse(&source, None)
            .with_context(|| format!("parse {}", path.display()))?;
        for (entry, query) in &compiled {
            for occ in matches(query, &tree, &source, entry) {
                out.push((path.clone(), occ));
            }
        }
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use std::path::{Path, PathBuf};

    fn python_dir() -> PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../treebank-python")
    }

    /// The gate itself: python's manifest compiles and every entry is live.
    #[test]
    fn pythons_manifest_passes_its_own_gate() {
        let summary = super::check(&python_dir()).expect("narrowing check");
        assert!(summary.contains("python3"), "{summary}");
        assert!(summary.contains("python2"), "{summary}");
    }

    /// The point of the whole exercise: a python3 consumer can be handed a
    /// parser that reports `print x` as out of its row, which the union
    /// table alone can never do.
    #[test]
    fn the_python3_row_rejects_a_py2_print() {
        let dir = python_dir();
        let file = dir.join("test/narrowing/python3/print-statement.py");
        let found = super::scan(&dir, "python3", &[file]).expect("scan");
        assert_eq!(found.len(), 1, "{found:?}", found = found.len());
        assert!(found[0].1.construct.contains("print"));
    }

    /// And the other direction is quiet: nothing in ordinary python3 source
    /// is out of the python3 row. A narrowing that fired here would be worse
    /// than no narrowing at all.
    #[test]
    fn ordinary_python3_source_is_in_row() {
        let dir = python_dir();
        // The py2 row's own fixtures are modern python3 by construction.
        let files: Vec<_> = ["fstring.py", "walrus.py", "nonlocal.py"]
            .iter()
            .map(|f| dir.join("test/narrowing/python2").join(f))
            .collect();
        let found = super::scan(&dir, "python3", &files).expect("scan");
        assert!(found.is_empty(), "unexpected out-of-row: {:?}", found.len());
    }

    /// An occurrence is not a verdict about the file. The py2-only
    /// constructs the python3 row narrows are exactly the ones the python2
    /// row does not, and vice versa — checked here so the two manifests
    /// cannot quietly start disagreeing about what a py2 construct is.
    #[test]
    fn the_two_rows_narrow_opposite_directions() {
        let dir = python_dir();
        let py2_only = dir.join("test/narrowing/python3/backticks.py");
        assert_eq!(
            super::scan(&dir, "python3", &[py2_only.clone()])
                .unwrap()
                .len(),
            1
        );
        assert_eq!(super::scan(&dir, "python2", &[py2_only]).unwrap().len(), 0);
    }
}
