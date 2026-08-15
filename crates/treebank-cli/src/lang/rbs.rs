use std::collections::HashMap;
use std::path::Path;

use anyhow::{Context, Result};

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
/// The community signature collection, ranked alongside the gems.
const COLLECTION: &str = "ruby/gem_rbs_collection";

pub struct Rbs;

impl Lang for Rbs {
    fn name(&self) -> LangName {
        LangName::Rbs
    }

    /// The gem ranking, with `ruby/gem_rbs_collection` in front of it.
    ///
    /// RBS signatures live in two places and a corpus of only one is biased.
    /// A gem either ships its own `sig/` — 240 of the top 1000 do — or it does
    /// not, and for the ones that do not the community maintains signatures
    /// centrally in gem_rbs_collection. Those are the populations, and they do
    /// not overlap much: the collection covers Rails and its neighbours, which
    /// ship no signatures of their own, while the gems that self-host skew
    /// toward tooling. Taking only the first left `rbs` itself and
    /// language_server-protocol at 32% of all files.
    ///
    /// It is ranked 0 with 0 downloads because it has none — it is a
    /// repository, not a release. `RankedCrate.downloads` is a traffic metric
    /// everywhere else in this repo and inventing one here would be worse than
    /// an obvious zero.
    fn rank(&self, db: &Path, k: usize) -> Result<Vec<RankedCrate>> {
        let mut ranked = vec![RankedCrate {
            rank: 0,
            name: COLLECTION.to_string(),
            version: String::new(),
            downloads: 0,
        }];
        ranked.extend(Ruby.rank(db, k)?);
        Ok(ranked)
    }

    /// Gems resolve through RubyGems; the collection resolves to a GitHub
    /// source tarball at its current head, with the sha standing in for a
    /// version — the same arrangement `lang::github` uses, and for the same
    /// reason: a repository has no releases, so "version" here means what
    /// HEAD was when we fetched.
    fn resolve(&self, pkg: &RankedCrate) -> Result<(String, String)> {
        if pkg.name != COLLECTION {
            return Ruby.resolve(pkg);
        }
        let url = format!("https://api.github.com/repos/{COLLECTION}/commits/HEAD");
        let doc: serde_json::Value = ureq::get(&url)
            .set("User-Agent", "treebank")
            .call()
            .with_context(|| format!("GET {url}"))?
            .into_json()?;
        let sha = doc["sha"]
            .as_str()
            .with_context(|| format!("{COLLECTION}: no head sha"))?;
        Ok((
            sha[..12.min(sha.len())].to_string(),
            format!("https://codeload.github.com/{COLLECTION}/tar.gz/{sha}"),
        ))
    }

    fn nested_archives(&self) -> bool {
        Ruby.nested_archives()
    }

    fn nested_archive_member(&self, rel: &Path) -> bool {
        Ruby.nested_archive_member(rel)
    }

    /// Two archive shapes, told apart by whether the entry has a directory
    /// component at all.
    ///
    /// A `.gem`'s outer tar is FLAT — `metadata.gz`, `checksums.yaml.gz`,
    /// `data.tar.gz`, one component each — and stripping one would leave
    /// nothing. A GitHub source tarball wraps everything in a single
    /// `<repo>-<sha>/`, which must go. Nothing else reaches here: the inner
    /// `data.tar.gz` keeps its own root through the nested walker.
    fn archive_strip(&self, entry: &Path, _is_zip: bool) -> usize {
        usize::from(entry.components().count() > 1)
    }

    /// `.rbs` only — the single extension tree-sitter-rbs's tree-sitter.json
    /// claims, and unambiguous: nothing else uses it.
    ///
    /// `vendor/` is excluded for the reason ruby and python exclude their
    /// vendored trees: a gem that vendors a dependency ships someone else's
    /// signatures, so a failure there is attributed to the wrong package.
    fn classify(&self, rel: &Path) -> Option<Option<String>> {
        // The collection ships its own test fixtures and tooling signatures
        // alongside the gem ones; only `gems/<name>/<version>/` is signature
        // data for a real gem.
        if rel.starts_with("test") || rel.starts_with("bin") {
            return None;
        }
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
