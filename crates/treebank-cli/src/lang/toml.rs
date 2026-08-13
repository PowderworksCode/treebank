use std::collections::HashMap;
use std::path::Path;

use anyhow::Result;
use rayon::prelude::*;

use super::{rust, Lang};
use crate::ledger::LangName;
use crate::rank::RankedCrate;

pub struct Toml;

impl Lang for Toml {
    fn name(&self) -> LangName {
        LangName::Toml
    }

    /// TOML owns no registry, so it has no ranking of its own. It is a
    /// *guest* language, like bash inside Debian sources: it never ships a
    /// package, it only rides inside other people's. crates.io is the host
    /// with by far the most TOML per byte — every published crate carries a
    /// `Cargo.toml` and many carry `rustfmt.toml`, `clippy.toml`,
    /// `deny.toml`, `rust-toolchain.toml` — and it is the ecosystem whose
    /// download figure the roadmap costed this language against.
    ///
    /// So the ranking is rust's, delegated rather than reimplemented: the
    /// corpus is "the `.toml` files inside the top-K crates by all-time
    /// downloads". That makes the two corpora share a fetch and a cache,
    /// and it makes this language's ranking exactly as good as rust's,
    /// which is the honest description of what it is.
    fn rank(&self, db: &Path, k: usize) -> Result<Vec<RankedCrate>> {
        rust::Rust.rank(db, k)
    }

    fn resolve(&self, pkg: &RankedCrate) -> Result<(String, String)> {
        rust::Rust.resolve(pkg)
    }

    /// `.toml` only — the single extension tree-sitter-toml's
    /// `tree-sitter.json` claims, following the same rule as python, lua and
    /// javascript.
    ///
    /// Deliberately narrower than the editors. Helix routes `Cargo.lock`,
    /// `poetry.lock`, `pdm.lock`, `uv.lock` and eight `*.conf` globs
    /// (containers, mounts, policy, registries, storage, staticcheck) to
    /// this same grammar, and they are all really TOML. They are left out
    /// so `classify()` matches what the grammar itself advertises, and
    /// because they would change the corpus's character: a lock file is
    /// machine-generated output, and one `Cargo.lock` per crate would
    /// swamp the hand-written files with a single template. Adding them is
    /// a deliberate change with its own sweep evidence, not a silent
    /// widening — the same call treebank-lua made about `.rockspec`.
    fn classify(&self, rel: &Path) -> Option<Option<String>> {
        (rel.extension()?.to_str()? == "toml").then_some(None)
    }

    fn grammar_dirs(&self) -> &'static [&'static str] {
        &["."]
    }

    /// The `toml` crate, in-process, like rust's `syn` — no fork, no stdin
    /// protocol, 0.053 s per 1000 files.
    ///
    /// `Table` and not a syntax-only parse: duplicate keys, a table
    /// redefined over a key, dotted-key conflicts and calendar-invalid
    /// datetimes (`2024-02-30`) are all *spec-level* invalid TOML rather
    /// than a later type-check, so a syntax-only TOML parser silently
    /// accepts about a dozen classes of invalid TOML — the treebank-php
    /// trap. `toml::de::DeTable` and `toml_edit::DocumentMut` were measured
    /// against this on a 25-file adversarial battery and agree on every
    /// file, so the stage is a checked non-choice; the ledger records that.
    ///
    /// The crate parses `&str`, so this function owns exactly one spec
    /// rule: UTF-8 well-formedness. That is deliberate and it fails closed
    /// — TOML requires a valid UTF-8 document, so ill-formed bytes are a
    /// *verdict* (invalid, i.e. corpus noise) and not an I/O failure. An
    /// unreadable file is a different thing and is NOT a verdict: it
    /// propagates as an error, so a mistyped corpus root fails the sweep
    /// instead of turning every grammar failure into noise.
    ///
    /// Do not strip a leading BOM here. The crate already distinguishes a
    /// leading BOM (valid), a mid-stream one (invalid) and a doubled one
    /// (invalid); pre-stripping would turn the doubled case into the
    /// leading case and silently call an invalid file valid.
    fn validate(&self, srcroot: &Path, paths: &[String]) -> Result<HashMap<String, bool>> {
        paths
            .par_iter()
            .map(|p| {
                let bytes = std::fs::read(srcroot.join(p))
                    .map_err(|e| anyhow::anyhow!("toml oracle: read {}: {e}", p))?;
                let valid = match std::str::from_utf8(&bytes) {
                    Err(_) => false,
                    Ok(text) => text.parse::<::toml::Table>().is_ok(),
                };
                Ok((p.clone(), valid))
            })
            .collect()
    }
}
