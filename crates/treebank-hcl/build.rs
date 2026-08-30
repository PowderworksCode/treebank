fn main() {
    let src_dir = std::path::Path::new("src");

    let mut c_config = cc::Build::new();
    c_config.std("c11").include(src_dir);
    c_config
        .flag_if_supported("-Wno-unused-parameter")
        .flag_if_supported("-Wno-unused-but-set-variable")
        .flag_if_supported("-Wno-trigraphs");
    c_config.file(src_dir.join("parser.c"));
    // The scanner carries three decisions the table cannot: where a
    // newline is a terminator rather than trivia, which template mode is
    // on top of the stack, and where a heredoc's delimiter line is. See
    // the header of src/scanner.c.
    c_config.file(src_dir.join("scanner.c"));
    c_config.compile("tree-sitter-hcl");

    println!("cargo:rerun-if-changed=src/parser.c");
    println!("cargo:rerun-if-changed=src/scanner.c");
}
