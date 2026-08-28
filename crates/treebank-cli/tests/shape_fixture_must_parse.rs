// A fixture that stops parsing must FAIL the shape gate, not vanish from
// it. `shape` skips files it cannot parse -- comparing shapes against an
// error tree is noise, and over a corpus those failures belong to the
// sweep. A fixture directory has no sweep behind it: CI has no corpus and
// shape is the only check that reads those files. So a fixture that
// regressed into an ERROR would leave the measured set in silence while
// the zero ceiling kept reporting green over the hole.
//
// The input here is malformed for BOTH parsers on purpose. Using a
// construct only our grammar rejects would make this test a hostage of
// whichever gap it borrowed -- it would start failing the day that gap was
// closed, which is the day the grammar got better.
use std::process::Command;

#[test]
fn an_unparseable_fixture_fails_the_gate() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .parent()
        .unwrap()
        .to_path_buf();

    // The java span oracle is javac through the single-file source
    // launcher. Without a JDK there is no oracle and nothing to assert.
    if Command::new("javac").arg("-version").output().is_err() {
        eprintln!("skipped: no javac on PATH");
        return;
    }

    let dir = std::env::temp_dir().join("tb-shape-fixture-must-parse");
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(dir.join("Fine.java"), "class Fine { int a = 1; }\n").unwrap();

    let grammar = root.join("crates/treebank-java");
    let out = dir.join("shape.json");
    let run = || {
        Command::new(env!("CARGO_BIN_EXE_treebank"))
            .args(["shape", "--lang", "java", "--grammar"])
            .arg(&grammar)
            .arg("--dir")
            .arg(&dir)
            .arg("--out")
            .arg(&out)
            .output()
            .unwrap()
    };

    // A directory whose files all parse is the green case.
    let clean = run();
    assert!(
        clean.status.success(),
        "a fixture directory that parses should pass: {}",
        String::from_utf8_lossy(&clean.stderr)
    );

    // Now add one neither parser can read.
    std::fs::write(dir.join("Malformed.java"), "class Malformed { void m( { }\n").unwrap();
    let broken = run();
    let stderr = String::from_utf8_lossy(&broken.stderr);
    assert!(!broken.status.success(), "an unparseable fixture must fail the gate: {stderr}");
    assert!(
        stderr.contains("Malformed.java"),
        "the failure must name the file that could not be parsed: {stderr}"
    );

    let _ = std::fs::remove_dir_all(&dir);
}
