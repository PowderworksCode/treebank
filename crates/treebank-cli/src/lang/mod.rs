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

use anyhow::Result;

use crate::ledger::LangName;
use crate::rank::RankedCrate;

pub trait Lang: Sync {
    /// Canonical name; matches `--lang` and the `corpus/<lang>/` dir.
    fn name(&self) -> LangName;

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

/// Total: an unsupported name cannot be constructed, because clap and serde
/// both reject it at the boundary. Nothing here can fail.
pub fn get(name: LangName) -> &'static dyn Lang {
    static RUST: rust::Rust = rust::Rust;
    static TYPESCRIPT: typescript::TypeScript = typescript::TypeScript;
    static JAVASCRIPT: javascript::JavaScript = javascript::JavaScript;
    static JAVA: java::Java = java::Java;
    static CSHARP: csharp::CSharp = csharp::CSharp;
    match name {
        LangName::Rust => &RUST,
        LangName::Typescript => &TYPESCRIPT,
        LangName::Javascript => &JAVASCRIPT,
        LangName::Java => &JAVA,
        LangName::Csharp => &CSHARP,
    }
}
