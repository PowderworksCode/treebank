// rustc lexes 'a as a single token; a space after the quote is not a lifetime.
// Upstream tree-sitter-rust accepted this; patch 0003 makes it a parse error.
fn f<' a>(x: &' a str) -> &' a str {
    x
}
