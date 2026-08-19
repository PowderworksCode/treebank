// Does `has_error()` see a tree whose only defect is a MISSING token?
// The sweep's pass/fail verdict rides on this call, and `tree-sitter
// parse`'s exit code disagreed with it on 3,116 bash corpus files.
use std::process::Command;

#[test]
fn missing_token_counts_as_error() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .parent()
        .unwrap()
        .to_path_buf();
    let dir = root.join("crates/treebank-bash/src");
    let dylib = std::env::temp_dir().join("tb-missing-probe.so");
    let status = Command::new("cc")
        .args(["-fPIC", "-shared", "-O0", "-I"])
        .arg(&dir)
        .arg(dir.join("parser.c"))
        .arg(dir.join("scanner.c"))
        .arg("-o")
        .arg(&dylib)
        .status()
        .unwrap();
    assert!(status.success());
    let lang = unsafe {
        let lib = libloading::Library::new(&dylib).unwrap();
        let f: libloading::Symbol<unsafe extern "C" fn() -> tree_sitter::Language> =
            lib.get(b"tree_sitter_bash").unwrap();
        let l = f();
        std::mem::forget(lib);
        l
    };
    let mut p = tree_sitter::Parser::new();
    p.set_language(&lang).unwrap();
    // The corpus shape that produced `(MISSING heredoc_start)`.
    let src = std::fs::read(root.join("corpus/bash/src/basecamp__omarchy-fa955bfa9d2c/test/cli"))
        .unwrap_or_else(|_| b"cat <<'EOF\nx\n".to_vec());
    let tree = p.parse(&src, None).unwrap();
    let sexp = tree.root_node().to_sexp();
    let has_missing = sexp.contains("MISSING");
    eprintln!("has_missing={has_missing} has_error={}", tree.root_node().has_error());
    assert_eq!(has_missing, tree.root_node().has_error());
}
