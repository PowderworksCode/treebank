fn main() {
    let src_dir = std::path::Path::new("src");

    let mut c_config = cc::Build::new();
    c_config.std("c11").include(src_dir);
    c_config
        .flag_if_supported("-Wno-unused-parameter")
        .flag_if_supported("-Wno-unused-but-set-variable")
        .flag_if_supported("-Wno-trigraphs");
    // No scanner.c, and unlike most grammars here that is not a milestone
    // the grammar has yet to reach: JSON's tokenizer has no state to carry.
    // There is no indentation, no delimiter to remember, no keyword that is
    // sometimes an identifier — there are no identifiers — and no construct
    // whose lexing depends on what came before it. Every token is a regular
    // language over bytes, so the generated table is the whole parser.
    c_config.file(src_dir.join("parser.c"));
    c_config.compile("tree-sitter-json");

    println!("cargo:rerun-if-changed=src/parser.c");
}
