// A fixture that stops parsing must FAIL the shape gate, not vanish from
// it. `shape` skips files it cannot parse -- comparing shapes against an
// error tree is noise, and over a corpus those failures belong to the
// sweep. A fixture directory has no sweep behind it: CI has no corpus and
// shape is the only check that reads those files. So a fixture that
// regressed into an ERROR would leave the measured set in silence while
// the zero ceiling kept reporting green over the hole.
//
// This runs against the RUST grammar, whose span oracle is `syn` in
// process -- no subprocess, no JDK, no node_modules, no libclang. The
// property under test belongs to `shape` itself and is language-neutral,
// so the test should not import a toolchain dependency into
// `cargo test --workspace`. The first draft used java and did exactly
// that: the workspace job pins no JDK, and the java oracle skipped every
// file there while passing locally.
//
// The malformed input is rejected by BOTH parsers on purpose. A construct
// only our grammar rejects would make this test a hostage of whichever gap
// it borrowed -- it would start failing the day that gap was closed, which
// is the day the grammar got better.
use std::process::Command;

#[test]
fn an_unparseable_fixture_fails_the_gate() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .parent()
        .unwrap()
        .to_path_buf();

    let dir = std::env::temp_dir().join("tb-shape-fixture-must-parse");
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(dir.join("fine.rs"), "fn main() {\n    let a = 1;\n}\n").unwrap();

    let grammar = root.join("crates/treebank-rust");
    let out = dir.join("shape.json");
    let run = || {
        Command::new(env!("CARGO_BIN_EXE_treebank"))
            .args(["shape", "--lang", "rust", "--grammar"])
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
        "a fixture directory that parses should pass:\n{}",
        String::from_utf8_lossy(&clean.stderr)
    );

    // Now add one neither parser can read.
    std::fs::write(dir.join("malformed.rs"), "fn main( {\n").unwrap();
    let broken = run();
    let stderr = String::from_utf8_lossy(&broken.stderr);
    assert!(!broken.status.success(), "an unparsable fixture must fail the gate:\n{stderr}");
    assert!(
        stderr.contains("malformed.rs"),
        "the failure must name the file that could not be read:\n{stderr}"
    );
    // ...and say why, so the next person does not have to bisect CI for it.
    assert!(
        stderr.contains("our parse has an ERROR node")
            || stderr.contains("reference parser:")
            || stderr.contains("our parser returned no tree"),
        "the failure must name the cause:\n{stderr}"
    );

    let _ = std::fs::remove_dir_all(&dir);
}
