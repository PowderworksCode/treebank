//! Zig.
//!
//! Two things are different here from every other language in this crate,
//! and both come from the same fact: Zig's syntax moves between releases.
//!
//! 1. **The oracle version is half the verdict.** "Is this file valid Zig?"
//!    has no answer until a compiler version is named. `validate()` runs
//!    `tools/zig-oracle` built by exactly one pinned toolchain, and
//!    `ledger.json` records which — the same load-bearing role
//!    `generate_cli` plays for the parser and `oracle_version` plays for
//!    CPython. Measured on 11,672 files from this corpus, 801 (6.86%) do
//!    not get the same verdict from 0.11.0 through 0.16.0. Silently
//!    adjudicating with whatever `zig` is on PATH would make the gap
//!    numbers drift every time the box is updated, so this deliberately
//!    does not look on PATH: `TREEBANK_ZIG_ORACLE` names the binary, and
//!    the error when it is missing says which version to build.
//!    `crates/treebank-zig/ORACLE.md` has the full measurement.
//!
//! 2. **The corpus comes from GitHub, not a registry.** Zig has no package
//!    registry: `build.zig.zon` dependencies are URLs with content hashes,
//!    not registry coordinates, so there is nothing to rank by downloads.
//!    This uses the artifact-corpus path in `github.rs` — repositories by
//!    stars — whose biases are documented there and are real: stars are
//!    attention rather than use, and `language:zig` selects repositories
//!    that are *mostly* Zig. "Zig on GitHub by stars" is a specific and
//!    defensible population, but it is one population, not the population.

use std::collections::HashMap;
use std::path::Path;

use anyhow::{bail, Result};

use super::Lang;
use crate::ledger::LangName;
use crate::rank::RankedCrate;

pub struct Zig;

/// Where the pinned oracle binary is found. Deliberately NOT `zig` on PATH.
const ORACLE_ENV: &str = "TREEBANK_ZIG_ORACLE";

/// The toolchain `ledger.json` pins. Kept here as well so the error message
/// can name it, which is the difference between "oracle missing" and a
/// message that tells you exactly what to install.
const ORACLE_VERSION: &str = "0.16.0";

impl Lang for Zig {
    fn name(&self) -> LangName {
        LangName::Zig
    }

    fn rank(&self, _db: &Path, k: usize) -> Result<Vec<RankedCrate>> {
        super::github::rank(LangName::Zig, "Zig", k)
    }

    fn resolve(&self, pkg: &RankedCrate) -> Result<(String, String)> {
        super::github::resolve(LangName::Zig, pkg)
    }

    /// `.zig` only — the single extension tree-sitter-zig's
    /// `tree-sitter.json` claims, following the same rule as javascript and
    /// python.
    ///
    /// `.zon` is deliberately excluded even though this same parser handles
    /// it: ZON is a *different parse mode* (`Ast.parse(.., .zon)`), it is
    /// data rather than code, and the grammar does not advertise the
    /// extension. Admitting it would mean the oracle and the grammar were
    /// answering different questions about the same file.
    ///
    /// `zig-cache/`, `.zig-cache/` and `zig-out/` are build output — the
    /// former two contain whole copies of generated and dependency source.
    /// A failure there is attributed to the wrong repository and the same
    /// code is already in the corpus under its real owner, which is the
    /// reason javascript excludes bundles and python excludes `_vendor/`.
    fn classify(&self, rel: &Path) -> Option<Option<String>> {
        if rel.components().any(|c| {
            matches!(
                c.as_os_str().to_str(),
                Some("zig-cache") | Some(".zig-cache") | Some("zig-out")
            )
        }) {
            return None;
        }
        (rel.extension()?.to_str()? == "zig").then_some(None)
    }

    fn grammar_dirs(&self) -> &'static [&'static str] {
        &["."]
    }

    /// `tools/zig-oracle`: `std.zig.Ast.parse(gpa, src, .zig)`, the call the
    /// compiler itself makes to turn a file's text into a syntax tree. It
    /// resolves no `@import`, runs no `comptime` and links nothing, so a
    /// missing dependency is not an error and each file is judged entirely
    /// on its own bytes.
    ///
    /// Deliberately the parser and not `zig ast-check` (AstGen), which is
    /// the analogue of the `compile()`-over-`ast.parse()` choice py-oracle
    /// made and which was measured and rejected: AstGen enforces lint-grade
    /// rules (`unused function parameter`, `local variable is never
    /// mutated`) that reject well-formed Zig, it adds builtin-set drift on
    /// top of grammar drift, and on 0.13-0.15 it loops forever on
    /// `((1 + 1));`. See `crates/treebank-zig/ORACLE.md`.
    fn validate(&self, srcroot: &Path, paths: &[String]) -> Result<HashMap<String, bool>> {
        let Ok(bin) = std::env::var(ORACLE_ENV) else {
            bail!(
                "{ORACLE_ENV} is not set. The Zig oracle is pinned to a toolchain \
                 version on purpose — for Zig the compiler version IS the language \
                 version, so adjudicating with whatever `zig` happens to be on PATH \
                 would silently change every verdict when the box is updated.\n\
                 \n\
                 Build it against the version ledger.json pins ({ORACLE_VERSION}):\n\
                 \x20   tools/zig-oracle/build.sh /path/to/zig-{ORACLE_VERSION}/zig\n\
                 \x20   export {ORACLE_ENV}=tools/zig-oracle/check-{ORACLE_VERSION}"
            )
        };
        // The binary's own name carries the version it was built by, so a
        // mismatch against the ledger is visible rather than silent. This is
        // the one check that stops the whole point of the pin from eroding.
        if !bin.ends_with(ORACLE_VERSION) {
            eprintln!(
                "oracle: WARNING — {ORACLE_ENV}={bin} does not end in {ORACLE_VERSION}, \
                 the version ledger.json pins. Verdicts are only comparable to the \
                 recorded sweep numbers if the toolchain matches."
            );
        }
        super::stdin_oracle::run(
            &bin,
            &[],
            &format!("spawn {bin} — build it with tools/zig-oracle/build.sh"),
            srcroot,
            paths,
        )
    }
}
