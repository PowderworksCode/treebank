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

/// Whether a skip here is a bug rather than a missing tool.
///
/// The header above says this file makes the same bargain the pack tests make
/// about packs. That bargain has two halves: skip when the tool is absent, and
/// fail where a job promised it. Only the first was here, so a differential
/// that compared nothing reported a pass -- the shape the pack tests already
/// closed with TREEBANK_REQUIRE_PACK.
fn node_is_required() -> bool {
    std::env::var_os("TREEBANK_REQUIRE_NODE").is_some_and(|v| v != "0" && !v.is_empty())
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

/// Every grammar in the repository: the facets it declares, and the node
/// manifest the filtering reads.
fn grammars() -> Vec<(String, BTreeMap<String, Vec<String>>, Option<String>)> {
    let mut out = Vec::new();
    let crates = root().join("crates");
    let Ok(entries) = std::fs::read_dir(&crates) else {
        return out;
    };
    let mut dirs: Vec<_> = entries.filter_map(|e| e.ok()).map(|e| e.path()).collect();
    dirs.sort();
    for dir in dirs {
        let manifest = dir.join("roles.json");
        if !manifest.is_file() {
            continue;
        }
        let Ok(text) = std::fs::read_to_string(&manifest) else {
            continue;
        };
        let Ok(roles) = serde_json::from_str::<treebank::roles::RolesManifest>(&text) else {
            continue;
        };
        let name = dir
            .file_name()
            .unwrap()
            .to_string_lossy()
            .replace("treebank-", "");
        let node_types = std::fs::read_to_string(dir.join("src/node-types.json")).ok();
        out.push((name, roles.facets, node_types));
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
    // Field constraints, which is what member filtering turns on. A member
    // that cannot take the field must be dropped, and dropping all of them is
    // an error rather than an empty alternation.
    "({facet} body: (block))",
    "({facet} name: (_) @n)",
    "({facet} name: [(identifier) (attribute)])",
    "({facet} name: [(_) (identifier)])",
    "({facet} name: (_) body: (_))",
    "({facet} name: (identifier) body: (block))",
    "({facet} nonexistent_field: (a))",
    // A field whose value names no node type at all, so the constraint is
    // "present" rather than "of these types". A mutant that treated an empty
    // constraint as unsatisfiable survived the corpus without these.
    "({facet} name: _)",
    "({facet} name: \"literal\")",
    "({facet} name: _ body: (block))",
    "({facet} body: _)",
    "({facet} name: (nonexistent_type))",
    "(x ({facet} name: (identifier)))",
    "({facet} body: ({facet}))",
    "({facet} (#eq? @a \"name: (x)\"))",
    "({facet} name: (identifier) @n (#match? @n \"^_\"))",
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
struct Grammar<'a> {
    facets: &'a BTreeMap<String, Vec<String>>,
    node_types: Option<&'a str>,
}

#[derive(serde::Serialize)]
struct Case {
    grammar: usize,
    query: String,
    /// Whether to expand WITH node-types. Both modes are compared, because
    /// `expand` and `expand_with_types` are both public and a consumer can
    /// reach either.
    filtered: bool,
}

#[derive(serde::Serialize)]
struct Payload<'a> {
    grammars: Vec<Grammar<'a>>,
    cases: &'a [Case],
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
        assert!(
            !node_is_required(),
            "TREEBANK_REQUIRE_NODE is set: node is not on PATH, so the expand \
             parity differential compared nothing"
        );
        eprintln!("node not on PATH; skipping the expand parity differential");
        return;
    }
    let grammars = grammars();
    assert!(
        !grammars.is_empty(),
        "no roles.json found; the corpus would be vacuous"
    );
    assert!(
        grammars.iter().any(|(_, _, nt)| nt.is_some()),
        "no node-types.json found; the filtering half would not be exercised"
    );

    // Build every case first, so node is started once.
    let mut cases: Vec<Case> = Vec::new();
    let mut labels: Vec<(String, String, bool)> = Vec::new();
    for (index, (grammar, facets, node_types)) in grammars.iter().enumerate() {
        let Some(facet) = facets.keys().next() else {
            continue;
        };
        // Both modes: without node-types (what `expand` does) and with them
        // (what `Pack::expand_query` now does).
        let modes: &[bool] = if node_types.is_some() {
            &[false, true]
        } else {
            &[false]
        };
        for &filtered in modes {
            for template in CORPUS {
                let query = template.replace("{facet}", facet);
                labels.push((grammar.clone(), query.clone(), filtered));
                cases.push(Case {
                    grammar: index,
                    query,
                    filtered,
                });
            }
            // Every facet on its own, with a field constraint, so a grammar
            // with an odd member list is covered rather than only its first.
            for name in facets.keys() {
                for query in [format!("({name})"), format!("({name} name: (a) @n)")] {
                    labels.push((grammar.clone(), query.clone(), filtered));
                    cases.push(Case {
                        grammar: index,
                        query,
                        filtered,
                    });
                }
            }
        }
    }

    let payload = Payload {
        grammars: grammars
            .iter()
            .map(|(_, facets, nt)| Grammar {
                facets,
                node_types: nt.as_deref(),
            })
            .collect(),
        cases: &cases,
    };
    // The driver reads `nodeTypes`; serde would send `node_types`.
    let json = serde_json::to_value(&payload).expect("serialising");
    let json = {
        let mut v = json;
        if let Some(gs) = v.get_mut("grammars").and_then(|g| g.as_array_mut()) {
            for g in gs {
                if let Some(nt) = g.as_object_mut().and_then(|o| o.remove("node_types")) {
                    g.as_object_mut().unwrap().insert("nodeTypes".into(), nt);
                }
            }
        }
        serde_json::to_string(&v).expect("serialising")
    };

    let mut child = Command::new("node")
        .arg(root().join("site/tools/expand-parity.mjs"))
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::inherit())
        .spawn()
        .expect("spawning node");
    child
        .stdin
        .take()
        .unwrap()
        .write_all(json.as_bytes())
        .expect("writing cases");
    let output = child.wait_with_output().expect("running the parity driver");
    assert!(output.status.success(), "the parity driver failed");

    let answers: Vec<Answer> =
        serde_json::from_slice(&output.stdout).expect("parsing the driver's answers");
    assert_eq!(
        answers.len(),
        cases.len(),
        "driver answered a different number of cases"
    );

    // Parse each grammar's node-types once, as the crate does.
    let parsed: Vec<Option<treebank::node_types::NodeTypes>> = grammars
        .iter()
        .map(|(_, _, nt)| {
            nt.as_deref()
                .and_then(|j| treebank::node_types::NodeTypes::parse(j).ok())
        })
        .collect();

    let mut differences = Vec::new();
    let mut filtered_cases = 0;
    for (i, case) in cases.iter().enumerate() {
        let (grammar, query, filtered) = &labels[i];
        if *filtered {
            filtered_cases += 1;
        }
        let facets = &grammars[case.grammar].1;
        let types = if *filtered {
            parsed[case.grammar].as_ref()
        } else {
            None
        };
        let mine = treebank::expand::expand_with_types(&case.query, facets, types);
        let theirs = &answers[i];
        let mode = if *filtered { "filtered" } else { "plain" };
        match (&mine, theirs.ok) {
            (Ok(rust), true) => {
                if *rust != theirs.value {
                    differences.push(format!(
                        "{grammar} [{mode}]: {query:?}\n    rust: {rust:?}\n    js:   {:?}",
                        theirs.value
                    ));
                }
            }
            // Both reject it. The messages are allowed to differ; whether the
            // query is usable is what a caller acts on.
            (Err(_), false) => {}
            (Ok(rust), false) => differences.push(format!(
                "{grammar} [{mode}]: {query:?}\n    rust accepted: {rust:?}\n    js rejected:  {}",
                theirs.error
            )),
            (Err(e), true) => differences.push(format!(
                "{grammar} [{mode}]: {query:?}\n    rust rejected: {e}\n    js accepted:  {:?}",
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
    eprintln!(
        "{} cases agree across {} grammars ({filtered_cases} with node-types filtering)",
        cases.len(),
        grammars.len()
    );
}
