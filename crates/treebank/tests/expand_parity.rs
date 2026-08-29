//! The browser's expander must agree with this crate's, exactly.
//!
//! site/src/expand.mjs is a hand port of src/expand.rs, because the
//! playground runs in a browser and this crate does not. Two implementations
//! of one rewrite is a standing invitation to drift, and drift here is
//! particularly nasty: a query would mean one thing on treebank.dev and
//! another in the consumer's build, with no error to say so. A previous port
//! on this site diverged over HTML escaping and was only caught by a
//! differential, so this one ships with the differential from the start.
//!
//! Every grammar's real facets, crossed with a corpus of queries chosen for
//! the places a rewriter goes wrong: strings that contain facet names,
//! comments, nesting, unicode, and inputs that must fail.
//!
//! Skipped when node is absent, which is the same bargain the pack tests
//! make about packs: a missing tool is not a broken port.

use std::collections::BTreeMap;
use std::io::Write;
use std::path::PathBuf;
use std::process::{Command, Stdio};

fn root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn have_node() -> bool {
    Command::new("node")
        .arg("--version")
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .map(|s| s.success())
        .unwrap_or(false)
}

/// Every grammar in the repository, by the facets it actually declares.
fn grammars() -> Vec<(String, BTreeMap<String, Vec<String>>)> {
    let mut out = Vec::new();
    let crates = root().join("crates");
    let Ok(entries) = std::fs::read_dir(&crates) else { return out };
    let mut dirs: Vec<_> = entries.filter_map(|e| e.ok()).map(|e| e.path()).collect();
    dirs.sort();
    for dir in dirs {
        let manifest = dir.join("roles.json");
        if !manifest.is_file() {
            continue;
        }
        let Ok(text) = std::fs::read_to_string(&manifest) else { continue };
        let Ok(roles) = serde_json::from_str::<treebank::roles::RolesManifest>(&text) else {
            continue;
        };
        let name = dir.file_name().unwrap().to_string_lossy().replace("treebank-", "");
        out.push((name, roles.facets));
    }
    out
}

/// `{facet}` is replaced with a facet this grammar really has, so the corpus
/// exercises each grammar's own vocabulary rather than a made-up one.
const CORPUS: &[&str] = &[
    // The ordinary shapes.
    "({facet})",
    "({facet}) @hit",
    "({facet} name: (identifier) @n)",
    "({facet}) ({facet})",
    // Nesting: a facet inside a facet must resolve inside out.
    "({facet} body: ({facet}))",
    "(x ({facet}) (y ({facet})))",
    // A facet name inside a string is text, not a pattern.
    "\"({facet})\"",
    "(x \"({facet})\" ({facet}))",
    "(x \"escaped \\\" quote ({facet})\" ({facet}))",
    // Comments run to end of line and are copied verbatim.
    "; ({facet})\n({facet})",
    "({facet}) ; trailing ({facet})",
    "(x ; ) not a close\n  ({facet}))",
    // Things that are not facets are left alone.
    "(_declaration)",
    "(identifier) @id",
    "[(a) (b)] @alt",
    "(x (#match? @a \"({facet})\"))",
    // Anchors, wildcards, negation, quantifiers.
    "({facet} . (a) (b))",
    "(_ ({facet}))",
    "({facet} !name)",
    "({facet} (a)? (b)*)",
    // Whitespace and layout are preserved as-is.
    "(  {facet}  )",
    "({facet}\n  name: (a)\n)",
    // Unicode, where Rust chars and JS code units disagree if anyone is sloppy.
    "\"héllo ({facet})\" ({facet})",
    "; héllo\n({facet})",
    "(x \"日本語\" ({facet}))",
    // Must fail, and must fail on both sides.
    "({facet}",
    "(x \"unterminated",
    "({facet}))",
    // Degenerate but legal.
    "",
    "()",
    "(",
    ";",
    "\"\"",
];

#[derive(serde::Serialize)]
struct Case<'a> {
    query: String,
    facets: &'a BTreeMap<String, Vec<String>>,
}

#[derive(serde::Deserialize)]
struct Answer {
    ok: bool,
    #[serde(default)]
    value: String,
    #[serde(default)]
    error: String,
}

#[test]
fn the_browsers_expander_agrees_with_this_one() {
    if !have_node() {
        eprintln!("node not on PATH; skipping the expand parity differential");
        return;
    }
    let grammars = grammars();
    assert!(!grammars.is_empty(), "no roles.json found; the corpus would be vacuous");

    // Build every case first, so node is started once.
    let mut cases: Vec<Case> = Vec::new();
    let mut labels: Vec<(String, String)> = Vec::new();
    for (grammar, facets) in &grammars {
        let Some(facet) = facets.keys().next() else { continue };
        for template in CORPUS {
            let query = template.replace("{facet}", facet);
            labels.push((grammar.clone(), query.clone()));
            cases.push(Case { query, facets });
        }
        // Also every facet this grammar has, on its own, so a grammar with an
        // odd member list is covered rather than only its alphabetical first.
        for name in facets.keys() {
            let query = format!("({name} name: (a) @n)");
            labels.push((grammar.clone(), query.clone()));
            cases.push(Case { query, facets });
        }
    }

    let payload = serde_json::to_string(&cases).expect("serialising cases");
    let mut child = Command::new("node")
        .arg(root().join("site/tools/expand-parity.mjs"))
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::inherit())
        .spawn()
        .expect("spawning node");
    child.stdin.take().unwrap().write_all(payload.as_bytes()).expect("writing cases");
    let output = child.wait_with_output().expect("running the parity driver");
    assert!(output.status.success(), "the parity driver failed");

    let answers: Vec<Answer> =
        serde_json::from_slice(&output.stdout).expect("parsing the driver's answers");
    assert_eq!(answers.len(), cases.len(), "driver answered a different number of cases");

    let mut differences = Vec::new();
    for (i, case) in cases.iter().enumerate() {
        let (grammar, query) = &labels[i];
        let mine = treebank::expand::expand(&case.query, case.facets);
        let theirs = &answers[i];
        match (&mine, theirs.ok) {
            (Ok(rust), true) => {
                if *rust != theirs.value {
                    differences.push(format!(
                        "{grammar}: {query:?}\n    rust: {rust:?}\n    js:   {:?}",
                        theirs.value
                    ));
                }
            }
            // Both reject it. The messages are allowed to differ; whether the
            // query is usable is what a caller acts on.
            (Err(_), false) => {}
            (Ok(rust), false) => differences.push(format!(
                "{grammar}: {query:?}\n    rust accepted: {rust:?}\n    js rejected:  {}",
                theirs.error
            )),
            (Err(e), true) => differences.push(format!(
                "{grammar}: {query:?}\n    rust rejected: {e}\n    js accepted:  {:?}",
                theirs.value
            )),
        }
    }

    assert!(
        differences.is_empty(),
        "{} of {} cases differ between src/expand.rs and site/src/expand.mjs:\n\n{}",
        differences.len(),
        cases.len(),
        differences.join("\n\n")
    );
    eprintln!("{} cases agree across {} grammars", cases.len(), grammars.len());
}
