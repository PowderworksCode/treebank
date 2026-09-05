#!/usr/bin/env bash
# Regenerate the rustish parser, hold it to the corpus, and hold its
# bindings.json to rustc: every program under programs/ must print, under
# resolution from the data alone, what the compiled program prints.
set -euo pipefail

HERE=$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)
ROOT=$(cd "$HERE/../../../.." && pwd)

cd "$ROOT"
cargo run -q -p treebank-sdf3 --example lower -- "$HERE/rustish.sdf3" --generate

cd "$HERE"
cp grammar.json src/grammar.json
tree-sitter test

tmp=$(mktemp -d)
trap 'rm -rf "$tmp"' EXIT
cp grammar.js tree-sitter.json "$tmp/"
(cd "$tmp" && tree-sitter generate >/dev/null)
cmp "$tmp/src/parser.c" src/parser.c
echo "grammar.js and grammar.json generate an identical parser"

python3 "$HERE/../../tools/resolve_check.py" "$HERE" --entry main
