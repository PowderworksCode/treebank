use std::collections::HashMap;
use std::path::Path;

use anyhow::Result;

use super::{github, stdin_oracle, Lang};
use crate::ledger::LangName;
use crate::rank::RankedCrate;

pub struct Yaml;

/// Extensions `tree-sitter-yaml`'s own `tree-sitter.json` claims, and the
/// corpus boundary is that claim rather than ours.
const YAML_EXTENSIONS: [&str; 2] = ["yml", "yaml"];

/// Jinja statement keywords, the same list bash uses for the same reason.
/// Ansible and every Python-adjacent tool template YAML with these.
const JINJA_KEYWORDS: [&str; 14] = [
    "if", "for", "set", "block", "endif", "endfor", "macro", "include", "extends", "raw", "filter",
    "with", "call", "import",
];

impl Lang for Yaml {
    fn name(&self) -> LangName {
        LangName::Yaml
    }

    /// **YAML is a guest language, and unlike bash it cannot use Debian.**
    ///
    /// The guest-language corpus problem is bash's and the machinery is
    /// bash's: no package is written in YAML, so YAML arrives as CI config,
    /// manifests, playbooks, chart values and test fixtures inside packages
    /// written in something else. `debian::rank` is exactly the right shape
    /// for that and it is unusable here for a measured reason:
    /// **sources.debian.org's SLOC census has no `yaml` category at all**, so
    /// a `keep` predicate asking for YAML lines is identically false and the
    /// corpus comes out empty. Checked rather than assumed —
    /// `ansible-core 2.21.2-1`, which is YAML by the majority of its files,
    /// reports `python=179848, cs=5783, sh=5085, xml=34, makefile=21`; and
    /// `prometheus 3.5.3`, which is where this grammar's only measured gap
    /// came from, reports `yacc, sh, makefile, lex`. sloccount predates both
    /// YAML and Go and counts neither.
    ///
    /// So the source is GitHub, ranked by stars over repositories GitHub's
    /// own classifier calls YAML — `github::rank`, which bash built as its
    /// alternative source and which needs nothing new here. The bias is
    /// worth stating because it is invisible in the number: stars rank
    /// *repositories people like*, not *YAML people parse*, and a
    /// YAML-primary repository is by construction one whose YAML is the
    /// point (chart collections, manifest sets, Actions libraries) rather
    /// than one whose YAML is incidental config. That is a different
    /// population from the one Package Room will meet in the wild, and the
    /// honest fix is a second source later, not a silent claim now.
    fn rank(&self, _db: &Path, k: usize) -> Result<Vec<RankedCrate>> {
        github::rank(LangName::Yaml, "YAML", k)
    }

    fn resolve(&self, pkg: &RankedCrate) -> Result<(String, String)> {
        github::resolve(LangName::Yaml, pkg)
    }

    fn classify(&self, rel: &Path) -> Option<Option<String>> {
        (rel.extension().and_then(|e| e.to_str()))
            .filter(|e| YAML_EXTENSIONS.contains(e))
            .map(|_| None)
    }

    /// Two exclusions, both measured on a 3,217-file corpus.
    ///
    /// **A NUL means no grammar can ever parse the file.** treebank-lua
    /// established that tree-sitter's lexer reserves codepoint 0 for
    /// end-of-input, so a NUL truncates the parse and everything after it is
    /// ERROR — a failure no `grammar.js` change can fix. The oracle rejects
    /// such files (YAML 1.2.2 §5.1 excludes x00 from `c-printable`), so they
    /// would be recorded as noise rather than as gaps, but they would be
    /// noise forever, and a binary file that happens to end in `.yaml` is
    /// not a YAML document by any reading.
    ///
    /// **A template that renders to YAML is not YAML.** Helm charts put Go
    /// template actions where YAML syntax goes, and whether the result
    /// parses is an accident of where the braces land: `key: {{ .V }}` is
    /// two nested flow mappings and fails, `key: "{{ .V }}"` is a quoted
    /// scalar and passes. Measured over the corpus: 1,031 files carry a bare
    /// `{{` action, and they hold 101 of the 104 files the oracle rejects
    /// and 99 of the 103 the grammar fails. Excluding them takes the corpus
    /// to 2,178 files with 3 oracle rejects and 4 grammar failures, and
    /// leaves the one real gap in place. It costs something and the number
    /// is worth writing down: ~930 of those 1,031 files *are* valid YAML,
    /// and they are dropped anyway, because their parseability is a property
    /// of the template author's quoting habits rather than of the grammar.
    ///
    /// **`${{ }}` is deliberately NOT a template marker.** GitHub Actions
    /// expressions sit inside YAML scalars and the file is ordinary YAML:
    /// all 32 corpus files whose only `{{` is a `${{` are valid *and* parse
    /// clean, 0 rejects and 0 failures. The `$` lookbehind is what keeps a
    /// third of GitHub's YAML-primary repositories in the corpus, so it is
    /// load-bearing rather than a nicety.
    fn admit(&self, _rel: &Path, content: &[u8]) -> bool {
        if content.contains(&0) {
            return false;
        }
        !looks_like_a_template(content)
    }

    /// GitHub source archives are as big as the project, and for a guest
    /// language the size is set by the host while the yield is not — the
    /// same asymmetry bash measured on Debian. 250 MB, bash's figure, until
    /// there is a YAML measurement to replace it with.
    fn max_artifact_bytes(&self) -> Option<u64> {
        Some(250_000_000)
    }

    fn grammar_dirs(&self) -> &'static [&'static str] {
        &["."]
    }

    /// `tools/yaml-oracle`: js-yaml 5's event parser behind an explicit
    /// YAML 1.2.2 §5.2 decode layer.
    ///
    /// **This is the first oracle in this repo that is a position rather
    /// than a fact**, and `ledger.json`'s `oracle` block carries the whole
    /// argument: `authority: "position"`, the six alternatives that were
    /// measured, and the three populations they were measured on. The short
    /// version, because a reader here deserves the reason and not just a
    /// pointer: YAML has no reference implementation, the four mainstream
    /// parsers disagree with each other on 67 of the official suite's 402
    /// cases, and this one is the only candidate that scores 402/402 with
    /// neither an accepted-invalid nor a rejected-valid case. `rejects-valid`
    /// is the column that matters, because `validate()` only ever runs on
    /// files the grammar already failed and a valid file called invalid is
    /// booked as noise and hides a gap. libyaml — which the roadmap names
    /// for this language — rejects 51 of the suite's 308 valid cases.
    ///
    /// The stage is `parse`, the event stream, and that choice moves more
    /// verdicts than the choice of parser does: 73 of 3,217 real files
    /// change between this parser's parse and load stages (`!vault` tags,
    /// duplicate keys), against 0 between libyaml and go-yaml at the same
    /// stage. Neither a tag nor a duplicate key is a syntax property a
    /// tree-sitter grammar could be responsible for.
    fn validate(&self, srcroot: &Path, paths: &[String]) -> Result<HashMap<String, bool>> {
        stdin_oracle::run_node(Path::new("tools/yaml-oracle"), &[], srcroot, paths)
    }
}

/// A Go template action (`{{ … }}`) that is not a GitHub Actions expression
/// (`${{ … }}`), or a Jinja statement (`{% if … %}`).
///
/// The Jinja half is bash's function, keyword list and reasoning, kept
/// keyword-gated for the same measured reason: `{%` alone appears in real
/// documents that are not templates.
fn looks_like_a_template(content: &[u8]) -> bool {
    for i in 0..content.len().saturating_sub(1) {
        if &content[i..i + 2] != b"{{" {
            continue;
        }
        // `${{ … }}` is a GitHub Actions expression inside a YAML scalar,
        // not a template action standing where syntax goes.
        if i > 0 && content[i - 1] == b'$' {
            continue;
        }
        return true;
    }
    let mut i = 0;
    while let Some(off) = content[i..].windows(2).position(|w| w == b"{%") {
        let mut j = i + off + 2;
        // `{%-` is the whitespace-trimming form of the same tag.
        if content.get(j) == Some(&b'-') {
            j += 1;
        }
        while matches!(content.get(j), Some(b' ' | b'\t' | b'\r' | b'\n')) {
            j += 1;
        }
        let rest = &content[j..];
        if JINJA_KEYWORDS.iter().any(|kw| {
            rest.starts_with(kw.as_bytes())
                && !matches!(rest.get(kw.len()), Some(c) if c.is_ascii_alphanumeric() || *c == b'_')
        }) {
            return true;
        }
        i = i + off + 2;
    }
    false
}
