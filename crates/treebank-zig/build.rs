fn main() {
    let src_dir = std::path::Path::new("src");

    let mut c_config = cc::Build::new();
    c_config.std("c11").include(src_dir);
    c_config
        .flag_if_supported("-Wno-unused-parameter")
        .flag_if_supported("-Wno-unused-but-set-variable")
        .flag_if_supported("-Wno-trigraphs");
    // No scanner.c, and for once that is a property of the language rather
    // than of how far the grammar has got. Zig's tokenizer is a hand-written
    // DFA over a fixed token set with no context in it at all: no
    // indentation, no raw-string delimiter to remember, no regex-vs-division
    // decision, no template nesting. The one construct that looks like it
    // needs state — the multiline string — is a per-LINE token (`\\` to end
    // of line) that the parse table repeats, so a regular token expresses
    // it exactly.
    c_config.file(src_dir.join("parser.c"));
    c_config.compile("tree-sitter-zig");

    println!("cargo:rerun-if-changed=src/parser.c");
}
