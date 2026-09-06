#!/usr/bin/env bash
# Regenerate the mini parser from the lowered grammar and hold it to the
# corpus expectations. Needs the pinned tree-sitter CLI (notes/field_guide.md §0).
set -euo pipefail

HERE=$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)
ROOT=$(cd "$HERE/../../../.." && pwd)

cd "$ROOT"
cargo run -q -p treebank-sdf3 --example lower -- "$HERE/mini.sdf3"

cd "$HERE"
tree-sitter generate grammar.json
# `tree-sitter test` reads the grammar from src/, which `generate` fills only
# when it starts from grammar.js. grammar.json is the artifact of record.
cp grammar.json src/grammar.json
tree-sitter test

# The readable grammar.js is a second source, and must generate the same parser.
tmp=$(mktemp -d)
trap 'rm -rf "$tmp"' EXIT
cp grammar.js tree-sitter.json "$tmp/"
(cd "$tmp" && tree-sitter generate >/dev/null)
cmp "$tmp/src/parser.c" src/parser.c
echo "grammar.js and grammar.json generate an identical parser"

# The third backend, held to the same corpus, and the three compared.
python3 "$HERE/../../tools/winnow_check.py" "$HERE"
python3 "$HERE/../../tools/confer.py" "$HERE"
