//! Elixir: Hex packages, adjudicated by Elixir's own front end.
//!
//! Two things here are not shared with the languages before it.
//!
//! 1. **A Hex tarball is two archives.** `name-version.tar` is an outer tar
//!    holding `VERSION`, `CHECKSUM`, `metadata.config` and `contents.tar.gz`
//!    — the source is one level down. That is the shape `nested_archives`
//!    was built for by lua, where a quarter of `.src.rock` files carried
//!    upstream's tarball whole; here it is every package rather than a
//!    quarter, so the nested prefix (`contents.tar.gz/lib/foo.ex`) shows up
//!    on every corpus path. It is kept rather than special-cased because it
//!    is true, and because the alternative is a new trait hook that exists
//!    to make one registry's paths prettier.
//!
//! 2. **Hex is not an Elixir registry, it is the BEAM's registry.** Erlang
//!    packages live in the same namespace and rank alongside Elixir ones:
//!    measured over the top 200 by recent downloads, **48 (24%) contain no
//!    `.ex`/`.exs` file at all** — telemetry, ranch, cowlib, idna and the
//!    rest are Erlang. They are fetched and contribute nothing, which is
//!    the honest outcome: filtering them out would need a per-release
//!    request for `meta.build_tools` (the listing endpoint does not carry
//!    it) to save a few megabytes of download, and the same 48 packages are
//!    the entire point of the corpus for the Erlang grammar that comes next.

use std::collections::HashMap;
use std::path::Path;

use anyhow::Result;

use super::{stdin_oracle, Lang};
use crate::ledger::LangName;
use crate::rank::RankedCrate;

pub struct Elixir;

/// Where an off-PATH oracle interpreter is named, following zig's
/// `TREEBANK_ZIG_ORACLE`. `tools/beam-toolchain/fetch.sh` installs the
/// pinned Elixir under `~/.local/beam` rather than into the system, so on
/// most boxes this is how it is found; a distribution `elixir` on PATH is
/// used when the variable is unset, and `check.exs` refuses to produce
/// verdicts under the wrong minor either way.
const ORACLE_ENV: &str = "TREEBANK_ELIXIR";

/// What the ledger pins, kept here too so the error message can name it —
/// the difference between "oracle missing" and knowing what to install.
const ORACLE_VERSION: &str = "Elixir 1.20.3 on OTP 28";

impl Lang for Elixir {
    fn name(&self) -> LangName {
        LangName::Elixir
    }

    fn rank(&self, _db: &Path, k: usize) -> Result<Vec<RankedCrate>> {
        super::hex::rank(k)
    }

    /// Pure: `rank` already resolved the version, because Hex's listing
    /// endpoint carries `latest_stable_version` per package and a second
    /// request per package would buy nothing.
    ///
    /// The tarball is the published release itself — Hex has no separate
    /// "sdist", every release IS source, which is why the roadmap calls the
    /// corpus conventional for this language.
    fn resolve(&self, pkg: &RankedCrate) -> Result<(String, String)> {
        Ok((
            pkg.version.clone(),
            format!(
                "https://repo.hex.pm/tarballs/{}-{}.tar",
                pkg.name, pkg.version
            ),
        ))
    }

    /// `.ex` and `.exs`, the two extensions tree-sitter-elixir's
    /// `tree-sitter.json` claims, and one grammar parses both: the
    /// difference is that `.exs` is evaluated rather than compiled, which is
    /// a runtime distinction with no syntax behind it. Test files, `mix.exs`
    /// and `.formatter.exs` are therefore in the corpus, and should be —
    /// they are where a package's more unusual syntax tends to live.
    ///
    /// `.heex` and `.eex` are deliberately absent. They are template
    /// languages that EMBED Elixir rather than being Elixir, they have their
    /// own grammars (`tree-sitter-heex`, which Zed pins beside this one),
    /// and feeding them to this parser would manufacture failures out of
    /// files that are not Elixir. Measured on the top 200: one `.ex` file in
    /// the corpus is an EEx template wearing the wrong extension
    /// (credo's `.template.check.ex`), and the oracle correctly calls it
    /// invalid, so that class lands as noise rather than as a gap.
    fn classify(&self, rel: &Path) -> Option<Option<String>> {
        matches!(rel.extension()?.to_str()?, "ex" | "exs").then_some(None)
    }

    /// A Hex tarball wraps nothing: the outer tar's members are four
    /// single-component names (`VERSION`, `CHECKSUM`, `metadata.config`,
    /// `contents.tar.gz`), and `contents.tar.gz`'s own members are already
    /// root-relative (`lib/foo.ex`). The default of one component for a tar
    /// would strip those four names to nothing and drop every entry, so the
    /// package would extract empty — silently, since an empty package is not
    /// an error anywhere.
    fn archive_strip(&self, _entry: &Path, _is_zip: bool) -> usize {
        0
    }

    /// The source is inside `contents.tar.gz`. Without this every Hex
    /// package extracts to four metadata files and no code.
    fn nested_archives(&self) -> bool {
        true
    }

    fn grammar_dirs(&self) -> &'static [&'static str] {
        &["."]
    }

    /// `tools/elixir-oracle/check.exs`: Elixir's own front end via
    /// `Code.string_to_quoted/2`, the call the compiler makes to turn a
    /// file's text into an AST. It tokenizes and parses and stops — no macro
    /// expansion, no `use`/`import`/`require` resolution, no module
    /// attribute evaluation, no `.exs` execution — so a missing dependency
    /// is not an error and each file is judged on its own text.
    ///
    /// That it does not evaluate was verified rather than trusted, because
    /// Elixir runs arbitrary code at compile time and this is pointed at
    /// thousands of strangers' files: six adversarial fixtures produced zero
    /// side effects through this path, while compiling and running the same
    /// six produced ten. The battery and its control are described in
    /// `check.exs` and in the ledger.
    ///
    /// Batched through one long-lived VM rather than forked per file, which
    /// is worth 325x: `elixir -e` per file measures 312 s per 1000 — outside
    /// Tier A altogether — against 0.96 s per 1000 here, because the BEAM
    /// costs ~0.49 s to start and a batch pays that once.
    fn validate(&self, srcroot: &Path, paths: &[String]) -> Result<HashMap<String, bool>> {
        let program = std::env::var(ORACLE_ENV).unwrap_or_else(|_| "elixir".to_string());
        stdin_oracle::run(
            &program,
            &["tools/elixir-oracle/check.exs"],
            &format!(
                "{program} tools/elixir-oracle/check.exs — is {ORACLE_VERSION} installed? \
                 (tools/beam-toolchain/fetch.sh, then export {ORACLE_ENV}=<prefix>/bin/elixir)"
            ),
            srcroot,
            paths,
        )
    }
}
