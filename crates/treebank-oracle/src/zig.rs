use std::collections::HashMap;
use std::path::Path;

use anyhow::Result;

use crate::{stdin_oracle, LangName, Oracle};

pub struct Zig;

impl Oracle for Zig {
    fn name(&self) -> LangName {
        LangName::Zig
    }

    /// `zig fmt --stdin`, which is the compiler's own tokenizer and parser
    /// and nothing after them: it builds a `std.zig.Ast` and renders it
    /// back, so it fails exactly when the file does not parse. It reads no
    /// import, resolves no declaration and needs no build.zig — a file is
    /// judged on its own text.
    ///
    /// `zig ast-check` is the obvious candidate and is deliberately NOT
    /// used, for the reason the module header gives: an `invalid` verdict
    /// books a file our grammar failed as corpus noise, so an oracle that
    /// says `invalid` too readily reports a flawless grammar. `ast-check`
    /// runs AstGen as well as the parser and rejects files that parse
    /// perfectly well — an unused local, a pointless discard — and every
    /// one of those would excuse a real gap. The stricter tool is the
    /// dangerous one here, which is the opposite of the intuition.
    fn validate(&self, srcroot: &Path, paths: &[String]) -> Result<HashMap<String, bool>> {
        stdin_oracle::run(
            "bash",
            &[crate::tool("zig-oracle/check.sh")
                .to_string_lossy()
                .as_ref()],
            "bash tools/zig-oracle/check.sh — is zig installed?",
            srcroot,
            paths,
        )
    }
}
