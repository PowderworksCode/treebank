// The java oracle must judge the file that is on disk NOW.
//
// `fuzz` keeps one JVM for the whole run and asks about one derived program
// at a time, reusing the same scratch filename for every question.
// StandardJavaFileManager caches the file objects it hands out, keyed by
// path, so a manager shared across questions answered each new program with
// the previous program's verdict. Nothing failed loudly: the verdicts were
// merely wrong, which made `--seed` unreproducible and filled issues #187
// and #190 with programs that do not reproduce.
//
// This alternates a valid program with an invalid one on ONE path, which is
// exactly the shape that broke. Before the fix it returned 29 wrong verdicts
// in 80 rounds; the same loop with unique filenames returned none.
use std::collections::HashMap;
use std::path::Path;

use treebank_oracle::{get, LangName};

#[test]
fn reusing_a_filename_does_not_return_a_stale_verdict() {
    // Any JDK will do -- the two programs below are not version-sensitive.
    if std::process::Command::new("javac").arg("-version").output().is_err() {
        eprintln!("skipped: no javac on PATH");
        return;
    }

    let dir = std::env::temp_dir().join("tb-java-oracle-staleness");
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();

    let oracle = get(LangName::Java);
    let name = "probe.java".to_string();
    // `import LL ;` is a genuine PARSE error -- javac wants a qualified name
    // ("'.' expected"), so this does not depend on attribution.
    let cases: [(&str, bool); 2] = [("class LL { }\n", true), ("import LL ;\n", false)];

    let mut wrong: Vec<String> = Vec::new();
    for round in 0..20 {
        let (text, expected) = cases[round % cases.len()];
        std::fs::write(dir.join(&name), text).unwrap();
        let verdicts: HashMap<String, bool> =
            oracle.validate(Path::new(&dir), &[name.clone()]).unwrap();
        let got = verdicts.get(&name).copied();
        if got != Some(expected) {
            wrong.push(format!("round {round}: {text:?} expected {expected}, got {got:?}"));
        }
        let _ = std::fs::remove_file(dir.join(&name));
    }

    let _ = std::fs::remove_dir_all(&dir);
    assert!(wrong.is_empty(), "stale verdicts from a reused filename:\n{}", wrong.join("\n"));
}
