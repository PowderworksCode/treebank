//! Corpus acquisition. Everything ecosystem-specific — where the ranking
//! comes from, how tarballs resolve, which files belong in the corpus —
//! lives behind [`Ecosystem`]; `rank` and `fetch` are generic drivers.
//!
//! Deliberately self-contained (no grammar, sweep or oracle knowledge) so
//! the whole crate can move out of this repository.

pub mod fetch;
pub mod rank;

mod bash;
mod debian;
mod github;
mod java;
mod javascript;
mod npm;
mod python;
mod rust;
mod typescript;

use std::path::Path;

use anyhow::Result;

use crate::rank::RankedCrate;
pub use treebank_lang::LangName;

pub trait Ecosystem: Sync {
    /// Canonical name; matches `--lang` and the `corpus/<lang>/` dir.
    fn name(&self) -> LangName;

    /// Build the ranked top-K package list. `db` is `corpus/<lang>/db`
    /// (local dump data; only ecosystems that need one read it).
    fn rank(&self, db: &Path, k: usize) -> Result<Vec<RankedCrate>>;

    /// Resolve a ranked entry to (version, tarball_url).
    fn resolve(&self, pkg: &RankedCrate) -> Result<(String, String)>;

    /// Does this file belong in the corpus? `Some(dialect)` if so, where the
    /// dialect (e.g. "tsx") is the grammar-routing hint stored in the
    /// manifest; `Some(None)` means the language's default grammar.
    fn classify(&self, rel: &Path) -> Option<Option<String>>;

    /// Second-stage filter, with the file's bytes in hand, for languages
    /// where the extension does not settle which language a file even is.
    /// Default: keep what `classify` took.
    fn admit(&self, rel: &Path, content: &[u8]) -> bool {
        let _ = (rel, content);
        true
    }

    /// May an archive member be an archive worth walking into? Default
    /// `false`, which is right for every registry that ships one tarball
    /// per package. (LuaRocks was the ecosystem that needed `true`: a
    /// `.src.rock` is a zip that often carries upstream's release tarball
    /// inside. Recursion is one level.)
    fn nested_archives(&self) -> bool {
        false
    }

    /// Which member is the payload, for an ecosystem that knows. Default:
    /// any member that looks like an archive. (A `.gem` was the case for
    /// naming it: the source is always `data.tar.gz`, and sniffing its
    /// gzip-but-not-tar siblings costs two skipped-archive warnings per
    /// package.)
    fn nested_archive_member(&self, rel: &Path) -> bool {
        let _ = rel;
        true
    }

    /// How many leading components of an archive member's path are the
    /// archive's own wrapper and should be dropped. Compressed tarballs
    /// (crates.io, npm, PyPI sdists) wrap everything in one
    /// `<name>-<version>/` directory; some formats are root-relative, and
    /// Go's module zips prefix `<module>@<version>/`, which can be several
    /// components — so the count is the ecosystem's to decide.
    fn archive_strip(&self, entry: &Path, is_zip: bool) -> usize {
        let _ = entry;
        usize::from(!is_zip)
    }

    /// Largest artifact this ecosystem will download, if it has a limit.
    /// Registry tarballs are bounded by what an author publishes; a
    /// distribution's source archive is not. `None` — the default — means
    /// unbounded.
    fn max_artifact_bytes(&self) -> Option<u64> {
        None
    }
}

/// Total: an unsupported name cannot be constructed, because clap and serde
/// both reject it at the boundary. Nothing here can fail.
pub fn get(name: LangName) -> &'static dyn Ecosystem {
    static PYTHON: python::Python = python::Python;
    static RUST: rust::Rust = rust::Rust;
    static TYPESCRIPT: typescript::TypeScript = typescript::TypeScript;
    static JAVASCRIPT: javascript::JavaScript = javascript::JavaScript;
    static JAVA: java::Java = java::Java;
    static BASH: bash::Bash = bash::Bash;
    match name {
        LangName::Python => &PYTHON,
        LangName::Rust => &RUST,
        LangName::Typescript => &TYPESCRIPT,
        LangName::Javascript => &JAVASCRIPT,
        LangName::Java => &JAVA,
        LangName::Bash => &BASH,
    }
}
