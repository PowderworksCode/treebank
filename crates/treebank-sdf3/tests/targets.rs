//! Dialects and versions as modules: `hiding`, holes, and one exact parser
//! per target from one family source (notes/metagrammar.md §22).

use std::path::{Path, PathBuf};

fn sql(target: &str) -> PathBuf {
    Path::new(concat!(env!("CARGO_MANIFEST_DIR"), "/spike/sql")).join(format!("{target}.sdf3"))
}

fn constructors(m: &treebank_sdf3::ast::Module) -> Vec<String> {
    m.productions(false).filter_map(|p| p.reference()).collect()
}

#[test]
fn imports_resolve_against_the_family_root() {
    // postgres/9.4.sdf3 imports sql/core and postgres/base by family name,
    // not by path relative to postgres/.
    let m = treebank_sdf3::load_module(&sql("postgres/9.4")).unwrap();
    let c = constructors(&m);
    assert!(c.contains(&"Query.Select".to_string()), "sql/core reached");
    assert!(
        c.contains(&"Returning.Returning".to_string()),
        "postgres/base reached"
    );
    assert!(
        c.contains(&"Limit.Limit".to_string()),
        "sql/limit reached through postgres/base"
    );
}

#[test]
fn a_version_hides_one_constructor_and_the_next_imports_more() {
    let v95 = constructors(&treebank_sdf3::load_module(&sql("postgres/9.5")).unwrap());
    let v12 = constructors(&treebank_sdf3::load_module(&sql("postgres/12")).unwrap());
    let v15 = constructors(&treebank_sdf3::load_module(&sql("postgres/15")).unwrap());
    assert!(v95.contains(&"CreateTail.WithOids".to_string()));
    assert!(
        !v12.contains(&"CreateTail.WithOids".to_string()),
        "12 hides WITH OIDS"
    );
    assert!(
        v12.contains(&"CreateTail.WithoutOids".to_string()),
        "and keeps WITHOUT OIDS"
    );
    assert!(!v12.contains(&"Stmt.Merge".to_string()));
    assert!(v15.contains(&"Stmt.Merge".to_string()), "15 adds MERGE");
    assert!(
        !v15.contains(&"CreateTail.WithOids".to_string()),
        "and inherits the hiding"
    );
}

#[test]
fn a_fork_hides_a_whole_shared_module() {
    let mysql = constructors(&treebank_sdf3::load_module(&sql("mysql/5.7")).unwrap());
    let maria = constructors(&treebank_sdf3::load_module(&sql("mariadb/10.11")).unwrap());
    assert!(mysql.contains(&"Exp.Arrow".to_string()));
    assert!(
        !maria.contains(&"Exp.Arrow".to_string()) && !maria.contains(&"Exp.ArrowText".to_string())
    );
    assert!(maria.contains(&"With.With".to_string()), "and takes CTEs");
    // The priority line that named the hidden constructors went with them.
    let m = treebank_sdf3::load_module(&sql("mariadb/10.11")).unwrap();
    for chain in m.priorities() {
        for g in &chain.groups {
            assert!(!g.members.iter().any(|r| r == "Exp.Arrow"));
        }
    }
}

#[test]
fn hiding_nothing_is_an_error() {
    let dir = tempfile_dir();
    std::fs::write(
        dir.join("base.sdf3"),
        "module base\ncontext-free start-symbols S\nsorts S\ncontext-free syntax\n  S.A = <a>\n",
    )
    .unwrap();
    std::fs::write(
        dir.join("top.sdf3"),
        "module top\nimports base\nhiding S.Missing\n",
    )
    .unwrap();
    let err = treebank_sdf3::load_module(&dir.join("top.sdf3"))
        .unwrap_err()
        .to_string();
    assert!(err.contains("hides nothing"), "{err}");
}

#[test]
fn holes_close_optionals_and_drop_what_needs_them() {
    let dir = tempfile_dir();
    std::fs::write(
        dir.join("core.sdf3"),
        "module core\ncontext-free start-symbols S\nsorts S Tail Must\nlexical sorts ID\nlexical syntax\n  ID = [a-z]+\n  LAYOUT = [\\ \\n]\ncontext-free syntax\n  S.Plain = <x <ID> <tail:Tail?>>\n  S.Needs = <y <Must>>\n",
    )
    .unwrap();
    let m = treebank_sdf3::load_module(&dir.join("core.sdf3")).unwrap();
    let holes: Vec<(&str, &Vec<String>, &Vec<String>)> = m
        .holes
        .iter()
        .map(|h| (h.sort.as_str(), &h.blanked, &h.dropped))
        .collect();
    assert_eq!(holes.len(), 2, "{holes:?}");
    let tail = m.holes.iter().find(|h| h.sort == "Tail").unwrap();
    assert_eq!(tail.blanked, vec!["S.Plain"]);
    let must = m.holes.iter().find(|h| h.sort == "Must").unwrap();
    assert_eq!(must.dropped, vec!["S.Needs"]);
    assert_eq!(constructors(&m), vec!["S.Plain"]);
    // The lowering sees an ordinary module and says what was closed.
    let lowered = treebank_sdf3::lower(&m).unwrap();
    assert!(lowered
        .findings
        .iter()
        .any(|f| f.what.contains("sort Tail has no production")));
    // A filled hole is not a hole.
    std::fs::write(
        dir.join("ext.sdf3"),
        "module ext\nimports core\ncontext-free syntax\n  Tail.T = <t>\n",
    )
    .unwrap();
    let e = treebank_sdf3::load_module(&dir.join("ext.sdf3")).unwrap();
    assert_eq!(
        e.holes.iter().map(|h| h.sort.as_str()).collect::<Vec<_>>(),
        vec!["Must"]
    );
}

#[test]
fn every_sql_target_lowers_to_its_committed_grammar() {
    let root = Path::new(concat!(env!("CARGO_MANIFEST_DIR"), "/spike/sql"));
    let cfg: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(root.join("targets.json")).unwrap()).unwrap();
    for t in cfg["targets"].as_array().unwrap() {
        let t = t.as_str().unwrap();
        let out = root.join("targets").join(t.replace('/', "-"));
        let module = treebank_sdf3::load_module(&sql(t)).unwrap();
        let lowered = treebank_sdf3::lower_all(&module).unwrap().lowered;
        let mut grammar = lowered.grammar.clone();
        let conflicts = treebank_sdf3::read_conflicts(&out.join("tree-sitter.conflicts.json"))
            .unwrap()
            .unwrap_or_default();
        treebank_sdf3::apply_conflicts(&mut grammar, &conflicts);
        let committed = std::fs::read_to_string(out.join("grammar.json")).unwrap();
        assert_eq!(
            serde_json::to_string_pretty(&grammar).unwrap() + "\n",
            committed,
            "spike/sql/targets/{t}/grammar.json is stale; run spike/sql/verify.sh"
        );
        assert_eq!(grammar["name"], serde_json::json!(module.symbol_name()));
    }
}

#[test]
fn a_rejected_word_no_production_uses_is_still_reserved() {
    let m = treebank_sdf3::load_module(Path::new(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/spike/editions/rust/2024.sdf3"
    )))
    .unwrap();
    let lowered = treebank_sdf3::lower(&m).unwrap();
    let reserved: Vec<&str> = lowered.grammar["reserved"]["global"]
        .as_array()
        .unwrap()
        .iter()
        .filter_map(|v| v["value"].as_str())
        .collect();
    for w in ["async", "await", "dyn", "try", "gen"] {
        assert!(reserved.contains(&w), "{w} reserved in 2024");
    }
    assert!(lowered.grammar["rules"].get("_reserved_word").is_some());
    let m2015 = treebank_sdf3::load_module(Path::new(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/spike/editions/rust/2015.sdf3"
    )))
    .unwrap();
    let l2015 = treebank_sdf3::lower(&m2015).unwrap();
    assert!(l2015.grammar["rules"].get("_reserved_word").is_none());
}

fn tempfile_dir() -> PathBuf {
    let dir = std::env::temp_dir()
        .join(format!("treebank-sdf3-targets-{}", std::process::id()))
        .join(
            format!("{:?}", std::time::Instant::now())
                .replace(|c: char| !c.is_ascii_alphanumeric(), ""),
        );
    std::fs::create_dir_all(&dir).unwrap();
    dir
}
