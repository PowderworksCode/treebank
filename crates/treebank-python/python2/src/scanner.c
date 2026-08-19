// The scanner is shared across the Python variants; this stub is what
// `tree-sitter generate` and `build.rs` expect to find in a grammar's own
// `src/`.
//
// TREEBANK_PYTHON2 selects the py2 string-prefix rules — `ur` is legal,
// `f` is not. TREEBANK_SCANNER_PREFIX renames the external-scanner entry
// points to match this grammar's name, which is what lets one scanner
// source serve two parsers. See ../../common/scanner.c.
#define TREEBANK_PYTHON2 1
#define TREEBANK_SCANNER_PREFIX tree_sitter_python2
#include "../../common/scanner.c"
