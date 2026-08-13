//! Per-package Haskell configuration, read from the package's `.cabal` file.
//!
//! # Why a language needs this at all
//!
//! GHC's parser is configured. `\case` is a parse error without
//! `LambdaCase`, `%1 ->` without `LinearTypes`, `proc` is an ordinary
//! identifier until `Arrows` makes it a keyword — and real packages declare
//! those extensions **in the `.cabal` file**, not in the source. So a file
//! that parses inside its package fails on its own, and a sweep that judges
//! files on their own text alone books the difference as corpus noise.
//!
//! Measured on 5,631 `.hs` files from the top 40 Hackage packages by recent
//! downloads:
//!
//! | judged | valid | invalid |
//! |---|---|---|
//! | file alone | 3,958 | 1,673 (29.7%) |
//! | with its package's configuration | 4,532 | 1,099 (19.5%) |
//!
//! **575 files (10.2% of the corpus, 34.4% of all failures) change verdict**,
//! every one of them invalid → valid. That is the number this module exists
//! for. The residual is a different mechanism entirely and is not this
//! module's to fix: every one of the 153 files still failing in a 1,000-file
//! sample contains `#if`/`#ifdef` lines, i.e. they are CPP files, which is
//! `treebank_preprocessing`'s territory rather than `LANGUAGE`'s.
//!
//! # Why it is scoped rather than unioned
//!
//! The first cut of this measurement unioned every extension found anywhere
//! in a package and applied it to every file. That recovered the same 575
//! files and **broke one**: `cabal2nix`'s `Fetch.hs`, which uses `proc` as a
//! function argument, and which `-XArrows` therefore invalidates. Package
//! configuration is not a bag of flags — it is per component, and applying a
//! component's flags to a file outside it is as wrong as omitting them.
//!
//! The same throwaway pass also read every `*.cabal` file in the package
//! tree, and cabal2nix ships **357 of them**, 356 being golden-test fixtures
//! for the tool itself. So the package's own manifest is the one at the
//! package root, and nothing deeper is configuration.
//!
//! # What is deliberately approximate
//!
//! - **Conditionals are unioned, not evaluated.** `if impl(ghc >= 9.4)` and
//!   its `else` both contribute. Cabal resolves those against a compiler,
//!   OS and flag assignment that a corpus fetch does not have; the C
//!   preprocessor reduction in `treebank_preprocessing` exists because that
//!   guess is worth making carefully, and this is the cheap end of the same
//!   problem. Stated because it biases toward more extensions, which is the
//!   direction that can invalidate a file.
//! - **Unknown extension names are passed through** and GHC silently
//!   ignores what it does not know (verified: `-XTotallyBogus` changes no
//!   verdict and does not swallow the flag after it — see
//!   tools/hs-oracle/battery). This matters because .cabal files in a real
//!   corpus name extensions that current GHC has removed, `TypeInType`
//!   among them, and a package must not become uniformly invalid because
//!   its manifest mentions one.
//! - **`cpp-options` are not read.** They configure a preprocessor this
//!   oracle does not run; see the CPP note above.

use std::collections::{BTreeSet, HashMap};
use std::path::Path;
use std::sync::{Arc, LazyLock, Mutex};

/// One buildable component: where its modules live, and how they are parsed.
#[derive(Debug, Default, Clone, PartialEq)]
pub struct Component {
    /// `hs-source-dirs`, normalized, `.` for the package root. Cabal's own
    /// default when the field is absent is the package root, which is why
    /// an empty list is stored as `["."]` — that component then matches
    /// every file, and the longest-prefix rule below still prefers a
    /// component that named a real directory.
    pub source_dirs: Vec<String>,
    /// `-X` flags: `default-language` first, then `default-extensions`.
    pub flags: Vec<String>,
}

/// A package's manifest, reduced to what decides a parse.
#[derive(Debug, Default, Clone, PartialEq)]
pub struct CabalConfig {
    pub components: Vec<Component>,
}

impl CabalConfig {
    /// The flags for one file, given its path relative to the package root.
    ///
    /// Longest matching `hs-source-dirs` wins, because that is how cabal
    /// itself resolves a module to a component; ties union, because a file
    /// under two components' source dirs really is compiled twice, once
    /// under each configuration, and the parse has to succeed under the one
    /// that is more permissive.
    pub fn flags_for(&self, rel_to_package: &str) -> Vec<String> {
        let mut best: usize = 0;
        let mut flags: Vec<String> = Vec::new();
        let mut matched = false;
        for c in &self.components {
            for dir in &c.source_dirs {
                let depth = if dir == "." {
                    0
                } else if let Some(rest) = rel_to_package.strip_prefix(dir.as_str()) {
                    if !rest.starts_with('/') {
                        continue;
                    }
                    dir.split('/').count()
                } else {
                    continue;
                };
                if !matched || depth > best {
                    best = depth;
                    flags = c.flags.clone();
                    matched = true;
                } else if depth == best {
                    for f in &c.flags {
                        if !flags.contains(f) {
                            flags.push(f.clone());
                        }
                    }
                }
                break;
            }
        }
        flags
    }
}

/// Read and reduce the `.cabal` at a package root. Missing or unreadable
/// manifests give an empty configuration, which means "judge these files on
/// their own text" — the behaviour before this module existed, and the right
/// answer for a package that ships no manifest.
pub fn parse(text: &str) -> CabalConfig {
    // Stanza-aware, indentation-driven, and deliberately not a full cabal
    // parser: the fields that matter here are three, and a real one is a
    // 20k-line library with a Haskell dependency.
    let mut commons: HashMap<String, Component> = HashMap::new();
    let mut components: Vec<Component> = Vec::new();
    let mut imports: Vec<(usize, Vec<String>)> = Vec::new();

    let mut cur: Option<(Option<String>, Component, Vec<String>)> = None;
    let flush = |cur: &mut Option<(Option<String>, Component, Vec<String>)>,
                 commons: &mut HashMap<String, Component>,
                 components: &mut Vec<Component>,
                 imports: &mut Vec<(usize, Vec<String>)>| {
        if let Some((common_name, comp, imported)) = cur.take() {
            match common_name {
                Some(name) => {
                    commons.insert(name, comp);
                }
                None => {
                    if !imported.is_empty() {
                        imports.push((components.len(), imported));
                    }
                    components.push(comp);
                }
            }
        }
    };

    let mut lines = text.lines().peekable();
    while let Some(raw) = lines.next() {
        let line = strip_comment(raw);
        if line.trim().is_empty() {
            continue;
        }
        let indent = indent_of(&line);
        let trimmed = line.trim();

        // A stanza header sits at column 0: `library`, `library internal`,
        // `executable foo`, `test-suite`, `benchmark`, `common warnings`.
        if indent == 0 {
            if let Some((kind, name)) = stanza_header(trimmed) {
                flush(&mut cur, &mut commons, &mut components, &mut imports);
                let is_common = kind == "common";
                cur = Some((
                    is_common.then(|| name.to_ascii_lowercase()),
                    Component::default(),
                    Vec::new(),
                ));
                continue;
            }
            // Any other column-0 line is a package-level field
            // (`name:`, `build-type:`, …) and ends the current stanza.
            if trimmed.contains(':') {
                flush(&mut cur, &mut commons, &mut components, &mut imports);
                continue;
            }
        }

        let Some((_, comp, imported)) = cur.as_mut() else { continue };
        let Some((key, first)) = trimmed.split_once(':') else { continue };
        let key = key.trim().to_ascii_lowercase();
        if !matches!(
            key.as_str(),
            "hs-source-dirs" | "hs-source-dir" | "default-extensions" | "default-language" | "import"
        ) {
            continue;
        }
        // Field values continue on any line indented deeper than the field
        // name itself. `if`/`else` blocks are inside that span and their
        // bodies are read as ordinary fields, which is the union documented
        // at the top of this file.
        let mut value = first.trim().to_string();
        while let Some(next) = lines.peek() {
            let next_line = strip_comment(next);
            if next_line.trim().is_empty() {
                lines.next();
                continue;
            }
            if indent_of(&next_line) > indent && !next_line.trim().contains(':') {
                value.push(' ');
                value.push_str(next_line.trim());
                lines.next();
            } else {
                break;
            }
        }

        let items: Vec<String> = value
            .split([',', ' ', '\t'])
            .map(|s| s.trim())
            .filter(|s| !s.is_empty())
            .map(|s| s.to_string())
            .collect();
        match key.as_str() {
            "import" => imported.extend(items.into_iter().map(|s| s.to_ascii_lowercase())),
            "hs-source-dirs" | "hs-source-dir" => {
                for d in items {
                    let d = normalize_dir(&d);
                    if !comp.source_dirs.contains(&d) {
                        comp.source_dirs.push(d);
                    }
                }
            }
            "default-language" => {
                if let Some(l) = items.first() {
                    let flag = format!("-X{l}");
                    if !comp.flags.contains(&flag) {
                        comp.flags.insert(0, flag);
                    }
                }
            }
            "default-extensions" => {
                for e in items {
                    // `default-extensions: LambdaCase, NoImplicitPrelude` —
                    // the names are already GHC's, `No` prefix included.
                    if !e.chars().next().is_some_and(|c| c.is_ascii_alphabetic()) {
                        continue;
                    }
                    let flag = format!("-X{e}");
                    if !comp.flags.contains(&flag) {
                        comp.flags.push(flag);
                    }
                }
            }
            _ => {}
        }
    }
    flush(&mut cur, &mut commons, &mut components, &mut imports);

    // `common` stanzas are where modern packages keep their extension list,
    // pulled in with `import: warnings, extensions`. Ignoring them would
    // lose the configuration of exactly the packages that organise it best.
    for (idx, names) in imports {
        for name in names {
            if let Some(src) = commons.get(&name) {
                let comp = &mut components[idx];
                for f in &src.flags {
                    if !comp.flags.contains(f) {
                        comp.flags.push(f.clone());
                    }
                }
                for d in &src.source_dirs {
                    if !comp.source_dirs.contains(d) {
                        comp.source_dirs.push(d.clone());
                    }
                }
            }
        }
    }

    for c in &mut components {
        if c.source_dirs.is_empty() {
            c.source_dirs.push(".".to_string());
        }
    }
    components.retain(|c| !c.flags.is_empty());
    CabalConfig { components }
}

fn stanza_header(line: &str) -> Option<(String, &str)> {
    let (head, rest) = match line.split_once(char::is_whitespace) {
        Some((h, r)) => (h, r.trim()),
        None => (line, ""),
    };
    let kind = head.trim().to_ascii_lowercase();
    matches!(
        kind.as_str(),
        "library" | "executable" | "test-suite" | "benchmark" | "common" | "foreign-library"
    )
    .then_some((kind, rest))
}

fn strip_comment(line: &str) -> String {
    match line.find("--") {
        // Only a comment that starts the (indented) line is stripped: `--`
        // appears inside version ranges and option strings.
        Some(i) if line[..i].trim().is_empty() => line[..i].to_string(),
        _ => line.to_string(),
    }
}

fn indent_of(line: &str) -> usize {
    line.len() - line.trim_start().len()
}

fn normalize_dir(d: &str) -> String {
    let d = d.trim().trim_matches('"').trim_end_matches('/');
    let d = d.strip_prefix("./").unwrap_or(d);
    if d.is_empty() || d == "." {
        ".".to_string()
    } else {
        d.to_string()
    }
}

/// The package's own manifest is the `.cabal` at the package root and
/// nothing deeper. Reading the tree instead finds test fixtures — cabal2nix
/// alone ships 356 of them — and configures the package from another
/// package's manifest.
fn read_package_cabal(pkg_root: &Path) -> Option<String> {
    let mut found: BTreeSet<String> = BTreeSet::new();
    for entry in std::fs::read_dir(pkg_root).ok()?.flatten() {
        let path = entry.path();
        if path.extension().is_some_and(|e| e == "cabal") {
            found.insert(path.to_string_lossy().into_owned());
        }
    }
    // Sorted, so a package that ships two manifests at its root resolves the
    // same way on every machine rather than in readdir order.
    found.iter().next().and_then(|p| std::fs::read_to_string(p).ok())
}

static CACHE: LazyLock<Mutex<HashMap<String, Arc<CabalConfig>>>> =
    LazyLock::new(|| Mutex::new(HashMap::new()));

/// The configuration for a corpus package, computed once.
///
/// Keyed by the corpus's package directory, which is the unit a manifest
/// describes; a sweep asks for tens of thousands of files across a few
/// hundred packages, so this is read once per package rather than once per
/// file. Same shape as `c.rs`'s include-path cache, for the same reason.
pub fn for_package(srcroot: &Path, pkgdir: &str) -> Arc<CabalConfig> {
    if let Some(hit) = CACHE.lock().unwrap().get(pkgdir) {
        return hit.clone();
    }
    let cfg = Arc::new(
        read_package_cabal(&srcroot.join(pkgdir))
            .map(|t| parse(&t))
            .unwrap_or_default(),
    );
    CACHE.lock().unwrap().insert(pkgdir.to_string(), cfg.clone());
    cfg
}

#[cfg(test)]
mod tests {
    use super::*;

    fn flags(text: &str, file: &str) -> Vec<String> {
        parse(text).flags_for(file)
    }

    #[test]
    fn library_stanza_configures_its_own_source_dir() {
        let c = "\
name: demo
library
  hs-source-dirs: src
  default-language: Haskell2010
  default-extensions: LambdaCase, OverloadedStrings
";
        assert_eq!(
            flags(c, "src/Demo.hs"),
            vec!["-XHaskell2010", "-XLambdaCase", "-XOverloadedStrings"]
        );
    }

    /// The cabal2nix regression, minimized. `-XArrows` makes `proc` a
    /// keyword, so a test stanza's extension must not reach a library file:
    /// unioning the package's extensions is what broke `Fetch.hs`.
    #[test]
    fn a_components_extensions_do_not_leak_to_another() {
        let c = "\
name: demo
library
  hs-source-dirs: src
  default-language: Haskell2010
test-suite spec
  hs-source-dirs: test
  default-language: Haskell2010
  default-extensions: Arrows
";
        assert_eq!(flags(c, "src/Fetch.hs"), vec!["-XHaskell2010"]);
        assert_eq!(flags(c, "test/Spec.hs"), vec!["-XHaskell2010", "-XArrows"]);
    }

    #[test]
    fn longest_source_dir_wins() {
        let c = "\
library
  hs-source-dirs: .
  default-extensions: LambdaCase
executable app
  hs-source-dirs: app/cli
  default-extensions: MultiWayIf
";
        assert_eq!(flags(c, "app/cli/Main.hs"), vec!["-XMultiWayIf"]);
        assert_eq!(flags(c, "Lib.hs"), vec!["-XLambdaCase"]);
        // `app/cli` must not match `app/climate/...` — prefix on the string
        // is not prefix on the path.
        assert_eq!(flags(c, "app/climate/X.hs"), vec!["-XLambdaCase"]);
    }

    /// Where modern packages actually keep their extension list.
    #[test]
    fn common_stanzas_are_imported() {
        let c = "\
common extensions
  default-language: GHC2021
  default-extensions:
    LambdaCase
    DerivingVia
library
  import: extensions
  hs-source-dirs: src
";
        assert_eq!(
            flags(c, "src/M.hs"),
            vec!["-XGHC2021", "-XLambdaCase", "-XDerivingVia"]
        );
    }

    /// Both branches contribute: cabal resolves these against a compiler and
    /// flag assignment a corpus fetch does not have.
    #[test]
    fn conditional_branches_are_unioned() {
        let c = "\
library
  hs-source-dirs: src
  default-extensions: LambdaCase
  if impl(ghc >= 9.4)
    default-extensions: MultiWayIf
  else
    default-extensions: TupleSections
";
        let f = flags(c, "src/M.hs");
        assert!(f.contains(&"-XMultiWayIf".to_string()));
        assert!(f.contains(&"-XTupleSections".to_string()));
    }

    #[test]
    fn multi_line_and_comma_forms_both_parse() {
        let commas = "library\n  hs-source-dirs: src, lib\n  default-extensions: A, B\n";
        let lines = "library\n  hs-source-dirs:\n    src\n    lib\n  default-extensions:\n    A\n    B\n";
        assert_eq!(parse(commas), parse(lines));
        assert_eq!(flags(commas, "lib/M.hs"), vec!["-XA", "-XB"]);
    }

    /// A leading `--` is a comment; a `--` inside a version range is not.
    #[test]
    fn only_leading_comments_are_stripped() {
        let c = "\
library
  hs-source-dirs: src
  -- default-extensions: Arrows
  default-extensions: LambdaCase
  build-depends: base >=4 && <5
";
        assert_eq!(flags(c, "src/M.hs"), vec!["-XLambdaCase"]);
    }

    #[test]
    fn a_file_in_no_component_gets_no_flags() {
        let c = "library\n  hs-source-dirs: src\n  default-extensions: Arrows\n";
        assert!(flags(c, "test/golden/Case.hs").is_empty());
    }

    #[test]
    fn package_level_fields_do_not_leak_into_the_last_stanza() {
        // `name:`/`license:` after a stanza are package-level again; a reader
        // that keeps appending to the open stanza would attach them to it.
        let c = "\
library
  hs-source-dirs: src
  default-extensions: LambdaCase
source-repository head
  type: git
";
        assert_eq!(flags(c, "src/M.hs"), vec!["-XLambdaCase"]);
    }

    #[test]
    fn no_manifest_means_no_configuration() {
        assert_eq!(parse(""), CabalConfig::default());
        assert!(flags("name: demo\nversion: 1\n", "src/M.hs").is_empty());
    }
}
