use std::path::Path;

use anyhow::{bail, Result};

use crate::rank::RankedCrate;
use crate::{github, python, Ecosystem};
use treebank_lang::LangName;

pub struct Yaml;

/// **YAML is a guest language, more completely than shell is.** Nobody
/// publishes a YAML package; there is no registry, no download count, and
/// nothing that could be ranked. Every corpus for it is therefore somebody
/// else's corpus with the YAML taken out of it, and the only decision worth
/// making is whose.
///
/// GitHub's own classification is the measurement that settles how the two
/// sources here divide the world. Linguist marks YAML as `data` and leaves
/// data languages out of a repository's language statistics, so
/// `language:YAML` matches only repositories that contain almost nothing
/// else — 2,107 of them in total when this was written, against tens of
/// millions of repositories that carry YAML. "The top YAML repositories" is
/// consequently not a sample of where YAML lives; it is a sample of the one
/// place YAML is the whole product.
///
/// That place is real and it is `github` below: GitOps cluster
/// repositories, Ansible collections, Home Assistant configurations,
/// machine-generated manifest sets. It is dense, it is enormous, and it is
/// syntactically narrow — plain mappings, sequences and the occasional
/// block scalar, written by templates as often as by people.
///
/// `pypi` is the other half and answers the other question: what does YAML
/// look like when it ships INSIDE software written in something else. A
/// source distribution carries the workflows, the pre-commit configuration,
/// the conda recipe, the OpenAPI document and the test fixtures, and those
/// are where anchors, explicit keys, multi-document streams and tags
/// actually occur. It is a far smaller corpus per package — many top
/// packages carry no YAML at all — and a far wider one per file.
///
/// Neither is a popularity ranking of YAML, because no such thing exists.
/// `rank` writes `db/source.json` so the ledger can say which one a number
/// came from, the way bash's and zig's do.
#[derive(Clone, Copy, PartialEq)]
enum Source {
    Github,
    Pypi,
}

fn source() -> Result<Source> {
    match std::env::var("TREEBANK_YAML_CORPUS").as_deref() {
        Ok("pypi") => Ok(Source::Pypi),
        Ok("github") | Err(_) => Ok(Source::Github),
        Ok(other) => bail!("TREEBANK_YAML_CORPUS={other:?}: expected \"github\" or \"pypi\""),
    }
}

/// The extensions this grammar's own `tree-sitter.json` claims.
const YAML_EXTENSIONS: [&str; 2] = ["yaml", "yml"];

/// Directories whose contents belong to somebody else's package. The same
/// reasoning javascript's bundle exclusion and python's `_vendor` exclusion
/// use: a failure there is attributed to the wrong package, and the file is
/// already in the corpus under the package that owns it.
const VENDORED: [&str; 4] = ["node_modules", "vendor", "third_party", ".git"];

impl Ecosystem for Yaml {
    fn name(&self) -> LangName {
        LangName::Yaml
    }

    fn rank(&self, db: &Path, k: usize) -> Result<Vec<RankedCrate>> {
        let (ranked, name, note) = match source()? {
            Source::Github => (
                github::rank(LangName::Yaml, "YAML", k)?,
                "github",
                "repositories GitHub classifies as YAML, by stars. Linguist calls \
                 YAML a data language and leaves it out of repository language \
                 statistics, so this selects the small population of repositories \
                 that are ALMOST NOTHING BUT YAML — cluster manifests, Ansible \
                 collections, appliance configuration — rather than a sample of \
                 where YAML lives.",
            ),
            Source::Pypi => (
                python::rank_pypi(k)?,
                "pypi",
                "the source distributions of the most-downloaded PyPI packages, for \
                 the YAML they carry: workflows, pre-commit configuration, conda \
                 recipes, OpenAPI documents, test fixtures. YAML as it ships inside \
                 software written in something else, which is where the constructs a \
                 manifest never uses are found.",
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
            Source::Github => github::resolve(LangName::Yaml, pkg),
            Source::Pypi => python::resolve_sdist(pkg),
        }
    }

    fn classify(&self, rel: &Path) -> Option<Option<String>> {
        if rel.components().any(|c| {
            c.as_os_str()
                .to_str()
                .is_some_and(|s| VENDORED.contains(&s))
        }) {
            return None;
        }
        let ext = rel.extension()?.to_str()?;
        YAML_EXTENSIONS.contains(&ext).then_some(None)
    }

    /// The extension says a file claims to be YAML. Whether it IS one is a
    /// second question, and for YAML it is the whole question, because the
    /// single largest body of `.yaml` files in the world is Helm charts and
    /// a Helm chart is not YAML.
    ///
    /// `key: {{ .Values.image }}` IS valid YAML — the braces lex as nested
    /// flow mappings and every implementation reads it — so interpolation
    /// alone cannot be the test, exactly as bash found for Jinja. What
    /// cannot be YAML is a template STATEMENT: a `{{- if … }}` or `{% for … %}`
    /// standing where a node has to be, or a Go-template trim marker
    /// anywhere at all, since `{{-` and `-}}` have no YAML reading.
    ///
    /// Leaving them in would not merely add noise. A template that renders
    /// to YAML is judged by the oracle on its own text, and a two-valued
    /// oracle that rejects it books our failure as noise rather than as a
    /// gap — which is the harmless direction — but one that happens to
    /// ACCEPT it books our failure as a grammar gap. Both are measurements
    /// of the wrong thing.
    fn admit(&self, rel: &Path, content: &[u8]) -> bool {
        let _ = rel;
        if content.contains(&0) {
            return false;
        }
        !looks_like_a_template(content)
    }

    /// 250 MB, for the reason bash's cap exists: a guest language's
    /// artifact size is decided by its host, and a handful of enormous
    /// repositories would otherwise be most of the download for a
    /// vanishing share of the files.
    fn max_artifact_bytes(&self) -> Option<u64> {
        Some(250_000_000)
    }
}

/// Does this file carry a template STATEMENT rather than a template
/// interpolation?
///
/// Two tests, and both are about text that has no YAML reading at all:
///
///   * a Go-template trim marker, `{{-` or `-}}`. Helm's own style guide
///     uses these on nearly every control line, and `{{-` cannot begin a
///     flow mapping.
///   * a line whose first non-blank characters open a template tag —
///     `{{`, `{%`. Inside a line, `{{ x }}` is a legal (if odd) nested flow
///     mapping and is left alone; at the head of a line, where YAML wants a
///     key or an entry indicator, it is a control statement.
fn looks_like_a_template(content: &[u8]) -> bool {
    if content.windows(3).any(|w| w == b"{{-" || w == b"-}}") {
        return true;
    }
    for line in content.split(|&b| b == b'\n') {
        let line = line
            .iter()
            .position(|b| !matches!(b, b' ' | b'\t'))
            .map_or(&[][..], |i| &line[i..]);
        if line.starts_with(b"{{") || line.starts_with(b"{%") {
            return true;
        }
    }
    false
}

#[cfg(test)]
mod tests {
    use super::looks_like_a_template;

    #[test]
    fn interpolation_is_not_a_statement() {
        // Ansible writes this by the thousand and every YAML parser reads
        // it; excluding it would throw away most of the corpus.
        assert!(!looks_like_a_template(b"image: {{ ansible_hostname }}\n"));
        assert!(!looks_like_a_template(b"a: 1\nb: [x, y]\n"));
    }

    #[test]
    fn helm_control_lines_are_templates() {
        assert!(looks_like_a_template(
            b"{{- if .Values.enabled }}\nkind: Pod\n"
        ));
        assert!(looks_like_a_template(b"  {% for x in xs %}\n- {{ x }}\n"));
        assert!(looks_like_a_template(b"name: {{ .Chart.Name -}}\n"));
    }
}
