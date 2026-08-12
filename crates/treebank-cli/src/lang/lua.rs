use std::collections::HashMap;
use std::path::Path;

use anyhow::{bail, Context, Result};

use super::{stdin_oracle, Lang};
use crate::ledger::LangName;
use crate::rank::RankedCrate;

pub struct Lua;

/// The interpreter the oracle must run under. Deliberately the *versioned*
/// binary and not `lua`: on this machine `lua` happens to be 5.4, on the next
/// one it is whatever the distro symlinked, and for this language that is not
/// a detail — 5.1, 5.2, 5.3, 5.4, LuaJIT and Luau are different syntaxes, so
/// the interpreter on PATH would silently decide what "invalid" means. The
/// ledger's `oracle` field pins the same version this constant names, and
/// `check.lua` refuses to run under anything else.
const LUA: &str = "lua5.4";

impl Lang for Lua {
    fn name(&self) -> LangName {
        LangName::Lua
    }

    fn rank(&self, _db: &Path, k: usize) -> Result<Vec<RankedCrate>> {
        rank_luarocks(k)
    }

    /// LuaRocks module pages list versions newest-first, so the first
    /// non-development row is the current release. Development rows
    /// (`scm-1`, `dev-1`, carrying `development_flag`) are skipped: they
    /// track a VCS branch rather than a release, so what they contain
    /// depends on when you asked, which is not a corpus you can re-measure.
    ///
    /// The tarball is the `.src.rock` — a zip carrying the rockspec plus the
    /// author's actual source tree. `fetch::extract` sniffs the magic bytes
    /// and takes the zip path, whose entries are already root-relative, which
    /// is right here: a `.src.rock` has the rockspec at top level *beside*
    /// the source directory, so stripping a leading component would throw
    /// away the source.
    fn resolve(&self, pkg: &RankedCrate) -> Result<(String, String)> {
        let (author, name) = pkg
            .name
            .split_once('/')
            .with_context(|| format!("{}: expected an author/module name", pkg.name))?;
        let url = format!("https://luarocks.org/modules/{author}/{name}");
        let body = ureq::get(&url)
            .call()
            .with_context(|| format!("GET {url}"))?
            .into_string()?;
        let href = format!("<a href=\"/modules/{author}/{name}/");
        for row in body.split("<div class=\"version_row\">").skip(1) {
            let row = row.split("</div>").next().unwrap_or(row);
            if row.contains("development_flag") {
                continue;
            }
            let Some(version) = between(row, &href, "\">") else { continue };
            // The per-author manifest path, not the root one: two authors can
            // publish the same rock name, and the root path resolves to only
            // one of them.
            let rock =
                format!("https://luarocks.org/manifests/{author}/{name}-{version}.src.rock");
            return Ok((version.to_string(), rock));
        }
        bail!("{}: no non-development version listed at {url}", pkg.name)
    }

    /// `.lua` only — the single extension tree-sitter-lua's tree-sitter.json
    /// claims, following the same rule as python and javascript.
    ///
    /// `.rockspec` files are also Lua syntax and this grammar parses them,
    /// and every package in the corpus ships one; they are left out for now
    /// so `classify()` matches what the grammar advertises. Adding them is a
    /// deliberate change with its own sweep evidence rather than a silent
    /// widening — and it would change the corpus's character, since a
    /// rockspec is a data table rather than code.
    fn classify(&self, rel: &Path) -> Option<Option<String>> {
        // `__MACOSX/` holds AppleDouble resource forks (`._name`) that macOS's
        // zip writer stores beside every real entry. They carry a `.lua`
        // extension while being binary metadata rather than source, so they
        // are not Lua by any definition — and because they open with a NUL
        // byte they would be counted as corpus noise forever. Measured: 7 in
        // the top-500 corpus, all from one rock published from a Mac.
        if rel.components().any(|c| c.as_os_str() == "__MACOSX") {
            return None;
        }
        (rel.extension()?.to_str()? == "lua").then_some(None)
    }

    /// A `.src.rock` is a zip carrying the rockspec plus the source, and how
    /// the source is carried is the packager's choice: often an unpacked
    /// directory, but for roughly a quarter of rocks it is upstream's release
    /// tarball dropped in whole. Measured over the top 50 by downloads: 12 of
    /// 50, including argparse (#8), lpeg (#19), luasocket and lua_cliargs.
    /// Without this they extract to zero files and read as packages with no
    /// Lua in them.
    fn nested_archives(&self) -> bool {
        true
    }

    fn grammar_dirs(&self) -> &'static [&'static str] {
        &["."]
    }

    /// tools/lua-oracle: PUC-Rio Lua's own front end via `loadfile(path,
    /// "t")`, which compiles the chunk and stops — no execution, no
    /// `require`, no name resolution — so a missing dependency is not an
    /// error and each file is judged on its own text.
    ///
    /// `loadfile` and not `luac -p` for one measured reason and one
    /// correctness one. Measured: `luac -p` forks per file, and on a
    /// 1000-file sample that is 1.65 s of which ~1.15 s is process creation;
    /// batching through one interpreter is 0.17 s, ~10x, and needs no
    /// parallel driver. Correctness: `loadfile` is `luaL_loadfilex`, the same
    /// entry point `luac -p` itself uses — including skipping a leading `#!`
    /// line, which `load(<string>)` does not. Verified verdict-for-verdict
    /// against `luac -p` over 2606 real files: identical, zero disagreements.
    ///
    /// The one deliberate divergence is mode `"t"`, which refuses precompiled
    /// binary chunks that `luac -p` would accept. A bytecode blob has no Lua
    /// syntax, so the grammar rightly fails it, and calling it valid would
    /// manufacture a grammar gap out of a file that has no source at all.
    fn validate(&self, srcroot: &Path, paths: &[String]) -> Result<HashMap<String, bool>> {
        stdin_oracle::run(
            LUA,
            &["tools/lua-oracle/check.lua"],
            &format!("{LUA} tools/lua-oracle/check.lua — is lua5.4 installed? (apt install lua5.4)"),
            srcroot,
            paths,
        )
    }
}

fn between<'a>(haystack: &'a str, start: &str, end: &str) -> Option<&'a str> {
    let i = haystack.find(start)? + start.len();
    let rest = &haystack[i..];
    Some(&rest[..rest.find(end)?])
}

/// LuaRocks publishes no ranking API and no bulk download-count endpoint —
/// unlike crates.io and npm, and unlike PyPI it has no public download
/// dataset either. The only complete source of per-module totals is the
/// paginated module listing itself, which carries the exact count in a
/// `title` attribute beside an abbreviated display value ("7.8m"). So this
/// walks the listing to its end and sorts locally.
///
/// The metric is **cumulative downloads since the rock was published**, which
/// is the same *kind* of number as crates.io's total (traffic), not Java's
/// dependent-repos proxy — but it is all-time rather than recent, so it
/// favours long-lived packages over fast-growing ones. Stated because it
/// biases the corpus and the bias is not visible in the number.
///
/// Names are `author/module`, because LuaRocks is not a flat namespace: two
/// authors publish `luasocket` and both are real rocks with real downloads.
/// `fetch::pkg_dir` turns the slash into `__` for the corpus directory.
fn rank_luarocks(k: usize) -> Result<Vec<RankedCrate>> {
    // Far above the ~134 pages the listing has today; a guard against
    // pagination changing shape and turning this into an infinite loop.
    const MAX_PAGES: usize = 2000;
    let mut rows: Vec<(u64, String)> = Vec::new();
    for page in 1..=MAX_PAGES {
        let url = format!("https://luarocks.org/modules?page={page}");
        let body = ureq::get(&url)
            .call()
            .with_context(|| format!("GET {url}"))?
            .into_string()?;
        let before = rows.len();
        for row in body.split("<li class=\"module_row\">").skip(1) {
            let row = row.split("</li>").next().unwrap_or(row);
            let Some(name) = between(row, "<a href=\"/modules/", "\" class=\"title\">") else {
                continue;
            };
            // The display text is abbreviated ("7.8m"); the title attribute
            // holds the exact figure. Reading the text would silently rank
            // every popular rock below every unpopular one.
            let Some(count) = row
                .find("class=\"downloads\"")
                .map(|i| &row[i..])
                .and_then(|seg| between(seg, "<span title=\"", "\" class=\"value\">"))
            else {
                continue;
            };
            let Ok(downloads) = count.replace(',', "").parse::<u64>() else { continue };
            rows.push((downloads, name.to_string()));
        }
        if rows.len() == before {
            eprintln!("rank: luarocks listing ended at page {page} ({} modules)", rows.len());
            break;
        }
    }
    if rows.is_empty() {
        bail!("luarocks module listing came out empty — has the page markup changed?");
    }
    rows.sort_by(|a, b| b.0.cmp(&a.0).then_with(|| a.1.cmp(&b.1)));
    Ok(rows
        .into_iter()
        .take(k)
        .enumerate()
        .map(|(i, (downloads, name))| RankedCrate {
            rank: i + 1,
            name,
            // Resolved at fetch time from the module page, like java and python.
            version: String::new(),
            downloads,
        })
        .collect())
}
