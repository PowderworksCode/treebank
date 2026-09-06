//! The third backend: the committed `winnow/src/main.rs` of every spike is
//! what the winnow lowering emits, and the lowering's shape decisions hold.

use std::path::Path;

fn spike(name: &str) -> std::path::PathBuf {
    Path::new(concat!(env!("CARGO_MANIFEST_DIR"), "/spike")).join(name)
}

fn emitted(dir: &Path, module: &str) -> treebank_sdf3::winnow::Emitted {
    let m = treebank_sdf3::load_module(&dir.join(module)).unwrap();
    let lowered = treebank_sdf3::lower_all(&m).unwrap().lowered;
    treebank_sdf3::winnow::emit(&m, &lowered.names, &lowered.levels).unwrap()
}

#[test]
fn every_spike_lowers_to_its_committed_winnow_crate() {
    for name in ["mini", "rubyish", "cppish", "pyish", "rustish", "jsish"] {
        let dir = spike(name);
        let e = emitted(&dir, &format!("{name}.sdf3"));
        let committed = std::fs::read_to_string(dir.join("winnow/src/main.rs")).unwrap();
        assert_eq!(
            e.source, committed,
            "spike/{name}/winnow/src/main.rs is stale; run its verify.sh"
        );
        let toml = std::fs::read_to_string(dir.join("winnow/Cargo.toml")).unwrap();
        assert_eq!(e.cargo_toml, toml);
        assert_eq!(
            treebank_sdf3::report(&e.findings),
            std::fs::read_to_string(dir.join("winnow-findings.md")).unwrap(),
            "spike/{name}/winnow-findings.md is stale"
        );
    }
}

#[test]
fn operators_climb_and_non_assoc_is_exact() {
    let e = emitted(&spike("mini"), "mini.sdf3");
    // Infix productions are tails tried highest level first; the non-assoc
    // group blocks a second operator of its level.
    assert!(
        e.source.contains("if 3 >= min && block != Some(3)"),
        "mul at level 3 in the loop"
    );
    assert!(
        e.source.contains("block = if true { Some(1) }"),
        "eq/lt non-assoc at level 1"
    );
    assert!(e
        .findings
        .iter()
        .any(|f| f.what.contains("non-assoc") && f.what.contains("syntax error")));
    // A prefix operator parses its operand at its own level.
    assert!(
        e.source.contains("r_exp_prec(i, 4)"),
        "neg's operand at level 4"
    );
}

#[test]
fn layout_constraints_are_checked_in_place() {
    let r = emitted(&spike("rubyish"), "rubyish.sdf3");
    assert!(r.source.contains("col(i, sp[0].1.saturating_sub(1).max(sp[0].0)) as i64) + (1) == (col(i, sp[1].0) as i64)"), "1.last.col + 1 == 2.first.col");
    assert!(
        !r.findings.iter().any(|f| f.what.contains("scanner"))
            || r.findings
                .iter()
                .any(|f| f.what.contains("no variant, no scanner"))
    );
    let p = emitted(&spike("pyish"), "pyish.sdf3");
    assert!(
        p.source.contains("i.state.offside.push(c)"),
        "offside pushes a column limit"
    );
    assert!(
        p.source.contains("col0 = Some(c)"),
        "align-list checks the column in the loop"
    );
    assert!(
        p.source.contains("reach: line_end(i, end)"),
        "newline-terminated productions reach the line end for extras"
    );
}

#[test]
fn case_insensitive_keywords_are_caseless() {
    let dir = spike("sql");
    let m = treebank_sdf3::load_module(&dir.join("postgres/16.sdf3")).unwrap();
    let lowered = treebank_sdf3::lower_all(&m).unwrap().lowered;
    let e = treebank_sdf3::winnow::emit(&m, &lowered.names, &lowered.levels).unwrap();
    assert!(e.source.contains(r#"literal(Caseless("SELECT"))"#));
    assert!(e.source.contains("REJECT.iter().any(|k| eq_ci(k, text))"));
}
