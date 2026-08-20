use std::collections::HashMap;
use std::path::Path;

use anyhow::Result;

use crate::{stdin_oracle, LangName, Oracle};

pub struct Python2;

impl Oracle for Python2 {
    fn name(&self) -> LangName {
        LangName::Python2
    }

    /// CPython 2.7's own parser, reached from python3 through
    /// `typed_ast.ast27` — see tools/py-oracle/check27.py for why that is
    /// the same parser rather than an approximation of one, and for the two
    /// ways its verdicts differ from check.py's (it parses rather than
    /// compiles, and it has to neutralise PEP 263 itself).
    ///
    /// One oracle, not a fallback chain. The union grammar's oracle asked
    /// "does ANY version family accept this", because one table had to
    /// serve both; with the variants split, each table is judged by its own
    /// language and a py3-only file is simply invalid python 2.
    fn validate(&self, srcroot: &Path, paths: &[String]) -> Result<HashMap<String, bool>> {
        stdin_oracle::persistent(
            "py27",
            "python3",
            &[crate::tool("py-oracle/check27.py")
                .to_string_lossy()
                .as_ref()],
            "python3 tools/py-oracle/check27.py — needs the pinned typed_ast \
             (pip install typed_ast==1.5.5), which carries CPython 2.7's parser",
            srcroot,
            paths,
        )
    }
}
