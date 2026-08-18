//! Reference-parser oracles. "Our parser errored" does not mean "grammar
//! bug": corpora are full of test fixtures, templates, snippets and
//! other-version files, so every parse failure is adjudicated by the
//! language's reference parser before it is called a gap.
//!
//! Two rules every oracle follows, both learned from real defects:
//!
//! - **An unreadable file is never an invalid file.** `validate` is only
//!   called on files the grammar already failed, and an `invalid` verdict
//!   books the file as corpus noise — so an oracle that answers `invalid`
//!   for files it could not read silently converts every grammar failure
//!   into noise and reports a flawless grammar. Oracles fail loudly (an
//!   error, no verdict) on anything that stops them reading the bytes.
//! - **An oracle is proved by a negative battery, never by agreement.**
//!   Agreement on clean library code is worth nothing; only files that
//!   should be rejected test whether the oracle can reject.
//!
//! Deliberately self-contained (no grammar, corpus or sweep knowledge) so
//! the whole crate can move out of this repository. The node/python oracle
//! programs live in this crate's `tools/` and are resolved relative to
//! `CARGO_MANIFEST_DIR`, so the binary works from any cwd in-repo.

mod javascript;
mod python;
mod rust;
mod rust_spans;
mod spans;
mod stdin_oracle;
mod typescript;

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use anyhow::Result;

pub use spans::{get as spans_for, Edge, FileSpans, Span, SpanOracle};
pub use treebank_lang::LangName;

/// The oracle programs shipped inside this crate.
fn tool(name: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("tools").join(name)
}

pub trait Oracle: Sync {
    fn name(&self) -> LangName;

    /// Reference-parser validity for a batch of corpus-relative paths.
    /// Every requested path gets a verdict or the call errors; a missing
    /// verdict is never silently dropped.
    ///
    /// This is the UNION verdict: valid if ANY version family accepts.
    fn validate(&self, srcroot: &Path, paths: &[String]) -> Result<HashMap<String, bool>>;

    /// Validity under the CURRENT version of the language only.
    ///
    /// Separate from `validate` because the two answer different questions.
    /// `validate` asks "is this valid in the language at all", which is what
    /// decides gap-vs-noise. This asks "is this still valid today", which is
    /// what lets a declared version-policy rejection (DESIGN.md §4.2) be
    /// distinguished from a genuine gap: a construct may only be booked as
    /// `version` if the current oracle also rejects it. Without that second
    /// condition a policy entry could suppress a real, current-language
    /// failure, which is precisely the kind of self-granted exemption the
    /// sweep exists to prevent.
    ///
    /// Defaults to `validate`, which is correct for every language whose
    /// oracle has no version split.
    fn validate_current(&self, srcroot: &Path, paths: &[String]) -> Result<HashMap<String, bool>> {
        self.validate(srcroot, paths)
    }
}

/// Total: an unsupported name cannot be constructed, because clap and serde
/// both reject it at the boundary. Nothing here can fail.
pub fn get(name: LangName) -> &'static dyn Oracle {
    static PYTHON: python::Python = python::Python;
    static RUST: rust::Rust = rust::Rust;
    static TYPESCRIPT: typescript::TypeScript = typescript::TypeScript;
    static JAVASCRIPT: javascript::JavaScript = javascript::JavaScript;
    match name {
        LangName::Python => &PYTHON,
        LangName::Rust => &RUST,
        LangName::Typescript => &TYPESCRIPT,
        LangName::Javascript => &JAVASCRIPT,
    }
}
