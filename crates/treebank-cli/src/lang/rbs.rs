use std::collections::HashMap;
use std::path::Path;

use anyhow::Result;

use super::{ruby::Ruby, Lang};
use crate::ledger::LangName;
use crate::rank::RankedCrate;

/// RBS — Ruby's type signature language.
///
/// A separate language from Ruby, not a dialect of it: `def foo: (Integer) ->
/// String` is RBS and CRuby rejects it, `def foo(a) = a + 1` is Ruby and the
/// RBS parser rejects it. It has its own grammar, its own reference parser
/// (`RBS::Parser`) and its own file extension, which is why it is a grammar
/// here rather than an extension admitted into ruby's `classify`. Measured
/// while deciding that: of 40 sampled `.rbs` files, CRuby rejected 35.
///
/// Everything about *getting* the corpus is ruby's, though, and is delegated
/// rather than copied — RBS signatures ship inside gems, so the registry, the
/// download ranking and the nested `.gem` archive handling are the same
/// problem with the same answer. Only `classify` and `validate` differ, which
/// is the whole of what makes this a different language.
pub struct Rbs;

impl Lang for Rbs {
    fn name(&self) -> LangName {
        LangName::Rbs
    }

    fn rank(&self, db: &Path, k: usize) -> Result<Vec<RankedCrate>> {
        Ruby.rank(db, k)
    }

    fn resolve(&self, pkg: &RankedCrate) -> Result<(String, String)> {
        Ruby.resolve(pkg)
    }

    fn nested_archives(&self) -> bool {
        Ruby.nested_archives()
    }

    fn nested_archive_member(&self, rel: &Path) -> bool {
        Ruby.nested_archive_member(rel)
    }

    fn archive_strip(&self, entry: &Path, is_zip: bool) -> usize {
        Ruby.archive_strip(entry, is_zip)
    }

    /// `.rbs` only — the single extension tree-sitter-rbs's tree-sitter.json
    /// claims, and unambiguous: nothing else uses it.
    ///
    /// `vendor/` is excluded for the reason ruby and python exclude their
    /// vendored trees: a gem that vendors a dependency ships someone else's
    /// signatures, so a failure there is attributed to the wrong package.
    fn classify(&self, rel: &Path) -> Option<Option<String>> {
        if rel
            .components()
            .any(|c| c.as_os_str().to_str() == Some("vendor"))
        {
            return None;
        }
        (rel.extension()?.to_str()? == "rbs").then_some(None)
    }

    fn grammar_dirs(&self) -> &'static [&'static str] {
        &["."]
    }

    /// tools/rbs-oracle: RBS's own parser via `RBS::Parser.parse_signature`,
    /// which parses and stops — no type checking, no constant resolution, and
    /// it never loads the signatures a file references, so a signature naming
    /// a class it cannot see is not an error. The rbs version decides what
    /// counts as valid and is recorded in ledger.json; see the note there,
    /// because for this language the version is unusually load-bearing.
    fn validate(&self, srcroot: &Path, paths: &[String]) -> Result<HashMap<String, bool>> {
        super::stdin_oracle::run(
            "ruby",
            &[Path::new("tools/rbs-oracle/check.rb").to_string_lossy().as_ref()],
            "ruby tools/rbs-oracle/check.rb — is ruby installed, with the rbs gem >= 4.0?",
            srcroot,
            paths,
        )
    }
}
