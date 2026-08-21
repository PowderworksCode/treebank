use std::path::Path;
use std::process::Command;

use anyhow::{bail, Context, Result};
use sha2::{Digest, Sha256};

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

    let mut hasher = Sha256::new();
    hasher.update(std::fs::read(&parser_c)?);
    if scanner_c.exists() {
        let scanner = std::fs::read(&scanner_c)?;
        hasher.update(&scanner);
        // A variant's src/scanner.c is a stub that `#include`s the shared
        // scanner (VARIANTS.md §2), so the stub's bytes do not change when
        // the real scanner does. Without hashing what it includes, editing
        // common/scanner.c leaves this cache serving the PREVIOUS parser --
        // which is how a deliberately broken variant first passed the
        // crossvariant gate.
        for included in local_includes(&scanner_c, &scanner) {
            hasher.update(std::fs::read(&included).with_context(|| {
                format!("{} includes {}", scanner_c.display(), included.display())
            })?);
        }
    }
    let key = format!("{:x}", hasher.finalize());
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
        Ok((language, key[..16].to_string()))
    }
}

/// Quoted `#include` paths in a C file, resolved relative to it. One level
/// deep, which is all the variant stubs use; a deeper chain would need this
/// to recurse, and the day one appears is the day to make it.
pub fn local_includes(file: &Path, contents: &[u8]) -> Vec<std::path::PathBuf> {
    let dir = file.parent().unwrap_or(Path::new("."));
    String::from_utf8_lossy(contents)
        .lines()
        .filter_map(|line| {
            let line = line.trim_start();
            let rest = line.strip_prefix("#include")?.trim_start();
            let inner = rest.strip_prefix('"')?;
            let end = inner.find('"')?;
            Some(dir.join(&inner[..end]))
        })
        .filter(|p| p.exists())
        .collect()
}
