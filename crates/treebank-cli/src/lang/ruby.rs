use std::collections::HashMap;
use std::path::Path;

use anyhow::{bail, Context, Result};

use super::Lang;
use crate::ledger::LangName;
use crate::rank::RankedCrate;

pub struct Ruby;

impl Lang for Ruby {
    fn name(&self) -> LangName {
        LangName::Ruby
    }

    fn rank(&self, _db: &Path, k: usize) -> Result<Vec<RankedCrate>> {
        rank_rubygems(k)
    }

    /// RubyGems' own API for the current release, then that version's
    /// `.gem`. The **`ruby` platform** is required, for the same reason
    /// python refuses to fall back to a wheel: a platform gem
    /// (`nokogiri-1.19.4-x86_64-linux-gnu.gem`) ships precompiled native
    /// extensions and a pruned tree, which is build output rather than the
    /// tree the author wrote. `gems/<name>.json` already answers with the
    /// `ruby` platform for every gem checked, so the fallback below is a
    /// guard rather than the normal path; a gem that publishes no source
    /// release at all errors here and the fetch driver skips it.
    fn resolve(&self, pkg: &RankedCrate) -> Result<(String, String)> {
        let url = format!("https://rubygems.org/api/v1/gems/{}.json", pkg.name);
        let doc: serde_json::Value = ureq::get(&url)
            .call()
            .with_context(|| format!("GET {url}"))?
            .into_json()?;
        let version = doc["version"]
            .as_str()
            .with_context(|| format!("{}: no version", pkg.name))?
            .to_string();
        if doc["platform"].as_str() == Some("ruby") {
            let uri = doc["gem_uri"]
                .as_str()
                .with_context(|| format!("{} {version}: no gem_uri", pkg.name))?;
            return Ok((version, uri.to_string()));
        }
        // The latest release is platform-specific; take the newest source
        // release instead. The versions list is newest-first.
        let url = format!("https://rubygems.org/api/v1/versions/{}.json", pkg.name);
        let all: Vec<serde_json::Value> = ureq::get(&url)
            .call()
            .with_context(|| format!("GET {url}"))?
            .into_json()?;
        let version = all
            .iter()
            .find(|v| v["platform"].as_str() == Some("ruby"))
            .and_then(|v| v["number"].as_str())
            .with_context(|| format!("{}: publishes no ruby-platform gem", pkg.name))?
            .to_string();
        let uri = format!("https://rubygems.org/gems/{}-{version}.gem", pkg.name);
        Ok((version, uri))
    }

    /// Neither layer of a `.gem` has a wrapper directory to strip: the outer
    /// tar holds `metadata.gz` / `checksums.yaml.gz` / `data.tar.gz` at its
    /// root, and the inner `data.tar.gz` starts straight at `lib/`. The
    /// default (strip one component from any non-zip tar) would eat `lib/`.
    fn archive_strip(&self, _entry: &Path, _is_zip: bool) -> usize {
        0
    }

    /// A `.gem` is a plain (uncompressed) tar holding `metadata.gz`,
    /// `checksums.yaml.gz` and `data.tar.gz`; every source file is inside
    /// that last member, so walking only the outer layer finds no Ruby at
    /// all. Same shape as LuaRocks' `.src.rock`, and it uses the same hook.
    fn nested_archives(&self) -> bool {
        true
    }

    /// `data.tar.gz` carries the whole package; `metadata.gz` and
    /// `checksums.yaml.gz` are gzipped YAML, not tars.
    fn nested_archive_member(&self, rel: &Path) -> bool {
        rel == Path::new("data.tar.gz")
    }

    /// Ruby source, by extension or by filename.
    ///
    /// `.rb` alone was the starting point, matching the single extension
    /// tree-sitter-ruby's tree-sitter.json claims. That undercounts the
    /// language badly: Ruby's build and configuration files are Ruby
    /// programs, not a config format, and this grammar parses them. Every
    /// entry below is present in the top-1000 gem corpus AND is a recognised
    /// Ruby filename convention (GitHub linguist's Ruby set), and every one
    /// was checked against CRuby before being added — see the ledger for the
    /// per-type counts and verdicts.
    ///
    /// Two neighbours are deliberately NOT here, both of which look like
    /// they belong:
    ///
    /// `.rbs` is the big one — 2,216 files in this corpus, more than every
    /// extension added here combined. RBS is Ruby's *type signature*
    /// language, a separate grammar with its own tree-sitter parser, and
    /// CRuby rejects 35 of 40 sampled files. Taking it would have added two
    /// thousand files that are not Ruby and recorded them as corpus noise,
    /// burying any real gap in the report.
    ///
    /// `Pluginfile` appears once and parses, but one gem's choice of name is
    /// not a convention; it is in no editor's Ruby list, and admitting names
    /// on a single sighting is how a corpus acquires files nobody can
    /// attribute.
    ///
    /// `vendor/` is excluded for the reason python excludes `_vendor/` and
    /// javascript excludes bundles: gems that vendor a dependency ship
    /// someone else's source, so a failure there is attributed to the wrong
    /// package and the same code is already in the corpus under its real
    /// owner.
    fn classify(&self, rel: &Path) -> Option<Option<String>> {
        /// Extensions whose files are Ruby programs.
        const EXTENSIONS: &[&str] = &[
            "rb",       // the language's own
            "rake",     // rake tasks
            "gemspec",  // a gem's manifest, evaluated as Ruby
            "ru",       // rackup: config.ru
            "gemfile",  // appraisal's per-matrix gemfiles, e.g. rails_7.gemfile
            "rbi",      // sorbet type stubs — Ruby syntax, unlike .rbs
            "eye",      // eye process-monitor configs
            "podspec",  // cocoapods
            "god",      // god process-monitor configs
            "jbuilder", // jbuilder views
        ];
        /// Extensionless files that are Ruby by convention.
        const FILENAMES: &[&str] = &[
            "Rakefile",
            "rakefile",
            "Gemfile",
            "Guardfile",
            "Appraisals",
            "Steepfile",
            "Dangerfile",
            "Capfile",
            "Brewfile",
            "Vagrantfile",
            "Thorfile",
            "Fastfile",
            "Berksfile",
            "Puppetfile",
            "Podfile",
            ".simplecov",
        ];
        if rel
            .components()
            .any(|c| c.as_os_str().to_str() == Some("vendor"))
        {
            return None;
        }
        // Filename first: `.simplecov` has no extension in the sense
        // `Path::extension` means, and `Gemfile.lock` is a lockfile rather
        // than a Gemfile, so an exact match on the whole name is the only
        // thing that admits one without the other.
        let name = rel.file_name()?.to_str()?;
        if FILENAMES.contains(&name) {
            return Some(None);
        }
        let ext = rel.extension()?.to_str()?;
        EXTENSIONS.contains(&ext).then_some(None)
    }

    fn grammar_dirs(&self) -> &'static [&'static str] {
        &["."]
    }

    /// tools/rb-oracle: CRuby's own parser via
    /// `RubyVM::AbstractSyntaxTree.parse_file`, which parses and stops — no
    /// require, no execution, no constant resolution — so a missing gem is
    /// not an error and each file is judged on its own. The interpreter's
    /// version decides what counts as valid Ruby and is recorded in
    /// ledger.json under `oracle`; see the note there.
    fn validate(&self, srcroot: &Path, paths: &[String]) -> Result<HashMap<String, bool>> {
        super::stdin_oracle::run(
            "ruby",
            &[Path::new("tools/rb-oracle/check.rb").to_string_lossy().as_ref()],
            "ruby tools/rb-oracle/check.rb — is ruby installed?",
            srcroot,
            paths,
        )
    }
}

/// RubyGems publishes real per-gem download counts, but no "top N" endpoint
/// of its own — the numbers are queryable one gem at a time. ecosyste.ms
/// indexes the registry and can sort by them, which makes this the same
/// KIND of metric as crates.io, npm and PyPI: traffic, not Java's
/// dependent-repos proxy, even though it arrives through the same
/// ecosyste.ms API that java uses.
fn rank_rubygems(k: usize) -> Result<Vec<RankedCrate>> {
    const PER_PAGE: usize = 100;
    let mut ranked = Vec::new();
    let mut page = 1;
    while ranked.len() < k {
        let url = format!(
            "https://packages.ecosyste.ms/api/v1/registries/rubygems.org/packages\
             ?sort=downloads&order=desc&per_page={PER_PAGE}&page={page}"
        );
        let batch: Vec<serde_json::Value> = ureq::get(&url)
            .call()
            .with_context(|| format!("GET {url}"))?
            .into_json()?;
        if batch.is_empty() {
            break;
        }
        eprintln!("rank: ecosyste.ms rubygems page {page} ({} gems)", batch.len());
        for entry in batch {
            let (Some(name), Some(downloads)) =
                (entry["name"].as_str(), entry["downloads"].as_u64())
            else {
                continue;
            };
            ranked.push(RankedCrate {
                rank: ranked.len() + 1,
                name: name.to_string(),
                // Resolved at fetch time from RubyGems, like java and python.
                version: String::new(),
                downloads,
            });
            if ranked.len() == k {
                break;
            }
        }
        page += 1;
    }
    if ranked.is_empty() {
        bail!("rubygems rank list came out empty");
    }
    Ok(ranked)
}
