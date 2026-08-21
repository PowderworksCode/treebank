fn main() {
    let src_dir = std::path::Path::new("src");

    let mut c_config = cc::Build::new();
    c_config.std("c11").include(src_dir);
    c_config
        .flag_if_supported("-Wno-unused-parameter")
        .flag_if_supported("-Wno-unused-but-set-variable")
        .flag_if_supported("-Wno-trigraphs");
    // Ruby needs one: every delimited literal, heredocs (whose bodies begin
    // after the line their operator is on), newline statement boundaries,
    // and the spacing-sensitive operator readings (`a * b` vs `foo *args`).
    c_config.file(src_dir.join("parser.c"));
    c_config.file(src_dir.join("scanner.c"));
    c_config.compile("tree-sitter-ruby");

    println!("cargo:rerun-if-changed=src/parser.c");
    println!("cargo:rerun-if-changed=src/scanner.c");
}
