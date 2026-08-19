use std::collections::HashMap;
use std::path::Path;

use anyhow::Result;

use crate::{stdin_oracle, LangName, Oracle};

pub struct Bash;

impl Oracle for Bash {
    fn name(&self) -> LangName {
        LangName::Bash
    }

    /// `bash -n`: bash's own parser and nothing else. It reads the script,
    /// builds the command list and stops before executing a single word, so
    /// a file is judged on its own text with no side effects.
    ///
    /// The version matters and is recorded in ledger.toml, because bash's
    /// grammar has grown: `${x@Q}`, `${x@a}` and the `[[ ... ]]` regex
    /// operator are all newer than scripts still in the corpus.
    fn validate(&self, srcroot: &Path, paths: &[String]) -> Result<HashMap<String, bool>> {
        stdin_oracle::run(
            "bash",
            &[crate::tool("bash-oracle/check.sh").to_string_lossy().as_ref()],
            "bash tools/bash-oracle/check.sh — is bash installed?",
            srcroot,
            paths,
        )
    }
}
