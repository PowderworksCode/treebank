use std::collections::HashMap;
use std::path::Path;

use anyhow::Result;

use crate::{stdin_oracle, LangName, Oracle};

pub struct Ruby;

impl Oracle for Ruby {
    fn name(&self) -> LangName {
        LangName::Ruby
    }

    /// tools/rb-oracle: CRuby's own parser via
    /// `RubyVM::AbstractSyntaxTree.parse_file`, which runs the same parser
    /// `ruby` runs and stops at the AST — no require, no execution, no
    /// constant resolution — so a missing gem is not an error and each file
    /// is judged on its own text. The interpreter's version decides what
    /// counts as valid Ruby and is recorded in ledger.toml under `oracles`;
    /// `it` blocks are 3.4+, `{x:}` shorthand is 3.1+, endless methods 3.0+.
    fn validate(&self, srcroot: &Path, paths: &[String]) -> Result<HashMap<String, bool>> {
        stdin_oracle::run(
            "ruby",
            &[crate::tool("rb-oracle/check.rb").to_string_lossy().as_ref()],
            "ruby tools/rb-oracle/check.rb — is ruby installed?",
            srcroot,
            paths,
        )
    }
}
