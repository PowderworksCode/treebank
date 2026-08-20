use std::collections::HashMap;
use std::path::Path;

use anyhow::Result;

use crate::{stdin_oracle, LangName, Oracle};

pub struct Python;

impl Oracle for Python {
    fn name(&self) -> LangName {
        LangName::Python
    }

    /// CPython 3, and only CPython 3. `compile(src, path, 'exec')` — parse
    /// and post-parse SyntaxErrors, no import, no execution — so each file
    /// is judged on its own text. The interpreter version is the language
    /// version and is recorded in ledger.toml.
    ///
    /// This used to be a UNION oracle: python3 first, then python2.7 for
    /// whatever python3 rejected, a file counting as valid if either
    /// accepted it. That was right for a union grammar and is wrong for a
    /// variant. With the tables split, `python3` is judged by python 3
    /// alone, and a py2-only file is simply invalid here — which is the
    /// verdict that makes the grammar's rejection of it correct rather
    /// than merely tolerated. Python 2 has its own oracle now, on the
    /// python2 variant, in python2.rs.
    fn validate(&self, srcroot: &Path, paths: &[String]) -> Result<HashMap<String, bool>> {
        stdin_oracle::persistent(
            "py3",
            "python3",
            &[crate::tool("py-oracle/check.py").to_string_lossy().as_ref()],
            "python3 tools/py-oracle/check.py — is python3 installed?",
            srcroot,
            paths,
        )
    }

    /// `ast.parse` alone: the parser without the checks CPython runs after
    /// it. `validate` uses `compile`, deliberately and for reasons its
    /// script's header sets out; the gap between the two is what this
    /// measures.
    fn validate_syntax_only(
        &self,
        srcroot: &Path,
        paths: &[String],
    ) -> Result<Option<HashMap<String, bool>>> {
        Ok(Some(stdin_oracle::persistent(
            "py3-syntax",
            "python3",
            &[crate::tool("py-oracle/syntax.py")
                .to_string_lossy()
                .as_ref()],
            "python3 tools/py-oracle/syntax.py — is python3 installed?",
            srcroot,
            paths,
        )?))
    }

    /// CPython 3 alone — kept as its own entry point even though
    /// `validate` is now also py3-only, because callers ask this one for
    /// the CURRENT language specifically and that meaning should not
    /// depend on what `validate` happens to be today.
    fn validate_current(&self, srcroot: &Path, paths: &[String]) -> Result<HashMap<String, bool>> {
        stdin_oracle::persistent(
            "py3",
            "python3",
            &[crate::tool("py-oracle/check.py").to_string_lossy().as_ref()],
            "python3 tools/py-oracle/check.py — is python3 installed?",
            srcroot,
            paths,
        )
    }
}
