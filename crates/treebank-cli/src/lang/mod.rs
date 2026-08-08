//! One implementation per language. Everything language-specific — where the
//! ranking comes from, how tarballs resolve, which files belong in the
//! corpus, how files route to grammars, and what the reference parser is —
//! lives behind this trait; rank/fetch/sweep/oracle are generic drivers.

mod csharp;
mod java;
mod javascript;
mod node_oracle;
mod npm;
mod rust;
mod typescript;

use std::collections::HashMap;
use std::path::Path;

use anyhow::{bail, Result};

use crate::rank::RankedCrate;

pub trait Lang: Sync {
    /// Canonical name; matches `--lang` and the `corpus/<lang>/` dir.
    fn name(&self) -> &'static str;

    /// Build the ranked top-K package list. `db` is `corpus/<lang>/db`
    /// (local dump data; only languages that need one read it).
    fn rank(&self, db: &Path, k: usize) -> Result<Vec<RankedCrate>>;

    /// Resolve a ranked entry to (version, tarball_url).
    fn resolve(&self, pkg: &RankedCrate) -> Result<(String, String)>;

    /// Does this file belong in the corpus? `Some(dialect)` if so, where the
    /// dialect (e.g. "tsx") is the grammar-routing hint stored in the
    /// manifest; `Some(None)` means the language's default grammar.
    fn classify(&self, rel: &Path) -> Option<Option<String>>;

    /// Grammar dirs to load, in routing-index order, relative to the
    /// grammar repo root. Single-grammar languages return `["."]`.
    fn grammar_dirs(&self) -> &'static [&'static str];

    /// Index into `grammar_dirs()` for a file.
    fn route(&self, dialect: &Option<String>, rel: &str) -> usize {
        let _ = (dialect, rel);
        0
    }

    /// Reference-parser validity for a batch of corpus-relative paths.
    fn validate(&self, srcroot: &Path, paths: &[String]) -> Result<HashMap<String, bool>>;
}

pub fn get(name: &str) -> Result<&'static dyn Lang> {
    static RUST: rust::Rust = rust::Rust;
    static TYPESCRIPT: typescript::TypeScript = typescript::TypeScript;
    static JAVASCRIPT: javascript::JavaScript = javascript::JavaScript;
    static JAVA: java::Java = java::Java;
    static CSHARP: csharp::CSharp = csharp::CSharp;
    Ok(match name {
        "rust" => &RUST,
        "typescript" => &TYPESCRIPT,
        "javascript" => &JAVASCRIPT,
        "java" => &JAVA,
        "csharp" => &CSHARP,
        other => bail!("unsupported lang {other} (have: rust, typescript, javascript, java, csharp)"),
    })
}
