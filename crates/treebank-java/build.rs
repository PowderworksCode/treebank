fn main() {
    let src_dir = std::path::Path::new("src");

    let mut c_config = cc::Build::new();
    c_config.std("c11").include(src_dir);
    c_config
        .flag_if_supported("-Wno-unused-parameter")
        .flag_if_supported("-Wno-unused-but-set-variable")
        .flag_if_supported("-Wno-trigraphs");
    // No scanner.c: java needs no external scanner. Every token it has is
    // regular, including the text block, which the `"""…"""` rule takes as
    // one token precisely so that no hand-written lexer is required.
    c_config.file(src_dir.join("parser.c"));
    c_config.compile("tree-sitter-java");

    println!("cargo:rerun-if-changed=src/parser.c");
}
