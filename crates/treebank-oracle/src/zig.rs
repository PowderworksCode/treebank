use std::collections::HashMap;
use std::path::Path;

use anyhow::Result;

use crate::{stdin_oracle, LangName, Oracle};

pub struct Zig;

/// The two version families the grammar claims, as they are named on PATH.
/// `zig` is the current release and `zig-0.11` the oldest supported one,
/// the same arrangement as python's `python3` / `python2`.
const CURRENT: &str = "zig";
const LEGACY: &str = "zig-0.11";

fn judge(
    program: &str,
    hint: &str,
    srcroot: &Path,
    paths: &[String],
) -> Result<HashMap<String, bool>> {
    stdin_oracle::run(
        "bash",
        &[
            crate::tool("zig-oracle/check.sh")
                .to_string_lossy()
                .as_ref(),
            program,
        ],
        hint,
        srcroot,
        paths,
    )
}

impl Oracle for Zig {
    fn name(&self) -> LangName {
        LangName::Zig
    }

    /// The union oracle for a union grammar (notes/DESIGN.md §4.3): a file is
    /// valid Zig if ANY version family accepts it. The current release
    /// judges every file, and whatever it rejects is re-judged by 0.11.
    ///
    /// The second family is REQUIRED, not a nicety, and Zig needs it more
    /// than any other language here. It is pre-1.0 and it REMOVES syntax:
    /// `usingnamespace` was deleted in 0.15, `async`/`await`/`suspend` have
    /// not compiled since 0.11, and the pre-0.12 `for` loop is gone. A
    /// sweep run with only the current oracle books every file using one of
    /// those as noise, which is precisely the half of the version union
    /// that would then go unmeasured.
    ///
    /// Both halves are `zig fmt --stdin`: the compiler's own tokenizer and
    /// parser, building a `std.zig.Ast` and rendering it back, so it fails
    /// exactly when the file does not parse. It follows no `@import` and
    /// needs no build.zig, so a file is judged on its own text.
    ///
    /// `zig ast-check` is the obvious candidate and is deliberately NOT
    /// used. An `invalid` verdict books a file the grammar failed as corpus
    /// NOISE, so the readier an oracle is to say it, the more flawless the
    /// grammar looks — and `ast-check` runs AstGen as well as the parser,
    /// rejecting files that parse perfectly well (an unused local, a
    /// discard of something already void). Every one of those would excuse
    /// a real gap. Here the stricter tool is the dangerous one, which is
    /// the opposite of the intuition.
    fn validate(&self, srcroot: &Path, paths: &[String]) -> Result<HashMap<String, bool>> {
        let mut verdicts = judge(
            CURRENT,
            "bash tools/zig-oracle/check.sh zig — is zig installed?",
            srcroot,
            paths,
        )?;

        let rejected: Vec<String> = paths
            .iter()
            .filter(|p| verdicts.get(*p).copied() == Some(false))
            .cloned()
            .collect();
        if rejected.is_empty() {
            return Ok(verdicts);
        }

        let legacy = judge(
            LEGACY,
            "bash tools/zig-oracle/check.sh zig-0.11 — zig 0.11 is REQUIRED \
             for the union oracle; Zig REMOVES syntax between releases, so a \
             current-only sweep books every `usingnamespace` and every \
             `async` file as noise",
            srcroot,
            &rejected,
        )?;
        for (path, valid) in legacy {
            if valid {
                verdicts.insert(path, true);
            }
        }
        Ok(verdicts)
    }

    /// The current release alone. `validate` above is the union of it and
    /// 0.11, and the difference between the two is exactly the set of files
    /// that use syntax Zig has since removed — which is what lets a
    /// declared version-policy rejection be told apart from a real gap.
    fn validate_current(&self, srcroot: &Path, paths: &[String]) -> Result<HashMap<String, bool>> {
        judge(
            CURRENT,
            "bash tools/zig-oracle/check.sh zig — is zig installed?",
            srcroot,
            paths,
        )
    }
}
