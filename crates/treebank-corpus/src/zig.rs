use std::path::Path;

use anyhow::{bail, Context, Result};

use crate::rank::RankedCrate;
use crate::{github, Ecosystem};
use treebank_lang::LangName;

pub struct Zig;

/// Where the released source tarballs are listed. It is the same document
/// `zig fmt`'s own installer reads, and every entry carries a size the
/// fetch driver can check against.
const DOWNLOAD_INDEX: &str = "https://ziglang.org/download/index.json";

/// **Zig has a package manager and no registry.** `build.zig.zon` names a
/// dependency by URL and content hash, so there is no index to rank, no
/// download count to sort by and no server that knows which packages
/// exist. That is a fact about the ecosystem in 2026, not a gap in this
/// module: the "Zig package index" sites that exist are themselves GitHub
/// scrapes. It is why this ecosystem has two sources rather than one (see
/// [`Source`]), and why neither of them is a popularity ranking of Zig
/// packages, because no such thing is published.
///
/// The GitHub half ranks repositories by stars, with everything
/// `lang::github` says about that bias applying unchanged — attention
/// rather than use, a metric that costs nothing and never decays, and
/// `language:Zig` selecting repositories that are MOSTLY Zig.
///
/// One bias is Zig's own and worth naming separately: the language is
/// pre-1.0 and its syntax has moved inside the window the grammar claims
/// (0.11 through 0.16), in both directions. A star-ranked GitHub corpus is
/// weighted toward repositories that are maintained, so it is weighted
/// toward RECENT Zig, and it under-represents the older forms —
/// `usingnamespace`, the pre-0.12 `for` loop, `async`/`await` before they
/// were shelved — that the version-union policy (notes/DESIGN.md §4.2) still
/// requires the grammar to parse. That is the gap the `upstream` source
/// exists to close: one release per minor covers every syntax generation
/// by construction.
const GITHUB_LANGUAGE: &str = "Zig";

/// The extensions this grammar's own `tree-sitter.json` claims. `.zon` is
/// here because it is the same tokenizer and the same expression grammar —
/// a `build.zig.zon` is one anonymous initializer — and a file the grammar
/// parses is a file the sweep should be judged on.
const ZIG_EXTENSIONS: [&str; 2] = ["zig", "zon"];

/// The two artifact corpora, and they answer different questions.
///
/// `github` is "what do people write", ranked by stars, with everything
/// `lang::github` says about that bias. `upstream` is "what does the
/// language itself contain": the released SOURCE TARBALLS from
/// ziglang.org, one per version, which between them carry the standard
/// library, the self-hosted compiler and the behaviour test suite.
///
/// They are not interchangeable and the ledger must name which one a
/// number came from, so `rank` writes `db/source.json` the way bash's
/// does. `upstream` is also the only one that needs no credentials: the
/// GitHub SEARCH API is what ranks stars, and a session without access to
/// it can build the second corpus and not the first.
#[derive(Clone, Copy, PartialEq)]
enum Source {
    Github,
    Upstream,
}

fn source() -> Result<Source> {
    match std::env::var("TREEBANK_ZIG_CORPUS").as_deref() {
        Ok("upstream") => Ok(Source::Upstream),
        Ok("github") | Err(_) => Ok(Source::Github),
        Ok(other) => bail!("TREEBANK_ZIG_CORPUS={other:?}: expected \"github\" or \"upstream\""),
    }
}

/// The oldest release this corpus covers. It is the older half of the
/// union oracle (`treebank-oracle::zig`), and a file no oracle can judge
/// is not a corpus file: it would enter as an unadjudicable failure and
/// leave as noise, having measured nothing. Zig 0.10 and earlier are a
/// different enough language that judging them needs a third oracle, not
/// a wider grammar.
const OLDEST: (u64, u64, u64) = (0, 11, 0);

/// Released versions, newest first, ONE PER MINOR.
///
/// Two filters, both about what the corpus would otherwise measure twice.
/// `master` is excluded because it moves, so a corpus built from it is not
/// reproducible and the "version" in the manifest would name nothing. And
/// only the newest patch of each minor is kept: 0.14.0 and 0.14.1 ship
/// standard libraries that are nearly identical, so taking both doubles
/// the weight of that release's idioms and correlates every failure in it
/// with itself. What the version union needs measured is one release per
/// syntax generation, which is what a minor is.
fn releases(k: usize) -> Result<Vec<RankedCrate>> {
    let doc: serde_json::Value = ureq::get(DOWNLOAD_INDEX)
        .call()
        .with_context(|| format!("GET {DOWNLOAD_INDEX}"))?
        .into_json()
        .context("parse ziglang.org download index")?;
    let obj = doc
        .as_object()
        .context("download index is not a JSON object")?;

    let mut versions: Vec<String> = obj
        .iter()
        .filter(|(name, _)| name.as_str() != "master")
        // A release with no `src` tarball is one this corpus cannot use.
        .filter(|(_, entry)| entry.get("src").and_then(|s| s.get("tarball")).is_some())
        .map(|(name, _)| name.clone())
        .filter(|name| semver_key(name) >= OLDEST)
        .collect();
    versions.sort_by(|a, b| semver_key(b).cmp(&semver_key(a)));
    versions.dedup_by_key(|v| {
        let (major, minor, _) = semver_key(v);
        (major, minor)
    });
    versions.truncate(k);

    anyhow::ensure!(
        !versions.is_empty(),
        "no released zig versions in the index"
    );
    Ok(versions
        .into_iter()
        .enumerate()
        .map(|(i, version)| RankedCrate {
            rank: i + 1,
            name: "zig".to_string(),
            version,
            // There is no download count to record and inventing one would
            // put a number in the manifest that means nothing. The rank is
            // recency and says so.
            downloads: 0,
        })
        .collect())
}

/// Sortable form of a `MAJOR.MINOR.PATCH` tag. Anything unparsable sorts
/// last rather than erroring: the index is upstream's file, not ours.
fn semver_key(v: &str) -> (u64, u64, u64) {
    let mut it = v.split('.').map(|p| p.parse::<u64>().unwrap_or(0));
    (
        it.next().unwrap_or(0),
        it.next().unwrap_or(0),
        it.next().unwrap_or(0),
    )
}

impl Ecosystem for Zig {
    fn name(&self) -> LangName {
        LangName::Zig
    }

    fn rank(&self, db: &Path, k: usize) -> Result<Vec<RankedCrate>> {
        let (ranked, name, note) = match source()? {
            Source::Github => (
                github::rank(LangName::Zig, GITHUB_LANGUAGE, k)?,
                "github",
                "Zig publishes no registry: build.zig.zon names dependencies by URL \
                 and hash, so there is no index to rank. Stars are the only ordering \
                 available and the ledger records what they bias toward.",
            ),
            Source::Upstream => (
                releases(k)?,
                "upstream",
                "the released source tarballs from ziglang.org, newest first: the \
                 standard library, the self-hosted compiler and the behaviour test \
                 suite, one package per version. Ranked by recency, because there is \
                 no popularity metric over releases and inventing one would be worse \
                 than saying so.",
            ),
        };
        std::fs::create_dir_all(db)?;
        std::fs::write(
            db.join("source.json"),
            serde_json::json!({
                "source": name,
                "requested_k": k,
                "ranked": ranked.len(),
                "note": note,
            })
            .to_string(),
        )?;
        eprintln!("rank: corpus source is {name}");
        Ok(ranked)
    }

    fn resolve(&self, pkg: &RankedCrate) -> Result<(String, String)> {
        match source()? {
            Source::Github => github::resolve(LangName::Zig, pkg),
            // Deterministic from the version, which is why `rank` writes no
            // index for this source: every release is published at the same
            // path under the same name.
            Source::Upstream => Ok((
                pkg.version.clone(),
                format!(
                    "https://ziglang.org/download/{v}/zig-{v}.tar.xz",
                    v = pkg.version
                ),
            )),
        }
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
