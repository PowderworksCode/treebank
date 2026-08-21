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

use std::path::{Path, PathBuf};
use std::process::Command;

use anyhow::{bail, Context, Result};
use treebank_lang::LangName;

/// The variant directories of a language crate, in declaration order, or
/// `["."]` for a single-variant language whose grammar sits at the crate
/// root. Read from tree-sitter.json's `path` fields, which is where
/// tree-sitter itself looks, so the gate and the generator cannot disagree
/// about what exists.
pub fn variant_dirs(crate_dir: &Path) -> Vec<PathBuf> {
    let Ok(text) = std::fs::read_to_string(crate_dir.join("tree-sitter.json")) else {
        return vec![crate_dir.to_path_buf()];
    };
    let Ok(json) = serde_json::from_str::<serde_json::Value>(&text) else {
        return vec![crate_dir.to_path_buf()];
    };
    let dirs: Vec<PathBuf> = json["grammars"]
        .as_array()
        .map(|gs| {
            gs.iter()
                .filter_map(|g| g["path"].as_str())
                .map(|p| crate_dir.join(p))
                .collect()
        })
        .unwrap_or_default();
    if dirs.is_empty() {
        vec![crate_dir.to_path_buf()]
    } else {
        dirs
    }
}

/// The variant a language's SHARED manifests describe, and the one a
/// cross-language check meets it as: the first that tree-sitter.json
/// declares. For python that is python3 — what `roles.json` covers, and
/// what `treebank_python::LANGUAGE` means. A single-variant language is
/// its own default, so callers need no special case.
pub fn default_variant(crate_dir: &Path) -> PathBuf {
    variant_dirs(crate_dir)
        .into_iter()
        .next()
        .unwrap_or_else(|| crate_dir.to_path_buf())
}

pub fn run(grammar_dir: &Path, crates_dir: &Path, rosetta_dir: &Path) -> Result<()> {
    let name = grammar_dir
        .file_name()
        .map(|s| s.to_string_lossy().to_string())
        .unwrap_or_default();
    let mut failed = Vec::new();

    // 0. Registration, once per LANGUAGE. Everything below asks whether
    //    the grammar is GOOD; this asks whether the rest of the
    //    repository can find it at all. A grammar whose directory name is
    //    not a language, or whose tree-sitter.json disagrees with the
    //    registry about which files it parses, builds and tests perfectly
    //    and is invisible to `--lang`.
    match registered(grammar_dir) {
        Ok(lang) => println!("  registered    {lang}"),
        Err(e) => {
            println!("  registered    FAIL: {e}");
            failed.push("registered");
        }
    }

    // Gates 1-3 are per PARSE TABLE, so a multi-variant language runs them
    // once per variant: each has its own generated src/, its own corpus and
    // its own negative corpus.
    let variants = variant_dirs(grammar_dir);
    let multi = variants.len() > 1;
    for dir in &variants {
        let label = if multi {
            format!(
                " [{}]",
                dir.file_name().map(|n| n.to_string_lossy()).unwrap_or_default()
            )
        } else {
            String::new()
        };

        // 1. Reproducible generation. Without this the parser that ships and
        //    the grammar you can read drift apart silently.
        match generation_is_reproducible(dir) {
            Ok(true) => println!("  generate      reproducible{label}"),
            Ok(false) => {
                println!("  generate      DRIFTED — src/ is not what grammar.js produces{label}");
                failed.push("generate");
            }
            Err(e) => {
                println!("  generate      could not check: {e}{label}");
                failed.push("generate");
            }
        }

        // 2. The grammar's own corpus tests: tree SHAPE, not accept/reject.
        match run_tool("tree-sitter", &["test"], dir) {
            Ok(true) => println!("  corpus tests  pass{label}"),
            _ => {
                println!("  corpus tests  FAIL{label}");
                failed.push("corpus tests");
            }
        }

        // 3. The negative corpus. Sweeps catch rejects-valid-code; this
        //    catches accepts-invalid-code, which no corpus of real source
        //    can reveal.
        let neg = dir.join("test/negative");
        if neg.is_dir() {
            match crate::sweep::negative_quiet(dir, &neg) {
                Ok(()) => println!("  negative      all rejected{label}"),
                Err(e) => {
                    println!("  negative      FAIL: {e}{label}");
                    failed.push("negative");
                }
            }
        }
    }

    // 3b. The variants must still be different languages. Nothing above can
    //     see them converging: each variant passes its own gates all the way
    //     into being one lenient grammar wearing two names.
    match crate::crossvariant::run_quiet(grammar_dir) {
        Ok(summary) if summary.starts_with('0') => {}
        Ok(summary) => println!("  crossvariant  {summary}"),
        Err(e) => {
            println!("  crossvariant  FAIL: {e}");
            failed.push("crossvariant");
        }
    }

    // 4. Vocabulary conformance. One manifest per language, so once.
    match crate::roles_check(grammar_dir) {
        Ok(summary) => println!("  roles         {summary}"),
        Err(e) => {
            println!("  roles         FAIL: {e}");
            failed.push("roles");
        }
    }

    // 4b. The FIELD_GUIDE.md smell detector, enforced where the grammar
    //     has written its lint_policy.toml ratchets, advisory otherwise.
    match crate::lint::run(grammar_dir) {
        Ok(()) => println!("  lint          ok"),
        Err(e) => {
            println!("  lint          FAIL: {e}");
            failed.push("lint");
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
        bail!(
            "{name}: {} gate(s) failed: {}",
            failed.len(),
            failed.join(", ")
        );
    }
    println!("verify OK: {name}");
    Ok(())
}

/// Is this grammar dir a language the registry knows, and does it claim
/// the files the registry says it parses?
///
/// The second half is the one that catches something: file extensions get
/// written down in `treebank-lang` (where the sweep and the fuzzer read
/// them) and again in the crate's `tree-sitter.json` (where every editor
/// and every `tree-sitter` invocation reads them), and nothing else
/// compares the two. The check is one-directional — a grammar may claim
/// extensions the registry does not list, the way bash claims `.ebuild` —
/// because the failure that matters is the registry sending files at a
/// grammar that does not admit to handling them.
fn registered(grammar_dir: &Path) -> Result<LangName> {
    let dir = grammar_dir
        .canonicalize()
        .unwrap_or_else(|_| grammar_dir.to_path_buf());
    let dirname = dir
        .file_name()
        .and_then(|s| s.to_str())
        .context("grammar dir has no name")?;
    let suffix = dirname
        .strip_prefix("treebank-")
        .with_context(|| format!("{dirname} is not named treebank-<language>"))?;
    let lang = LangName::from_name(suffix).with_context(|| {
        format!("no language called {suffix}: add it to the registry in crates/treebank-lang")
    })?;
    if lang.grammar() != lang {
        bail!(
            "{lang} is declared as a dialect of {}, so this directory should not exist",
            lang.grammar()
        );
    }

    let manifest = dir.join("tree-sitter.json");
    let text = std::fs::read_to_string(&manifest)
        .with_context(|| format!("read {}", manifest.display()))?;
    let json: serde_json::Value =
        serde_json::from_str(&text).with_context(|| format!("parse {}", manifest.display()))?;
    let grammars = json["grammars"]
        .as_array()
        .context("tree-sitter.json has no grammars")?;
    let claimed: Vec<&str> = grammars
        .iter()
        .filter_map(|g| g["file-types"].as_array())
        .flatten()
        .filter_map(|t| t.as_str())
        .collect();
    let missing: Vec<&str> = lang
        .grammar_extensions()
        .into_iter()
        .filter(|e| !claimed.contains(e))
        .collect();
    if !missing.is_empty() {
        bail!(
            "the registry routes .{} here but tree-sitter.json claims only {claimed:?}",
            missing.join(", .")
        );
    }
    Ok(lang)
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

#[cfg(test)]
mod tests {
    use super::{registered, Path};
    use treebank_lang::LangName;

    fn crates_dir() -> std::path::PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("..")
            .canonicalize()
            .unwrap()
    }

    /// The registry and the grammar crates say the same thing, in both
    /// directions.
    ///
    /// This is the check that used to be a paragraph in a review comment.
    /// Adding a language means writing a grammar and registering it, and
    /// those two halves are in different files with nothing between them:
    /// a crate nobody registered builds and tests and is unreachable from
    /// `--lang`, and a registered language with no crate is a `--lang`
    /// value that fails at the point of use.
    #[test]
    fn the_registry_and_the_grammar_crates_agree() {
        let crates = crates_dir();

        // Every grammar crate is a language, named the same way, claiming
        // the extensions the registry routes to it.
        let mut found = Vec::new();
        for entry in std::fs::read_dir(&crates).unwrap() {
            let dir = entry.unwrap().path();
            // A grammar crate keeps grammar.js at its root, or one per
            // VARIANT directory (VARIANTS.md §2). Looking only at the root
            // would drop a multi-variant language out of this check
            // entirely — the same silently-ungated hole that globbing one
            // level deep opened in CI's discovery step.
            if !super::variant_dirs(&dir)
                .iter()
                .any(|d: &std::path::PathBuf| d.join("grammar.js").is_file())
            {
                continue;
            }
            match registered(&dir) {
                Ok(lang) => found.push(lang),
                Err(e) => panic!("{}: {e:#}", dir.display()),
            }
        }

        // And every language that is its own grammar has one.
        for &lang in LangName::ALL {
            if lang.grammar() != lang {
                continue;
            }
            assert!(
                found.contains(&lang),
                "{lang} is registered as its own grammar but crates/{} has no \
                 grammar.js, at its root or in a variant directory",
                lang.grammar_crate(),
            );
        }
    }
}
