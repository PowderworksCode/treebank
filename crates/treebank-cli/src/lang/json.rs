use std::collections::HashMap;
use std::path::Path;

use anyhow::Result;

use super::{npm, stdin_oracle, Lang};
use crate::ledger::LangName;
use crate::rank::RankedCrate;

pub struct Json;

impl Lang for Json {
    fn name(&self) -> LangName {
        LangName::Json
    }

    /// JSON is a **guest** language: it owns no registry and no package, it
    /// only ever rides inside other people's. npm is where it rides most —
    /// every published package carries at least one `.json` by construction
    /// — and the ranking is already implemented for javascript and
    /// typescript, so the corpus costs nothing new. The consequence is a
    /// monoculture and it is measured rather than glossed: see the ledger's
    /// `corpus.monoculture`.
    fn rank(&self, _db: &Path, k: usize) -> Result<Vec<RankedCrate>> {
        npm::rank(k)
    }

    fn resolve(&self, pkg: &RankedCrate) -> Result<(String, String)> {
        npm::resolve(pkg)
    }

    /// `.json` — the single extension tree-sitter-json's tree-sitter.json
    /// claims, following the same rule as python, lua and javascript.
    ///
    /// `.jsonc`, `.json5` and `.geojson` are deliberately NOT taken. The
    /// first two are other languages with their own grammars (Helix routes
    /// jsonc at this grammar and json5 at `Joakker/tree-sitter-json5`), and
    /// claiming an extension this grammar does not advertise is the silent
    /// widening the other languages refuse.
    ///
    /// Files *named* `tsconfig.json` or sitting under `.vscode/` are JSONC
    /// in practice and are kept anyway, which is the interesting decision
    /// here. They are 4.9% of the corpus and 100% of its strict-JSON
    /// rejects (measured: 70 of 1,426 files over the top 800 npm packages,
    /// all of them tsconfig or `.vscode/launch.json`). Dropping them would
    /// flatter the noise column by hiding files this grammar parses
    /// perfectly well — it has `comment` in `extras` — so they stay, and
    /// they are where a real JSONC dialect would attach later: a
    /// `Some(Some("jsonc"))` here plus a JSONC oracle, without touching
    /// `grammar_dirs`, because tree-sitter-json already is the JSONC
    /// grammar.
    fn classify(&self, rel: &Path) -> Option<Option<String>> {
        // A published package that ships its own `node_modules/` is shipping
        // other packages' files, and a failure there is attributed to the
        // wrong package — the same reason javascript drops bundles and
        // python drops `_vendor/` trees. Measured: 294 of 5,951 files
        // (4.9%), from exactly two packages, npm (209) and pnpm (85), whose
        // dependencies are overwhelmingly in the corpus already under their
        // own names. It is not a rounding error either: the first sweep's
        // two gap files were one jsonparse fixture counted twice, once as
        // itself and once as npm's copy of it.
        if rel.components().any(|c| c.as_os_str() == "node_modules") {
            return None;
        }
        (rel.extension()?.to_str()? == "json").then_some(None)
    }

    fn grammar_dirs(&self) -> &'static [&'static str] {
        &["."]
    }

    /// tools/json-oracle: V8's `JSON.parse`, batched through one node
    /// process. JSON has no imports, no configuration and no project
    /// context, so there is nothing to disable and nothing to be missing —
    /// this is the one language where "parse-only, no project context" is
    /// not a property that had to be arranged.
    ///
    /// V8 rather than `serde_json`, which would have been free (the CLI
    /// already depends on it) and 3x faster in-process, and which is not a
    /// conformant reference: it rejects nesting past depth 127 and rejects
    /// lone-surrogate escapes RFC 8259 permits. Over-strict is the
    /// dangerous direction — a valid file called invalid is booked as noise
    /// and a real gap vanishes — so the cheap oracle is the wrong one. The
    /// ledger's `oracle_not_serde_json` has the measurements.
    ///
    /// The script self-tests V8's acceptance boundary before it emits any
    /// verdict and exits 3 if the engine disagrees with the boundary these
    /// numbers assume; a node version alone would only say which binary is
    /// on PATH.
    fn validate(&self, srcroot: &Path, paths: &[String]) -> Result<HashMap<String, bool>> {
        stdin_oracle::run(
            "node",
            &["tools/json-oracle/check.mjs"],
            "node tools/json-oracle/check.mjs — is node installed?",
            srcroot,
            paths,
        )
    }
}
