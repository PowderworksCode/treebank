//! One implementation per language. Everything language-specific — where the
//! ranking comes from, how tarballs resolve, which files belong in the
//! corpus, how files route to grammars, and what the reference parser is —
//! lives behind this trait; rank/fetch/sweep/oracle are generic drivers.

mod bash;
mod c;
mod csharp;
mod debian;
mod exec_oracle;
mod github;
mod go;
mod java;
mod javascript;
mod npm;
mod php;
mod python;
mod rust;
mod stdin_oracle;
mod typescript;

use std::collections::HashMap;
use std::path::Path;

use anyhow::Result;

use treebank_preprocessing::Symbols;

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

    /// Second-stage filter, with the file's bytes in hand, for languages
    /// where the extension does not settle which language a file even is.
    /// C uses it to drop C++ headers: `.h` belongs to both languages and
    /// only the content tells them apart. Default: keep what `classify` took.
    fn admit(&self, rel: &Path, content: &[u8]) -> bool {
        let _ = (rel, content);
        true
    }

    /// What this language knows for certain about its own preprocessor, if
    /// it has one. `None` — the default — means the source is parsed exactly
    /// as written, which is right for every language without conditional
    /// compilation.
    ///
    /// A grammar parses all `#if` branches at once; a compiler parses only
    /// the live ones. Where that difference makes a file unparseable *as
    /// written* but fine as compiled, the failure is a property of the
    /// preprocessor, not a grammar bug, and the sweep says so rather than
    /// filing it as a gap. See `treebank_preprocessing`.
    fn preprocessing(&self) -> Option<&'static Symbols> {
        None
    }

    /// How many leading components of an archive member's path are the
    /// archive's own wrapper and should be dropped.
    ///
    /// Compressed tarballs (crates.io, npm, GitHub source archives, Debian
    /// `.orig.tar.*`) wrap everything in one `<name>-<version>/` directory;
    /// Maven sources jars and nupkgs are already root-relative. Container
    /// format was a good enough proxy for that distinction until Go, which
    /// is a zip that must be stripped, and by more than one component:
    /// every entry of a module proxy zip is prefixed `<module>@<version>/`,
    /// and `github.com/spf13/cobra@v1.10.2/` is three components. So the
    /// count is the language's to decide; the default is what every
    /// language did before Go existed.
    fn archive_strip(&self, entry: &Path, is_zip: bool) -> usize {
        let _ = entry;
        usize::from(!is_zip)
    }

    /// Largest artifact this language will download, if it has a limit.
    ///
    /// Registry tarballs are bounded by what an author publishes. Artifacts
    /// are not: a distribution's source archive is as big as the project it
    /// packages, and for a *guest* language — one that never owns a package,
    /// only rides inside them — the size is set by the host language while
    /// the yield is not. Bash measured 11.5 GB for its top 500 Debian
    /// sources, of which 7.6 GB was eight packages (three TeX
    /// distributions, two browsers, two Qt WebEngines, LibreOffice).
    /// `None` — the default — keeps every existing language's behaviour.
    fn max_artifact_bytes(&self) -> Option<u64> {
        None
    }

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
    static C: c::C = c::C;
    static PYTHON: python::Python = python::Python;
    static PHP: php::Php = php::Php;
    static GO: go::Go = go::Go;
    static BASH: bash::Bash = bash::Bash;
    match name {
        LangName::Rust => &RUST,
        LangName::Typescript => &TYPESCRIPT,
        LangName::Javascript => &JAVASCRIPT,
        LangName::Java => &JAVA,
        LangName::Csharp => &CSHARP,
        LangName::C => &C,
        LangName::Python => &PYTHON,
        LangName::Php => &PHP,
        LangName::Go => &GO,
        LangName::Bash => &BASH,
    }
}
