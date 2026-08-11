use std::collections::HashMap;
use std::path::Path;

use anyhow::{bail, Result};

use super::{debian, exec_oracle, github, Lang};
use crate::ledger::LangName;
use crate::rank::RankedCrate;

pub struct Bash;

/// **Shell is a guest language.** That is the one fact that makes bash's
/// corpus a different problem from every language already here, and it
/// changes the filter rather than the source.
///
/// C's filter asks "is this package C?", and the question is meaningful:
/// packages are written in C. No package is written in shell. Shell arrives
/// as build glue, init scripts, test harnesses, maintainer helpers and
/// wrappers inside packages written in something else — measured on sid, the
/// largest bodies of shell in the archive belong to git (278k lines, mostly
/// its test suite), systemd (41k) and bash itself (8.9k, next to 135k lines
/// of C). A "more shell than anything else" rule of C's shape would select
/// only the tiny shell-only packages and miss every one of those.
///
/// So the rule is a floor and nothing else: enough shell to be worth a
/// download, wherever it sits in the package. The consequence is worth
/// saying out loud — popcon installs are attributed to the *package*, not to
/// its shell, so this ranks "shell that ships inside software people
/// install", which is the honest reading of it and not the same as "popular
/// shell scripts".
fn is_sh(s: &debian::Sloc) -> bool {
    s.lines("sh") >= SH_MIN
}

/// Measured against the popcon walk rather than guessed; see ledger.json's
/// `corpus.ranking_note` for the distribution this came out of.
const SH_MIN: i64 = 500;

/// Filenames that are shell by name, with no extension to go on. These are
/// the two `tree-sitter.json` lists under `file-types` that are whole names
/// rather than suffixes.
const SHELL_FILENAMES: [&str; 2] = [".bashrc", ".bash_profile"];

/// Extensions `tree-sitter-bash`'s own `tree-sitter.json` claims.
const SHELL_EXTENSIONS: [&str; 4] = ["sh", "bash", "ebuild", "eclass"];

/// The interpreters this grammar is for, and an allowlist rather than a
/// denylist of the others. These are exactly the three in upstream's own
/// `first-line-regex`, `^#!.*\b(sh|bash|dash)\b.*$`.
///
/// It has to be an allowlist. zsh, ksh, fish and csh are different languages
/// that would be reported as gaps, but so are perl, python, awk, expect and
/// every other thing with a shebang — an extensionless file is a candidate
/// precisely because its name says nothing, so the shebang has to *grant*
/// admission, not merely fail to deny it. `\b` is what keeps `zsh` and `ksh`
/// out while letting `sh` in, and reducing the line to the program name has
/// the same effect more directly.
const SHELL_SHEBANGS: [&str; 3] = ["sh", "bash", "dash"];

/// Bash is the first language with no registry *and* a choice of artifact
/// source, and the two answer different questions — see `lang::debian` and
/// `lang::github` for what each biases toward. `TREEBANK_BASH_CORPUS` picks
/// one; the default is Debian, because popcon installs are a real usage
/// metric and GitHub stars are not.
///
/// The choice has to be recorded, not just made: a `top-k.json` gives no
/// hint of where its names came from, and "the bash sweep found N gaps" is
/// not a claim about bash unless the corpus behind it is named. `rank`
/// writes `db/source.json` and both numbers are reported together.
#[derive(Clone, Copy, PartialEq)]
enum Source {
    Debian,
    Github,
}

fn source() -> Result<Source> {
    match std::env::var("TREEBANK_BASH_CORPUS").as_deref() {
        Ok("github") => Ok(Source::Github),
        Ok("debian") | Err(_) => Ok(Source::Debian),
        Ok(other) => bail!("TREEBANK_BASH_CORPUS={other:?}: expected \"debian\" or \"github\""),
    }
}

impl Lang for Bash {
    fn name(&self) -> LangName {
        LangName::Bash
    }

    fn rank(&self, db: &Path, k: usize) -> Result<Vec<RankedCrate>> {
        let (ranked, name) = match source()? {
            Source::Debian => (
                debian::rank(LangName::Bash, db, k, "shell-carrying", &is_sh)?,
                "debian",
            ),
            Source::Github => (github::rank(LangName::Bash, "Shell", k)?, "github"),
        };
        std::fs::create_dir_all(db)?;
        std::fs::write(
            db.join("source.json"),
            serde_json::json!({
                "source": name,
                "requested_k": k,
                "ranked": ranked.len(),
                "note": "which artifact corpus corpus/bash/top-k.json came from; \
                         set by TREEBANK_BASH_CORPUS",
            })
            .to_string(),
        )?;
        eprintln!("rank: corpus source is {name}");
        Ok(ranked)
    }

    fn resolve(&self, pkg: &RankedCrate) -> Result<(String, String)> {
        match source()? {
            Source::Debian => debian::resolve(LangName::Bash, pkg),
            Source::Github => github::resolve(LangName::Bash, pkg),
        }
    }

    /// Two admission routes, because shell is the first language here whose
    /// files mostly **have no extension**: `/usr/bin/foo` is a shell script
    /// and nothing about its name says so. Measured on this machine's own
    /// `/usr` and `/etc`, 933 of the 963 shell scripts present are found by
    /// shebang and only 124 by `*.sh`/`*.bash` — an extension-only rule
    /// would have seen 13% of the corpus.
    ///
    /// So `classify` takes the extensions and names `tree-sitter.json`
    /// claims, *plus* every extensionless file as a candidate, and `admit`
    /// decides the candidates on their first line. Splitting it this way is
    /// forced by the trait: `classify` sees only a path, and the answer is
    /// not in the path.
    fn classify(&self, rel: &Path) -> Option<Option<String>> {
        let name = rel.file_name()?.to_str()?;
        if SHELL_FILENAMES.contains(&name) {
            return Some(None);
        }
        match rel.extension() {
            Some(ext) => SHELL_EXTENSIONS.contains(&ext.to_str()?).then_some(None),
            // A candidate: `admit` reads the shebang.
            None => Some(None),
        }
    }

    /// The shebang decides every extensionless candidate, and overrides the
    /// extension when the two disagree — a `*.sh` beginning
    /// `#!/usr/bin/env zsh` is real and does occur.
    ///
    /// The test on the first line is upstream's own: `tree-sitter.json`
    /// declares `first-line-regex: ^#!.*\b(sh|bash|dash)\b.*$`, so a file
    /// admitted here is a file tree-sitter-bash says it wants, and the
    /// corpus boundary is the grammar's own claim rather than ours.
    ///
    /// What this does NOT catch, stated because it shows up in the numbers:
    /// a `#!/bin/sh` polyglot whose body is another language. netpbm ships
    /// ten of them — a two-line shell preamble that `exec perl -x -S "$0"`
    /// and then 300 lines of Perl. They are admitted here, the oracle calls
    /// them invalid, and they are recorded as corpus noise, which is the
    /// right bucket but not a free one.
    fn admit(&self, rel: &Path, content: &[u8]) -> bool {
        // A NUL means bash itself refuses the file ("cannot execute binary
        // file", exit 126), so it can never be valid. The whole file is
        // scanned, not a leading window: 126 is deliberately NOT one of the
        // oracle's reject statuses, so a NUL that got past this check would
        // abort the sweep rather than be miscounted. Measured over both
        // corpora, no admitted file contains one.
        if content.contains(&0) {
            return false;
        }
        let head = &content[..content.len().min(8192)];
        let named = rel
            .file_name()
            .and_then(|n| n.to_str())
            .is_some_and(|n| SHELL_FILENAMES.contains(&n));
        let by_ext = rel
            .extension()
            .and_then(|e| e.to_str())
            .is_some_and(|e| SHELL_EXTENSIONS.contains(&e));
        match shebang_shell(head) {
            Some(shell) => SHELL_SHEBANGS.contains(&shell.as_str()),
            // No shebang at all: a `.sh`/`.bashrc` is still shell (sourced
            // fragments and `.bashrc` never carry one), an extensionless
            // file is just a file.
            None => named || by_ext,
        }
    }

    /// 250 MB. Shell is a guest language, so an artifact's size is decided
    /// by its host: the top 500 Debian sources by popcon come to 11.5 GB, of
    /// which **7.6 GB is eight packages** — texlive-extra (3.2 GB),
    /// chromium, qt6-webengine, texlive-lang, texlive-base, firefox-esr,
    /// libreoffice, qtwebengine — that between them are 1.6% of the corpus.
    /// Two-thirds of the bytes for one part in sixty.
    ///
    /// This is a real change to the population and it is stated rather than
    /// hidden: chromium and firefox-esr carry tens of thousands of lines of
    /// shell each, and they are **not** in the corpus. Every skip is logged
    /// by the fetch driver, and `ledger.json` records the cap next to the
    /// package count it produced.
    fn max_artifact_bytes(&self) -> Option<u64> {
        Some(250_000_000)
    }

    fn grammar_dirs(&self) -> &'static [&'static str] {
        &["."]
    }

    /// `bash -n`: bash's own parser, which reads the whole script and
    /// executes nothing. That property is the entire safety case for running
    /// a shell over thousands of strangers' scripts and it was verified on
    /// this machine with live canaries before the first sweep — `source
    /// /absent/file` is not an error, a `rm -rf` in a scanned script does not
    /// run, and neither do command substitutions, process substitutions,
    /// heredoc bodies, `eval` arguments or `BASH_ENV`. See
    /// `crates/treebank-bash/ORACLE.md`.
    ///
    /// bash cannot syntax-check a file from inside a long-lived shell —
    /// `set -n` stops that shell from executing the `source` that would read
    /// the next one — so this is a fork-per-file oracle and it inherits
    /// `exec_oracle`, exactly as that module's own note anticipates.
    ///
    /// **Two reject statuses, both measured.** `bash -n` exits 2 for a syntax
    /// error nearly everywhere, and **1** when the error is inside an
    /// array-assignment word list: `x=( a+([0-9]) )` exits 1 where the same
    /// pattern outside an array exits 2. That is not a curiosity — linux's
    /// `tools/testing/selftests/wireguard/netns.sh` is such a file, and with
    /// only `2` in the list the whole sweep would abort on it. 126 (bash
    /// refuses a binary or a directory) and 127 (no such file) stay outside
    /// the list on purpose, so a mistyped corpus root still fails loudly
    /// instead of scoring every file invalid.
    ///
    /// `--` guards a corpus path that begins with a dash.
    fn validate(&self, srcroot: &Path, paths: &[String]) -> Result<HashMap<String, bool>> {
        exec_oracle::run(
            "bash",
            &["-n", "--"],
            &[1, 2],
            "spawn bash -n — is bash installed?",
            srcroot,
            paths,
        )
    }
}

/// The interpreter named on a `#!` line, reduced to its bare name: the last
/// path component, and the first argument instead when that component is
/// `env`. `#!/usr/bin/env -S bash -e` is real, so leading `-` words are
/// skipped.
fn shebang_shell(head: &[u8]) -> Option<String> {
    let first = head.split(|b| *b == b'\n').next()?;
    let line = std::str::from_utf8(first).ok()?.trim_end_matches('\r');
    let rest = line.strip_prefix("#!")?.trim();
    let mut words = rest.split_whitespace();
    let mut prog = words.next()?.rsplit('/').next()?.to_string();
    if prog == "env" {
        prog = words
            .find(|w| !w.starts_with('-'))?
            .rsplit('/')
            .next()?
            .to_string();
    }
    Some(prog)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn shebang_names_the_interpreter() {
        assert_eq!(shebang_shell(b"#!/bin/bash\n").as_deref(), Some("bash"));
        assert_eq!(shebang_shell(b"#! /bin/sh\n").as_deref(), Some("sh"));
        assert_eq!(shebang_shell(b"#!/usr/bin/env bash\n").as_deref(), Some("bash"));
        assert_eq!(shebang_shell(b"#!/usr/bin/env -S bash -e\n").as_deref(), Some("bash"));
        assert_eq!(shebang_shell(b"#!/bin/zsh -f\n").as_deref(), Some("zsh"));
        assert_eq!(shebang_shell(b"#!/bin/bash\r\n").as_deref(), Some("bash"));
        assert_eq!(shebang_shell(b"echo hi\n"), None);
    }

    #[test]
    fn extensionless_files_need_a_shell_shebang() {
        let b = Bash;
        assert!(b.admit(Path::new("bin/foo"), b"#!/bin/sh\necho hi\n"));
        assert!(b.admit(Path::new("configure"), b"#! /bin/sh\n"));
        assert!(!b.admit(Path::new("README"), b"a readme\n"));
        assert!(!b.admit(Path::new("gen.py"), b"#!/usr/bin/python3\n"));
        assert!(!b.admit(Path::new("bin/tool"), b"#!/usr/bin/perl\nprint 1;\n"));
    }

    #[test]
    fn a_wrong_dialect_shebang_vetoes_the_extension() {
        let b = Bash;
        assert!(!b.admit(Path::new("x.sh"), b"#!/usr/bin/env zsh\n"));
        assert!(!b.admit(Path::new("x.sh"), b"#!/bin/csh\n"));
        // no shebang at all, but named as shell: still shell
        assert!(b.admit(Path::new("lib/common.sh"), b"foo() { :; }\n"));
        assert!(b.admit(Path::new(".bashrc"), b"PS1='$ '\n"));
    }

    #[test]
    fn binaries_are_never_admitted() {
        let b = Bash;
        assert!(!b.admit(Path::new("x.sh"), b"#!/bin/sh\n\0\0binary"));
    }

    #[test]
    fn classify_takes_claimed_names_and_extensionless_candidates() {
        let b = Bash;
        assert!(b.classify(Path::new("a/b.sh")).is_some());
        assert!(b.classify(Path::new("a/b.bash")).is_some());
        assert!(b.classify(Path::new("a/foo.ebuild")).is_some());
        assert!(b.classify(Path::new("a/.bashrc")).is_some());
        assert!(b.classify(Path::new("configure")).is_some());
        assert!(b.classify(Path::new("a/main.c")).is_none());
        assert!(b.classify(Path::new("a/README.md")).is_none());
    }
}
