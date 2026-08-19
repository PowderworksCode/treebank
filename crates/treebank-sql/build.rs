fn main() {
    let src_dir = std::path::Path::new("src");

    let mut c_config = cc::Build::new();
    c_config.std("c11").include(src_dir);
    c_config
        .flag_if_supported("-Wno-unused-parameter")
        .flag_if_supported("-Wno-unused-but-set-variable")
        .flag_if_supported("-Wno-trigraphs");
    // No scanner.c: nothing in this grammar's lexical surface needs one.
    // The two constructs that would — postgres's `$tag$ … $tag$` dollar
    // quoting and MySQL's `DELIMITER //` — are declared gaps in ledger.toml
    // rather than a scanner written before a corpus has asked for one.
    c_config.file(src_dir.join("parser.c"));
    c_config.compile("tree-sitter-sql");

    println!("cargo:rerun-if-changed=src/parser.c");
}
