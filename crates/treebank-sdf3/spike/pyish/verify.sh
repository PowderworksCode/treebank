#!/usr/bin/env bash
# Regenerate the pyish parser -- grammar and generated scanner -- from the
# lowered SDF3 and hold it to the corpus expectations. Needs the pinned
# tree-sitter CLI (notes/field_guide.md §0).
set -euo pipefail

HERE=$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)
ROOT=$(cd "$HERE/../../../.." && pwd)

cd "$ROOT"
cargo run -q -p treebank-sdf3 --example lower -- "$HERE/pyish.sdf3"

cd "$HERE"
tree-sitter generate grammar.json
cp grammar.json src/grammar.json
tree-sitter test

tmp=$(mktemp -d)
trap 'rm -rf "$tmp"' EXIT
cp grammar.js tree-sitter.json "$tmp/"
(cd "$tmp" && tree-sitter generate >/dev/null)
cmp "$tmp/src/parser.c" src/parser.c
echo "grammar.js and grammar.json generate an identical parser"

# The bindings: the generated query must compile and capture, and the data
# must classify every name as CPython's symtable does.
captures=$(tree-sitter query queries/locals.scm bindings/nested.py 2>/dev/null | grep -c capture)
echo "queries/locals.scm: $captures captures on bindings/nested.py"
[ "$captures" -gt 0 ]
python3 "$HERE/../../tools/bindings_check.py" "$HERE"
python3 "$HERE/../../tools/resolve_check.py" "$HERE"
