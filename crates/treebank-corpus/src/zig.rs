use std::path::Path;

use anyhow::Result;

use crate::rank::RankedCrate;
use crate::{github, Ecosystem};
use treebank_lang::LangName;

pub struct Zig;

/// **Zig has a package manager and no registry.** `build.zig.zon` names a
/// dependency by URL and content hash, so there is no index to rank, no
/// download count to sort by and no server that knows which packages exist.
/// That is a fact about the ecosystem in 2026, not a gap in this module:
/// the "Zig package index" sites that exist are themselves GitHub scrapes.
///
/// So the corpus is GitHub, ranked by stars, with everything `lang::github`
/// says about that bias applying unchanged — attention rather than use, a
/// metric that costs nothing and never decays, and `language:Zig` selecting
/// repositories that are MOSTLY Zig.
///
/// One bias is Zig's own and worth naming separately: the language is
/// pre-1.0 and its syntax has moved inside the window this grammar claims
/// (0.11 through 0.15). A star-ranked GitHub corpus is weighted toward
/// repositories that are maintained, so it is weighted toward RECENT Zig,
/// and it will under-represent the older forms — `usingnamespace`, the
/// pre-0.12 `for` loop, `async`/`await` before they were shelved — that the
/// version-union policy (DESIGN.md §4.2) still requires the grammar to
/// parse. A sweep pass rate is therefore evidence about current Zig first,
/// and the ledger says so rather than letting the number stand for both.
const GITHUB_LANGUAGE: &str = "Zig";

/// The extensions this grammar's own `tree-sitter.json` claims. `.zon` is
/// here because it is the same tokenizer and the same expression grammar —
/// a `build.zig.zon` is one anonymous initializer — and a file the grammar
/// parses is a file the sweep should be judged on.
const ZIG_EXTENSIONS: [&str; 2] = ["zig", "zon"];

impl Ecosystem for Zig {
    fn name(&self) -> LangName {
        LangName::Zig
    }

    fn rank(&self, db: &Path, k: usize) -> Result<Vec<RankedCrate>> {
        let ranked = github::rank(LangName::Zig, GITHUB_LANGUAGE, k)?;
        std::fs::create_dir_all(db)?;
        std::fs::write(
            db.join("source.json"),
            serde_json::json!({
                "source": "github",
                "requested_k": k,
                "ranked": ranked.len(),
                "note": "Zig publishes no registry: build.zig.zon names dependencies \
                         by URL and hash, so there is no index to rank. Stars are the \
                         only ordering available and the ledger records what they bias \
                         toward.",
            })
            .to_string(),
        )?;
        Ok(ranked)
    }

    fn resolve(&self, pkg: &RankedCrate) -> Result<(String, String)> {
        github::resolve(LangName::Zig, pkg)
    }

    fn classify(&self, rel: &Path) -> Option<Option<String>> {
        let ext = rel.extension()?.to_str()?;
        ZIG_EXTENSIONS.contains(&ext).then_some(None)
    }

    /// A NUL means the file is not source. Zig has no polyglot problem and
    /// no template dialect in wide use, so there is nothing else to filter:
    /// the extension settles what a `.zig` file is, which is the opposite
    /// of bash's situation and the reason this method is three lines.
    fn admit(&self, rel: &Path, content: &[u8]) -> bool {
        let _ = rel;
        !content.contains(&0)
    }
}
