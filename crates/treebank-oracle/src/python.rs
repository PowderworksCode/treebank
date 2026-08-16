use std::collections::HashMap;
use std::path::Path;

use anyhow::Result;

use crate::{stdin_oracle, LangName, Oracle};

pub struct Python;

impl Oracle for Python {
    fn name(&self) -> LangName {
        LangName::Python
    }

    /// tools/py-oracle: CPython's own parser via `ast.parse`, which parses
    /// and stops — no import, no execution, no name resolution — so a
    /// missing dependency is not an error and each file is judged on its
    /// own. The interpreter's version is the language version and is
    /// recorded in ledger.json; see the note there.
    fn validate(&self, srcroot: &Path, paths: &[String]) -> Result<HashMap<String, bool>> {
        stdin_oracle::run(
            "python3",
            &[crate::tool("py-oracle/check.py").to_string_lossy().as_ref()],
            "python3 tools/py-oracle/check.py — is python3 installed?",
            srcroot,
            paths,
        )
    }
}
