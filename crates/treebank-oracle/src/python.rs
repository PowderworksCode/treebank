use std::collections::HashMap;
use std::path::Path;

use anyhow::Result;

use crate::{stdin_oracle, LangName, Oracle};

pub struct Python;

impl Oracle for Python {
    fn name(&self) -> LangName {
        LangName::Python
    }

    /// The union oracle for a union grammar (DESIGN.md §4.3): a file is
    /// valid python if ANY version family accepts it. CPython 3 judges
    /// every file first via tools/py-oracle/check.py; whatever it rejects
    /// is re-judged by CPython 2.7 via check2.py. Both go through
    /// `compile(src, path, 'exec')` — parse and post-parse SyntaxErrors,
    /// no import, no execution — so each file is judged on its own text.
    /// The interpreter versions are the language versions and are recorded
    /// in ledger.toml.
    ///
    /// python2 is REQUIRED, not optional: a union grammar swept with only
    /// the py3 oracle books every py2-only file as noise, which silently
    /// hides the py2 half's gaps. If `python2` is not on PATH this errors
    /// loudly rather than degrading.
    fn validate(&self, srcroot: &Path, paths: &[String]) -> Result<HashMap<String, bool>> {
        let mut verdicts = stdin_oracle::persistent(
            "py3",
            "python3",
            &[crate::tool("py-oracle/check.py").to_string_lossy().as_ref()],
            "python3 tools/py-oracle/check.py — is python3 installed?",
            srcroot,
            paths,
        )?;

        let py3_rejected: Vec<String> = paths
            .iter()
            .filter(|p| verdicts.get(*p).copied() == Some(false))
            .cloned()
            .collect();
        if py3_rejected.is_empty() {
            return Ok(verdicts);
        }

        let py2 = stdin_oracle::persistent(
            "py2",
            "python2",
            &[crate::tool("py-oracle/check2.py").to_string_lossy().as_ref()],
            "python2 tools/py-oracle/check2.py — python2 (2.7) is REQUIRED \
             for the union oracle; a py3-only sweep would book every \
             py2-only file as noise",
            srcroot,
            &py3_rejected,
        )?;
        for (path, valid) in py2 {
            if valid {
                verdicts.insert(path, true);
            }
        }
        Ok(verdicts)
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
            &[crate::tool("py-oracle/syntax.py").to_string_lossy().as_ref()],
            "python3 tools/py-oracle/syntax.py — is python3 installed?",
            srcroot,
            paths,
        )?))
    }

    /// CPython 3 alone — the current language. `validate` above is the union
    /// of py3 and py2.7; this is the py3 half, and the difference between
    /// the two is exactly the set of py2-only files.
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
