//! `treebank verify` — every gate a grammar has to pass, in one command.
//!
//! The gates existed before this did; they were five separate invocations
//! that a contributor had to remember, and remembering is the failure mode
//! a checker is supposed to remove. CI ran them as separate steps, which is
//! right for CI (a failure names itself) and wrong for a working loop.
//!
//! Sweeps are deliberately NOT here: they need corpora measured in
//! gigabytes, and their numbers live in each grammar's ledger. What is here
//! is everything checkable from a clean checkout.

use std::path::Path;
use std::process::Command;

use anyhow::{bail, Result};

pub fn run(grammar_dir: &Path, crates_dir: &Path, rosetta_dir: &Path) -> Result<()> {
    let name = grammar_dir
        .file_name()
        .map(|s| s.to_string_lossy().to_string())
        .unwrap_or_default();
    let mut failed = Vec::new();

    // 1. Reproducible generation. Without this the parser that ships and the
    //    grammar you can read drift apart silently.
    match generation_is_reproducible(grammar_dir) {
        Ok(true) => println!("  generate      reproducible"),
        Ok(false) => {
            println!("  generate      DRIFTED — src/ is not what grammar.js produces");
            failed.push("generate");
        }
        Err(e) => {
            println!("  generate      could not check: {e}");
            failed.push("generate");
        }
    }

    // 2. The grammar's own corpus tests: tree SHAPE, not just accept/reject.
    match run_tool("tree-sitter", &["test"], grammar_dir) {
        Ok(true) => println!("  corpus tests  pass"),
        _ => {
            println!("  corpus tests  FAIL");
            failed.push("corpus tests");
        }
    }

    // 3. The negative corpus. Sweeps catch rejects-valid-code; this catches
    //    accepts-invalid-code, which no corpus of real source can reveal.
    let neg = grammar_dir.join("test/negative");
    if neg.is_dir() {
        match crate::sweep::negative_quiet(grammar_dir, &neg) {
            Ok(()) => println!("  negative      all rejected"),
            Err(e) => {
                println!("  negative      FAIL: {e}");
                failed.push("negative");
            }
        }
    }

    // 4. Vocabulary conformance.
    match crate::roles_check(grammar_dir) {
        Ok(summary) => println!("  roles         {summary}"),
        Err(e) => {
            println!("  roles         FAIL: {e}");
            failed.push("roles");
        }
    }

    // 5. The cross-language gate. A role threaded in one grammar and
    //    forgotten in another is silent everywhere else, because supertype
    //    matching is derivation-based.
    match crate::rosetta::run_quiet(rosetta_dir, crates_dir) {
        Ok(()) => println!("  rosetta       pass"),
        Err(e) => {
            println!("  rosetta       FAIL: {e}");
            failed.push("rosetta");
        }
    }

    if !failed.is_empty() {
        bail!("{name}: {} gate(s) failed: {}", failed.len(), failed.join(", "));
    }
    println!("verify OK: {name}");
    Ok(())
}

/// Regenerate and compare against what is committed. Uses git rather than a
/// hash of our own, so the answer is the same one CI's `git diff` gives.
fn generation_is_reproducible(grammar_dir: &Path) -> Result<bool> {
    if !run_tool("tree-sitter", &["generate"], grammar_dir)? {
        bail!("tree-sitter generate failed");
    }
    let out = Command::new("git")
        .args(["diff", "--quiet", "--", "src/"])
        .current_dir(grammar_dir)
        .status()?;
    Ok(out.success())
}

fn run_tool(bin: &str, args: &[&str], dir: &Path) -> Result<bool> {
    Ok(Command::new(bin)
        .args(args)
        .current_dir(dir)
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false))
}
