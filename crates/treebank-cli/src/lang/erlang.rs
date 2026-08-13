//! Erlang: Hex packages, adjudicated by a union of OTP's two front ends.
//!
//! Erlang is the language the roadmap picked to sit right after C, for the
//! contrast: the same preprocessor hazard that makes C Tier B is supposed to
//! be a solved problem here, because the ecosystem shipped `epp_dodger` —
//! a parser that reads source *without* running the preprocessor, so a file
//! never needs its includes to be judged.
//!
//! That is true, and it is only half of what an oracle needs. Measured over
//! 1,715 real files, `epp_dodger` alone rejects 6.1% of them, because it
//! parses a `-define`'s body as though it were a form and a macro body does
//! not have to be one — `-define(IS_ETAGC(C), C =:= 16#21; C >= 16#23, ...)`
//! is valid Erlang that it cannot read. Rejecting valid files books them as
//! corpus *noise*, which is the direction that hides grammar gaps. So
//! `validate()` runs a union: a file is valid if `epp_dodger` accepts it OR
//! the real preprocessor `epp` does. The two fail in nearly disjoint
//! directions and the union rejects 0.52%. `tools/erl-oracle/check.escript`
//! carries the full reasoning and the measurements.
//!
//! Everything about the registry is shared with elixir and lives in
//! `hex.rs`, including the two-archive tarball shape.

use std::collections::HashMap;
use std::path::Path;

use anyhow::Result;

use super::{stdin_oracle, Lang};
use crate::ledger::LangName;
use crate::rank::RankedCrate;

pub struct Erlang;

/// Where an off-PATH `escript` is named, following zig's
/// `TREEBANK_ZIG_ORACLE` and elixir's `TREEBANK_ELIXIR`.
/// `tools/beam-toolchain/fetch.sh --otp-only` installs the pinned OTP under
/// `~/.local/beam` rather than into the system.
const ORACLE_ENV: &str = "TREEBANK_ESCRIPT";

/// What the ledger pins, kept here too so the error message can name it.
const ORACLE_VERSION: &str = "Erlang/OTP 28";

impl Lang for Erlang {
    fn name(&self) -> LangName {
        LangName::Erlang
    }

    fn rank(&self, _db: &Path, k: usize) -> Result<Vec<RankedCrate>> {
        super::hex::rank(k)
    }

    fn resolve(&self, pkg: &RankedCrate) -> Result<(String, String)> {
        super::hex::resolve(pkg)
    }

    /// `.erl` and `.hrl`. One grammar parses both: a header is a sequence of
    /// forms like a module is, and headers are where the macro-heavy syntax
    /// lives, which is the interesting half for a grammar.
    ///
    /// tree-sitter-erlang's `tree-sitter.json` claims four more file types —
    /// `app`, `app.src`, `escript` and `rebar.config` — and they are left out
    /// for now, deliberately rather than by oversight. The first three and
    /// `rebar.config` are Erlang *terms* rather than modules: a build
    /// manifest and an application spec, data with no functions in it. They
    /// would change the corpus's character the way `.rockspec` files would
    /// have changed lua's, and `.escript` additionally starts with a `#!`
    /// line that is not Erlang at all. Adding them is a deliberate change
    /// with its own sweep evidence, not a silent widening.
    fn classify(&self, rel: &Path) -> Option<Option<String>> {
        matches!(rel.extension()?.to_str()?, "erl" | "hrl").then_some(None)
    }

    /// A Hex tarball's four outer members are single-component names, so the
    /// tar default of stripping one would drop every entry and the package
    /// would extract empty — silently, since an empty package is not an error
    /// anywhere. See `hex.rs`.
    fn archive_strip(&self, _entry: &Path, _is_zip: bool) -> usize {
        0
    }

    /// The source is inside `contents.tar.gz`, one level down.
    fn nested_archives(&self) -> bool {
        true
    }

    fn grammar_dirs(&self) -> &'static [&'static str] {
        &["."]
    }

    /// `tools/erl-oracle/check.escript`, a union of `epp_dodger` (no
    /// preprocessor, no includes needed) and `epp` (the real preprocessor,
    /// given the include dirs that can be reconstructed from the corpus
    /// tree). A file is valid if either accepts it.
    ///
    /// The corpus root is passed as an argument because `epp` needs the
    /// file's project around it and there is no build system to ask: the
    /// oracle walks every directory from the file up to that root, offering
    /// each one plus its `include/` and `src/`. Without the bound it would
    /// walk out of the corpus entirely.
    ///
    /// Two things this oracle must not become, both measured and both
    /// recorded in the ledger. `epp_dodger:parse_file/1` returns `{ok, _}`
    /// for a file full of syntax errors — they arrive as error *forms* — so
    /// the obvious reading calls every broken file valid. And
    /// `quick_parse_file/1`, which is 11% faster and agrees with
    /// `parse_file` on all 1,715 corpus files, silently accepts 21 of 24
    /// hand-written syntax errors. Agreement on clean library code is worth
    /// nothing here; only the negative battery caught it.
    fn validate(&self, srcroot: &Path, paths: &[String]) -> Result<HashMap<String, bool>> {
        let program = std::env::var(ORACLE_ENV).unwrap_or_else(|_| "escript".to_string());
        let root = srcroot.to_string_lossy().into_owned();
        stdin_oracle::run(
            &program,
            &["tools/erl-oracle/check.escript", &root],
            &format!(
                "{program} tools/erl-oracle/check.escript — is {ORACLE_VERSION} installed? \
                 (tools/beam-toolchain/fetch.sh --otp-only, then export {ORACLE_ENV}=<prefix>/bin/escript)"
            ),
            srcroot,
            paths,
        )
    }
}
