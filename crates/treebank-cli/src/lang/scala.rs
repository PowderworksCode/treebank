use std::collections::HashMap;
use std::io::Write;
use std::path::Path;
use std::process::{Command, Stdio};

use anyhow::{bail, Context, Result};

use super::java::{tag, CENTRAL};
use super::Lang;
use crate::ledger::LangName;
use crate::rank::RankedCrate;

pub struct Scala;

/// Scala binary versions, newest first. A Maven artifact built for Scala
/// carries its compiler's binary version in the artifact id — `cats-core_3`,
/// `spark-core_2.11` — which is the whole reason this language's dialect
/// problem is solvable at all.
const BINARY_VERSIONS: [&str; 4] = ["3", "2.13", "2.12", "2.11"];

impl Lang for Scala {
    fn name(&self) -> LangName {
        LangName::Scala
    }

    fn rank(&self, _db: &Path, k: usize) -> Result<Vec<RankedCrate>> {
        rank_maven_scala(k)
    }

    /// Identical to java's: Maven Central's own metadata for the current
    /// release, then the convention-named sources jar. Artifacts that publish
    /// no sources jar 404 here and the fetch driver skips them.
    fn resolve(&self, pkg: &RankedCrate) -> Result<(String, String)> {
        let (group, artifact) = pkg
            .name
            .split_once(':')
            .with_context(|| format!("{}: not a group:artifact coordinate", pkg.name))?;
        let path = format!("{}/{}", group.replace('.', "/"), artifact);
        let version = release(&path)
            .with_context(|| format!("{}: no release version in maven-metadata.xml", pkg.name))?;
        let jar = format!("{CENTRAL}/{path}/{version}/{artifact}-{version}-sources.jar");
        Ok((version, jar))
    }

    /// `.scala` only. `tree-sitter.json` also claims `.sbt`, and `.sc`
    /// scripts exist in the wild, but neither ships inside a sources jar in
    /// any quantity, and both would arrive with a dialect nothing in the
    /// corpus can declare.
    ///
    /// The routing hint stays `None`: Scala 2 and Scala 3 are one *grammar*
    /// (tree-sitter-scala parses both) and two *dialects*, and the dialect
    /// is the oracle's business, not the grammar router's. It is also not
    /// knowable here — `classify` sees a package-relative path, and the
    /// answer lives in the Maven coordinate one level up. See `validate`.
    fn classify(&self, rel: &Path) -> Option<Option<String>> {
        (rel.extension()?.to_str()? == "scala").then_some(None)
    }

    fn grammar_dirs(&self) -> &'static [&'static str] {
        &["."]
    }

    /// tools/scala-oracle: scalameta's parser, one file per `Parse`, run
    /// through the JDK's single-file source launcher. Parse-only, so
    /// unresolved imports are not errors and a file is judged on its own.
    ///
    /// The dialect is decided HERE, from the Maven coordinate in each file's
    /// corpus directory, and passed to the oracle per file. Measured over
    /// 3,508 corpus files: the declared dialect called 0 valid files invalid,
    /// while pinning every file to one dialect called between 61 (Scala213)
    /// and 301 (Scala3) of them invalid — 1.7% to 8.6% of a real corpus
    /// misfiled as noise, each one a place a grammar gap could hide.
    fn validate(&self, srcroot: &Path, paths: &[String]) -> Result<HashMap<String, bool>> {
        let script = Path::new("tools/scala-oracle/Check.java");
        anyhow::ensure!(
            script.exists(),
            "scala oracle missing at {} (run from the repo root)",
            script.display()
        );
        let classpath = Path::new("tools/scala-oracle/classpath");
        anyhow::ensure!(
            classpath.exists(),
            "scala oracle classpath missing — run tools/scala-oracle/fetch-jars.sh"
        );
        let classpath = std::fs::read_to_string(classpath)?.trim().to_string();

        // Build every line before spawning: an undeclarable dialect must stop
        // the sweep, not reach the oracle as some other file's verdict.
        let mut lines = Vec::with_capacity(paths.len());
        for p in paths {
            let dialect = dialect_for(p)?;
            lines.push(format!("{dialect}\t{}", srcroot.join(p).display()));
        }

        let mut child = Command::new("java")
            .arg("-cp")
            .arg(&classpath)
            .arg(script)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .spawn()
            .context("spawn `java tools/scala-oracle/Check.java` — is a JDK installed?")?;

        // Feed stdin from a thread: a large batch's output would otherwise
        // fill the stdout pipe and deadlock us before we finish writing.
        let mut stdin = child.stdin.take().context("oracle stdin")?;
        let writer = std::thread::spawn(move || -> std::io::Result<()> {
            for line in &lines {
                writeln!(stdin, "{line}")?;
            }
            stdin.flush()
        });
        let output = child.wait_with_output()?;
        let _ = writer
            .join()
            .map_err(|_| anyhow::anyhow!("oracle stdin thread panicked"))?;
        // stderr is inherited, so the oracle's own diagnostics have already
        // reached the terminal; only the status is news here.
        anyhow::ensure!(
            output.status.success(),
            "scala-oracle exited with {}",
            output.status
        );

        let mut map = HashMap::new();
        for line in String::from_utf8_lossy(&output.stdout).lines() {
            // "<path>\t<verdict>\t<dialect>", split from the right so a
            // corpus path containing a tab survives.
            let mut fields = line.rsplitn(3, '\t');
            let (Some(_dialect), Some(verdict), Some(path)) =
                (fields.next(), fields.next(), fields.next())
            else {
                continue;
            };
            let rel = Path::new(path)
                .strip_prefix(srcroot)
                .map(|r| r.to_string_lossy().into_owned())
                .unwrap_or_else(|_| path.to_string());
            map.insert(rel, verdict == "valid");
        }
        // Every path must come back. A missing verdict is read as `false` by
        // the sweep — i.e. as noise — so silence here would hide gaps.
        anyhow::ensure!(
            map.len() == paths.len(),
            "scala-oracle returned {} verdicts for {} paths",
            map.len(),
            paths.len()
        );
        Ok(map)
    }
}

/// The scalameta dialect for a corpus path, from the Maven coordinate in its
/// package directory (`org.apache.spark__spark-core_2.11-2.4.8/...`).
///
/// This is the whole dialect answer, and it is deliberately total: an
/// unroutable path is an error, never a default. Guessing a dialect would
/// produce a verdict the corpus does not support, and a wrong `invalid` is
/// recorded as noise — the one direction in which this pipeline hides its own
/// bugs. `rank` only ever emits Scala-suffixed coordinates, so a path that
/// reaches here without one means the corpus and the ranking have diverged.
fn dialect_for(path: &str) -> Result<&'static str> {
    let pkgdir = path.split('/').next().unwrap_or(path);
    // Match `_<binver>-` rather than splitting on the last '-': Maven
    // versions contain hyphens (`2.0.0-M3`, `3.3.0-SNAP4`), so the version is
    // not a suffix you can cut off first.
    for bin in BINARY_VERSIONS {
        if pkgdir.contains(&format!("_{bin}-")) {
            return Ok(match bin {
                "3" => "Scala3",
                "2.13" => "Scala213",
                "2.12" => "Scala212",
                _ => "Scala211",
            });
        }
    }
    bail!(
        "cannot tell which Scala dialect {pkgdir} is: no _2.11/_2.12/_2.13/_3 in the \
         Maven coordinate. This is a routing failure, not a verdict — refusing to guess."
    )
}

/// The `<release>` version of a Maven coordinate, or `None` if the coordinate
/// does not exist. `<latest>` can be a snapshot, so `<release>` comes first.
fn release(path: &str) -> Option<String> {
    let url = format!("{CENTRAL}/{path}/maven-metadata.xml");
    let xml = ureq::get(&url).call().ok()?.into_string().ok()?;
    tag(&xml, "release").or_else(|| tag(&xml, "latest"))
}

/// Ranked Scala coordinates for the corpus.
///
/// This deviates from java's `rank_maven`, which it otherwise reuses
/// wholesale, and the deviation is measured rather than stylistic.
/// ecosyste.ms ranks Maven artifacts by how many public repositories depend
/// on them, which for Scala is a decade-lagging metric: measured 2026-08-12,
/// the top 4,000 Maven artifacts by that count contain **zero** `_3`
/// coordinates, `org.apache.spark:spark-core_2.11` scores 8,772 while
/// `org.typelevel:cats-core_3` scores 2, and the top Scala artifacts resolve
/// to Spark 2.4.8, Kafka 2.4.1 and Akka 2.5.32. Taking that list literally
/// would sweep a Scala-3-era grammar against 2016 Scala 2.11 and never once
/// exercise the dialect split this language was queued for.
///
/// So the ranking is used for *what is popular* and the coordinate for *which
/// dialect to sweep*: rank by dependent repositories, collapse the cross-built
/// duplicates of one project (`foo_2.11`, `foo_2.12`, `foo_3` are one
/// project), then emit that project's newest published Scala 2 line and its
/// Scala 3 line, so both dialects reach the corpus. Popularity is the
/// project's; the ledger says so.
fn rank_maven_scala(k: usize) -> Result<Vec<RankedCrate>> {
    const PER_PAGE: usize = 100;
    let mut projects: Vec<(String, String, u64)> = Vec::new(); // group, base artifact, dependents
    let mut seen = std::collections::HashSet::new();
    let mut page = 1;
    // Scala coordinates are sparse in the registry-wide ranking, so this
    // walks further than java's does to find k of them.
    while projects.len() < k && page <= 60 {
        let url = format!(
            "https://packages.ecosyste.ms/api/v1/registries/repo1.maven.org/packages\
             ?sort=dependent_repos_count&order=desc&per_page={PER_PAGE}&page={page}"
        );
        let batch: Vec<serde_json::Value> = ureq::get(&url)
            .set("User-Agent", "treebank (https://treebank.dev)")
            .call()
            .with_context(|| format!("GET {url}"))?
            .into_json()?;
        if batch.is_empty() {
            break;
        }
        eprintln!(
            "rank: ecosyste.ms maven page {page} ({} artifacts, {} scala projects so far)",
            batch.len(),
            projects.len()
        );
        for entry in batch {
            let (Some(name), Some(dependents)) = (
                entry["name"].as_str(),
                entry["dependent_repos_count"].as_u64(),
            ) else {
                continue;
            };
            let Some((group, artifact)) = name.split_once(':') else {
                continue;
            };
            let Some(base) = scala_base(artifact) else {
                continue;
            };
            // Highest-ranked binary version of a project wins its slot; the
            // others are the same source cross-built.
            if seen.insert((group.to_string(), base.to_string())) {
                projects.push((group.to_string(), base.to_string(), dependents));
            }
            if projects.len() == k {
                break;
            }
        }
        page += 1;
    }
    if projects.is_empty() {
        bail!("maven scala rank list came out empty");
    }

    let mut ranked = Vec::new();
    for (group, base, dependents) in projects {
        let path = format!("{}/{base}", group.replace('.', "/"));
        // Newest published Scala 2 line, plus Scala 3 if the project has one.
        let scala2 = ["2.13", "2.12", "2.11"]
            .into_iter()
            .find(|v| release(&format!("{path}_{v}")).is_some());
        let lines = ["3"]
            .into_iter()
            .filter(|v| release(&format!("{path}_{v}")).is_some())
            .chain(scala2);
        for v in lines {
            ranked.push(RankedCrate {
                rank: ranked.len() + 1,
                name: format!("{group}:{base}_{v}"),
                version: String::new(), // resolved at fetch time from Central
                downloads: dependents,  // the project's, shared by its lines
            });
        }
    }
    eprintln!("rank: {} scala coordinates", ranked.len());
    Ok(ranked)
}

/// The artifact id without its Scala binary-version suffix, or `None` if it
/// has none — which means the artifact is not published for Scala at all.
fn scala_base(artifact: &str) -> Option<&str> {
    BINARY_VERSIONS
        .iter()
        .find_map(|v| artifact.strip_suffix(&format!("_{v}")))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dialect_comes_from_the_coordinate() {
        // Real corpus directory names, including the two shapes that break a
        // naive "cut at the last hyphen": hyphenated artifact ids and
        // hyphenated Maven versions.
        assert_eq!(
            dialect_for("org.apache.spark__spark-core_2.11-2.4.8/org/apache/spark/A.scala").unwrap(),
            "Scala211"
        );
        assert_eq!(
            dialect_for("com.typesafe.akka__akka-stream_2.12-2.9.0-M1/akka/A.scala").unwrap(),
            "Scala212"
        );
        assert_eq!(dialect_for("org.apache.kafka__kafka_2.13-3.9.1/A.scala").unwrap(), "Scala213");
        assert_eq!(
            dialect_for("org.apache.pekko__pekko-actor_3-2.0.0-M3/pekko/A.scala").unwrap(),
            "Scala3"
        );
    }

    #[test]
    fn an_undeclarable_dialect_is_an_error_not_a_guess() {
        let e = dialect_for("com.google.guava__guava-33.0.0/com/google/A.scala").unwrap_err();
        assert!(e.to_string().contains("refusing to guess"), "{e}");
    }

    #[test]
    fn cross_built_lines_collapse_to_one_project() {
        assert_eq!(scala_base("cats-core_3"), Some("cats-core"));
        assert_eq!(scala_base("spark-core_2.11"), Some("spark-core"));
        assert_eq!(scala_base("guava"), None);
        // `_2` is not a Scala binary version, and `_2.10` is older than
        // anything scalameta will be asked about here.
        assert_eq!(scala_base("commons-lang_2"), None);
    }
}
