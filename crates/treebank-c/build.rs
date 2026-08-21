fn main() {
    let src_dir = std::path::Path::new("src");

    let mut c_config = cc::Build::new();
    c_config.std("c11").include(src_dir);
    c_config
        .flag_if_supported("-Wno-unused-parameter")
        .flag_if_supported("-Wno-unused-but-set-variable")
        .flag_if_supported("-Wno-trigraphs");
    // No scanner.c. Every token C has is regular: the preprocessor's
    // line-oriented tails, the string prefixes and the pp-number are all
    // expressible as tokens, and the one construct that would need a
    // hand-written lexer — a raw string with a user-chosen delimiter — is
    // C++'s, not C's. It lives in treebank-cpp's scanner instead.
    c_config.file(src_dir.join("parser.c"));
    c_config.compile("tree-sitter-c");

    println!("cargo:rerun-if-changed=src/parser.c");
}
