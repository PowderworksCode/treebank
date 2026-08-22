use std::path::Path;
use std::process::Command;

use anyhow::{bail, Context, Result};
use sha2::{Digest, Sha256};

/// Exact identity of the generated sources that determine parser behaviour.
///
/// This is deliberately content-addressed rather than a repository commit:
/// a ledger committed alongside a grammar cannot name the commit containing
/// itself, and unrelated documentation commits do not make parser evidence
/// stale.
pub fn source_sha256(grammar_dir: &Path) -> Result<String> {
    let src = grammar_dir.join("src");
    let parser_c = src.join("parser.c");
    if !parser_c.exists() {
        bail!(
            "{} not found — not a generated grammar dir?",
            parser_c.display()
        );
    }
    let scanner_c = src.join("scanner.c");
    let mut hasher = Sha256::new();
    let parser = std::fs::read(&parser_c)?;
    hasher.update(b"parser.c\0");
    hasher.update((parser.len() as u64).to_le_bytes());
    hasher.update(parser);
    if scanner_c.exists() {
        let scanner = std::fs::read(&scanner_c)?;
        hasher.update(b"scanner.c\0");
        hasher.update((scanner.len() as u64).to_le_bytes());
        hasher.update(scanner);
    }
    Ok(format!("{:x}", hasher.finalize()))
}

/// Compile a grammar's parser.c (+ scanner.c if present) into a dylib and
/// load the tree-sitter Language from it, returning the language plus a
/// fingerprint of the compiled sources (used both to cache the dylib and to
/// key the sweep cache). The dylib is cached in the OS temp dir, so repeat
/// sweeps skip the compile.
pub fn load(grammar_dir: &Path) -> Result<(tree_sitter::Language, String)> {
    let src = grammar_dir.join("src");
    let parser_c = src.join("parser.c");
    if !parser_c.exists() {
        bail!(
            "{} not found — not a generated grammar dir?",
            parser_c.display()
        );
    }
    let scanner_c = src.join("scanner.c");

    let grammar_json: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(src.join("grammar.json"))?)
            .context("parse src/grammar.json")?;
    let name = grammar_json["name"]
        .as_str()
        .context("grammar.json has no name")?
        .to_string();

    let key = source_sha256(grammar_dir)?;
    let dylib = std::env::temp_dir().join(format!("treebank-{name}-{}.dylib", &key[..16]));

    if !dylib.exists() {
        let mut cmd = Command::new("cc");
        cmd.arg("-fPIC")
            .arg("-shared")
            .arg("-O1")
            .arg("-I")
            .arg(&src);
        cmd.arg(&parser_c);
        if scanner_c.exists() {
            cmd.arg(&scanner_c);
        }
        cmd.arg("-o").arg(&dylib);
        let out = cmd.output().context("run cc")?;
        if !out.status.success() {
            bail!("cc failed:\n{}", String::from_utf8_lossy(&out.stderr));
        }
        eprintln!("grammar: compiled {} -> {}", name, dylib.display());
    }

    let symbol_name = format!("tree_sitter_{name}");
    unsafe {
        let lib = libloading::Library::new(&dylib)?;
        let func: libloading::Symbol<unsafe extern "C" fn() -> *const ()> = lib
            .get(symbol_name.as_bytes())
            .with_context(|| format!("symbol {symbol_name}"))?;
        let language = tree_sitter::Language::from_raw(func() as *const _);
        // Keep the dylib mapped for the life of the process.
        std::mem::forget(lib);
        Ok((language, key))
    }
}
