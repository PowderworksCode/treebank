// Two parsers, one crate. Each variant directory carries its own generated
// parser.c and a scanner.c stub that includes the shared scanner with the
// right symbol prefix (VARIANTS.md §2), so the two compile into separate
// static libraries with no symbol collision.
fn main() {
    for variant in ["python3", "python2"] {
        let src_dir = std::path::Path::new(variant).join("src");

        let mut c_config = cc::Build::new();
        c_config.std("c11").include(&src_dir);
        c_config
            .flag_if_supported("-Wno-unused-parameter")
            .flag_if_supported("-Wno-unused-but-set-variable")
            .flag_if_supported("-Wno-trigraphs");
        c_config.file(src_dir.join("parser.c"));
        c_config.file(src_dir.join("scanner.c"));
        c_config.compile(&format!("tree-sitter-{variant}"));

        println!("cargo:rerun-if-changed={variant}/src/parser.c");
        println!("cargo:rerun-if-changed={variant}/src/scanner.c");
    }
    println!("cargo:rerun-if-changed=common/scanner.c");
}
