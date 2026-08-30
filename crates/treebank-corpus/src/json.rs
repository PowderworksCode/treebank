use std::path::Path;

use anyhow::{bail, Result};

use crate::rank::RankedCrate;
use crate::{github, npm, Ecosystem};
use treebank_lang::LangName;

pub struct Json;

/// **JSON has no ecosystem of its own, and that is the fact this module is
/// organised around.** There is no registry of JSON documents, no download
/// count for one, no version on one. JSON is never the artifact: it is
/// carried inside somebody else's artifact — a package's manifest, a
/// library's locale tables, a test fixture, a schema — so the corpus
/// question for this language is not "which packages" but "whose packages
/// do you open".
///
/// So there are two sources, and they answer different questions.
///
/// `npm` is the default and is the parasitic reading: JSON as it occurs
/// inside the ecosystem where it is load-bearing. It inherits npm's
/// download ranking, which is the strongest popularity metric available to
/// any corpus in this repository — real installs, not stars.
///
/// `github` is the other reading: the repositories GitHub's own linguist
/// classifies as majority-JSON, which are the ones where JSON IS the
/// artifact — `caniuse`, `mdn/browser-compat-data`, OpenStreetMap's tag
/// catalogues, JSON-schema collections, geodata. Ranked by stars, with
/// everything `lang::github` says about that metric applying unchanged.
///
/// One number is worth writing down because it is evidence rather than
/// commentary: that search returns about **2,000 repositories in all of
/// GitHub**. Every other language here has that many in its first page of
/// results. A population that small is not a sampling problem, it is the
/// same fact stated from the other end — almost nowhere is JSON the thing
/// being written, it is the thing being written INTO.
#[derive(Clone, Copy, PartialEq)]
enum Source {
    Npm,
    Github,
}

/// Linguist's name for the language, as the search API spells it. Linguist
/// classifies JSON as `type: data` and still counts it toward a
/// repository's majority language, which is why this search returns
/// anything at all.
const GITHUB_LANGUAGE: &str = "JSON";

fn source() -> Result<Source> {
    match std::env::var("TREEBANK_JSON_CORPUS").as_deref() {
        Ok("github") => Ok(Source::Github),
        Ok("npm") | Err(_) => Ok(Source::Npm),
        Ok(other) => bail!("TREEBANK_JSON_CORPUS={other:?}: expected \"npm\" or \"github\""),
    }
}

impl Ecosystem for Json {
    fn name(&self) -> LangName {
        LangName::Json
    }

    fn rank(&self, db: &Path, k: usize) -> Result<Vec<RankedCrate>> {
        let (ranked, name, note) = match source()? {
            Source::Npm => (
                npm::rank(k)?,
                "npm",
                "the top npm packages by downloads, opened for the .json files \
                 they ship. JSON has no registry of its own — it is never the \
                 artifact — so this ranks the HOST ecosystem and takes the JSON \
                 inside it. Downloads are real installs, which is the strongest \
                 popularity metric any corpus here uses.",
            ),
            Source::Github => (
                github::rank(LangName::Json, GITHUB_LANGUAGE, k)?,
                "github",
                "repositories GitHub classifies as majority-JSON, by stars: the \
                 places where JSON IS the artifact rather than a file inside \
                 one. About 2,000 such repositories exist in total, which is \
                 itself the measurement — and they are a different population \
                 from npm's, weighted to data catalogues, schema collections \
                 and geodata rather than package manifests.",
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
            Source::Npm => npm::resolve(pkg),
            Source::Github => github::resolve(LangName::Json, pkg),
        }
    }

    /// `.json` and nothing else, which is the dialect decision showing up
    /// in the corpus layer.
    ///
    /// `.jsonc` and `.json5` are deliberately absent because this grammar
    /// does not parse those languages — see ledger.toml. Admitting them
    /// would not widen the measurement, it would poison it: every such file
    /// would enter as a grammar failure that the oracle also rejects, be
    /// booked as noise, and quietly depress a pass rate with files nobody
    /// claimed to parse.
    ///
    /// What this DOES admit, and must, is a `.json` file that is really
    /// JSONC — `tsconfig.json` with comments in it. Those are the files the
    /// dialect decision is about, so filtering them out would be marking
    /// our own exam. They enter, they fail, the oracle agrees they are not
    /// JSON, and the ledger reports how many there were.
    fn classify(&self, rel: &Path) -> Option<Option<String>> {
        (rel.extension()?.to_str()? == "json").then_some(None)
    }

    /// A NUL means the file is not a JSON document — it is something binary
    /// wearing a `.json` name, and a corpus of those measures a file-typing
    /// bug rather than a grammar.
    ///
    /// Nothing else is filtered, and the omissions are deliberate. Machine
    /// -generated JSON is NOT excluded: a lockfile and a compiled locale
    /// table are among the most-parsed JSON documents in existence, and a
    /// corpus curated down to hand-written config would be a corpus of what
    /// this grammar finds pleasant. Minified JSON is not excluded either,
    /// because unlike minified JavaScript it inlines nobody else's code and
    /// so cannot attribute a failure to the wrong package — JSON has no
    /// bundler, and whitespace is the only thing a minifier can remove.
    fn admit(&self, rel: &Path, content: &[u8]) -> bool {
        let _ = rel;
        !content.contains(&0)
    }

    /// 250 MB, matching bash's cap, and for the `github` source rather
    /// than the npm one — an npm tarball is bounded by what an author is
    /// willing to publish, and a repository is not. The repositories this
    /// ecosystem's second source ranks are precisely the ones that hold
    /// JSON as DATA: geodata dumps, compatibility tables, tag catalogues.
    /// A handful of those are hundreds of megabytes of one generated
    /// document, which would cost an hour of download to add one file's
    /// worth of syntax. Every skip is logged by the fetch driver.
    fn max_artifact_bytes(&self) -> Option<u64> {
        Some(250_000_000)
    }
}
