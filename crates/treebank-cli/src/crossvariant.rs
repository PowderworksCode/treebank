//! `treebank crossvariant` — the gate that exists only because variants do
//! (VARIANTS.md §5).
//!
//! Every other gate asks a question about ONE grammar, and none of them can
//! see the failure mode a multi-variant language has: the variants drifting
//! back into one permissive union. Each table gets a little more accepting,
//! every individual sweep stays green — a corpus of real python 3 never
//! contains `print "x"`, so nothing in it ever rejects — and what you end up
//! with is two copies of one lenient grammar and a `--variant` flag that
//! selects between them without selecting anything.
//!
//! So this asserts the difference directly. `test/crossvariant/<a>-not-<b>/`
//! holds files that are valid in variant `a` and must be REJECTED by variant
//! `b`, and both halves are checked: a file that `a` also rejects is a
//! broken fixture, not a passing test, which is what stops the corpus itself
//! from rotting into "files nothing accepts".

use std::path::Path;

use anyhow::{bail, Context, Result};
use tree_sitter::Parser;

use crate::grammar;

pub fn run(crate_dir: &Path) -> Result<()> {
    let (checked, pairs) = run_inner(crate_dir, false)?;
    println!("crossvariant: {checked} files across {pairs} variant pairs");
    Ok(())
}

pub fn run_quiet(crate_dir: &Path) -> Result<String> {
    let (checked, pairs) = run_inner(crate_dir, true)?;
    Ok(format!("{checked} files, {pairs} pairs"))
}

fn run_inner(crate_dir: &Path, quiet: bool) -> Result<(usize, usize)> {
    let root = crate_dir.join("test/crossvariant");
    if !root.is_dir() {
        // A single-variant language has no pairs to check, and that is a
        // pass rather than a skip: there is nothing that could collapse.
        return Ok((0, 0));
    }

    let mut dirs: Vec<_> = std::fs::read_dir(&root)?
        .filter_map(|e| e.ok())
        .map(|e| e.path())
        .filter(|p| p.is_dir())
        .collect();
    dirs.sort();

    let mut checked = 0usize;
    let mut failures = Vec::new();

    for dir in &dirs {
        let name = dir
            .file_name()
            .and_then(|n| n.to_str())
            .context("crossvariant dir name")?;
        let (accepts, rejects) = name.split_once("-not-").with_context(|| {
            format!("crossvariant dir must be named <a>-not-<b>, got `{name}`")
        })?;

        let mut parser_a = variant_parser(crate_dir, accepts)?;
        let mut parser_b = variant_parser(crate_dir, rejects)?;

        let mut files: Vec<_> = std::fs::read_dir(dir)?
            .filter_map(|e| e.ok())
            .map(|e| e.path())
            .filter(|p| {
                p.is_file()
                    && !p
                        .file_name()
                        .is_some_and(|n| n.to_string_lossy().starts_with('.'))
            })
            .collect();
        files.sort();

        for path in &files {
            checked += 1;
            let src = std::fs::read(path)?;
            let rel = path.strip_prefix(crate_dir).unwrap_or(path).display();

            // The claim being made, in both directions.
            if !parses_clean(&mut parser_a, &src) {
                failures.push(format!(
                    "{rel}: {accepts} REJECTS it — the fixture claims it is valid {accepts}"
                ));
            }
            if parses_clean(&mut parser_b, &src) {
                failures.push(format!(
                    "{rel}: {rejects} ACCEPTS it — the variants have converged here"
                ));
            }
        }

        if !quiet {
            println!("  {accepts} not {rejects}: {} files", files.len());
        }
    }

    if !failures.is_empty() {
        for f in &failures {
            eprintln!("crossvariant: {f}");
        }
        bail!("{} crossvariant assertion(s) failed", failures.len());
    }

    Ok((checked, dirs.len()))
}

fn variant_parser(crate_dir: &Path, variant: &str) -> Result<Parser> {
    let dir = crate_dir.join(variant);
    if !dir.join("src/parser.c").exists() {
        bail!(
            "no generated grammar for variant `{variant}` at {}",
            dir.display()
        );
    }
    let (language, _) = grammar::load(&dir)?;
    let mut parser = Parser::new();
    parser.set_language(&language)?;
    Ok(parser)
}

fn parses_clean(parser: &mut Parser, src: &[u8]) -> bool {
    parser
        .parse(src, None)
        .map(|t| !t.root_node().has_error())
        .unwrap_or(false)
}
