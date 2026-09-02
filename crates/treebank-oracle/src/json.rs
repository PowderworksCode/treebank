use std::collections::HashMap;
use std::path::Path;

use anyhow::{Context, Result};
use rayon::prelude::*;

use crate::{LangName, Oracle};

pub struct Json;

impl Oracle for Json {
    fn name(&self) -> LangName {
        LangName::Json
    }

    /// `serde_json`, in-process, pinned by this workspace's `Cargo.lock`.
    ///
    /// JSON's oracle problem is the opposite of every other language's here.
    /// Zig has one reference parser because it has one compiler; JSON has
    /// hundreds and no owner, so "the reference implementation" is not a
    /// fact to look up — it is a choice, and the only honest way to make it
    /// is to MEASURE the candidates against a suite somebody else answered.
    /// Over nst/JSONTestSuite's determinate files (95 must-accept, 188
    /// must-reject):
    ///
    /// - serde_json 1.0.151 `from_slice::<Value>`: 95 accepted, 0 of the
    ///   must-rejects accepted.
    /// - CPython 3 `json.loads`: 95 accepted, 3 must-rejects accepted —
    ///   `NaN`, `Infinity`, `-Infinity`, which its docs call an extension.
    /// - jq 1.7: 95 accepted, **22** must-rejects accepted. Twenty are
    ///   number-literal widenings (`.2e-3`, `+1`, `-01`, `2.e3`, `NaN`,
    ///   `Inf`) and two are multi-value input (`[][]`, `{"a": true} "x"`).
    ///
    /// jq is disqualified precisely where it would hurt: the number literal
    /// and the top-level are the two places a JSON grammar is most likely to
    /// be too permissive, and jq is blind in both, so it would excuse
    /// exactly the bugs this oracle exists to catch. That is the same
    /// reasoning zig's ledger uses to refuse `zig ast-check`, pointed the
    /// other way — there the danger was an oracle too eager to say
    /// `invalid`, here it is one too eager to say `valid`, and both end in a
    /// grammar that looks flawless because nothing can contradict it.
    ///
    /// Being a library rather than a binary is the second reason. Every
    /// other oracle in this crate hopes the right version of some tool is on
    /// PATH; this one is a lockfile entry, so the oracle is pinned by
    /// construction and CI cannot silently drift onto another jq.
    ///
    /// `from_slice` and not `from_str`: several JSONTestSuite files are
    /// deliberately not valid UTF-8, and `from_str` would need the bytes
    /// decoded before the oracle saw them — which decides the verdict
    /// outside the oracle. `from_slice` validates the encoding itself and
    /// rejects, which is the right answer: RFC 8259 §8.1 requires UTF-8.
    ///
    /// An unreadable file is a hard error, never an `invalid` verdict, for
    /// the reason the crate docs give: `validate` is called only on files
    /// the grammar already failed, so `invalid` books the file as corpus
    /// NOISE, and an oracle that answers `invalid` when it cannot read
    /// turns every grammar failure into a clean sweep.
    fn validate(&self, srcroot: &Path, paths: &[String]) -> Result<HashMap<String, bool>> {
        paths
            .par_iter()
            .map(|p| {
                let full = srcroot.join(p);
                let src = std::fs::read(&full).with_context(|| {
                    format!(
                        "json oracle: cannot read {} — this is an oracle failure, \
                         not a verdict; check the corpus root",
                        full.display()
                    )
                })?;
                let valid = serde_json::from_slice::<serde_json::Value>(&src).is_ok();
                Ok((p.clone(), valid))
            })
            .collect()
    }

    /// `None`, and it is the interesting answer rather than a shrug.
    ///
    /// This hook exists for toolchains that can run their parser without the
    /// checks a compiler does afterwards. serde_json cannot, because for
    /// JSON there is nothing afterwards: the parse IS the value
    /// construction, and every check it performs beyond the grammar is one
    /// it performs while building a `Value`. That is not an accident of this
    /// crate — it is where JSON's implementation-defined zone lives. RFC
    /// 8259 leaves number range (§6), nesting depth (§9) and unpaired
    /// surrogates (§8.2) to the implementation, and all three are questions
    /// about a value, not about a token stream. So the grammar and this
    /// oracle disagree on 17 of the suite's 35 `i_` files with neither of
    /// them wrong, and no parse-only mode could close the gap because there
    /// is no parse-only mode to have. ledger.toml lists the classes.
    fn validate_syntax_only(
        &self,
        _srcroot: &Path,
        _paths: &[String],
    ) -> Result<Option<HashMap<String, bool>>> {
        Ok(None)
    }
}
