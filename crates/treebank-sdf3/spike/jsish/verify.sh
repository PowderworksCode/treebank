#!/usr/bin/env bash
# Regenerate the jsish parser, hold it to the corpus, and hold its
# bindings.json to node: every program under programs/ must print, under
# resolution from the data alone, what node prints.
set -euo pipefail

HERE=$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)
ROOT=$(cd "$HERE/../../../.." && pwd)

cd "$ROOT"
cargo run -q -p treebank-sdf3 --example lower -- "$HERE/jsish.sdf3"

cd "$HERE"
tree-sitter generate grammar.json
cp grammar.json src/grammar.json
tree-sitter test

# The vocabulary: the lowered roles.json and the generated node-types.json
# under the same checker `treebank roles` runs over every shipped grammar.
(cd "$ROOT" && cargo run -q -p treebank-sdf3 --example roles -- "$HERE")

tmp=$(mktemp -d)
trap 'rm -rf "$tmp"' EXIT
cp grammar.js tree-sitter.json "$tmp/"
(cd "$tmp" && tree-sitter generate >/dev/null)
cmp "$tmp/src/parser.c" src/parser.c
echo "grammar.js and grammar.json generate an identical parser"

python3 "$HERE/../../tools/resolve_check.py" "$HERE"

# The printer derived from the templates, held to the language's own formatter.
python3 "$HERE/../../tools/format_check.py" "$HERE"

# The third backend, held to the same corpus, and the three compared.
python3 "$HERE/../../tools/winnow_check.py" "$HERE"
python3 "$HERE/../../tools/confer.py" "$HERE"
