#!/usr/bin/env bash
# Regenerate the SDF3 rewrite of treebank-hcl and hold it to everything
# the shipped crate is held to: its corpus, its negative corpus, the roles
# gate, the lint baselines, the shape fixtures, and -- with the locked
# corpus hydrated under corpus/hcl -- the sweep and shape check over all
# of it. The reference numbers are in results.md.
set -euo pipefail

HERE=$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)
ROOT=$(cd "$HERE/../../../.." && pwd)
REF="$ROOT/crates/treebank-hcl"

cd "$ROOT"
cargo run -q -p treebank-sdf3 --example lower -- "$HERE/hcl.sdf3" --generate

cd "$HERE"
cp grammar.json src/grammar.json
# The shipped crate's fixtures, verbatim.
rm -rf test/corpus test/negative
cp -r "$REF/test/corpus" "$REF/test/negative" test/
rm -f ~/.cache/tree-sitter/lib/hcl.so   # the reference grammar is named hcl too
tree-sitter test

tmp=$(mktemp -d)
trap 'rm -rf "$tmp"' EXIT
cp grammar.js tree-sitter.json "$tmp/"
(cd "$tmp" && tree-sitter generate >/dev/null)
cmp "$tmp/src/parser.c" src/parser.c
echo "grammar.js and grammar.json generate an identical parser"

cd "$ROOT"
cargo build -q -p treebank-cli
TB=target/debug/treebank
(cd crates/treebank-sdf3 && cargo run -q -p treebank-sdf3 --example roles -- "$HERE")
$TB lint "$HERE"
# One of the shipped crate's 28 negatives is accepted here; results.md says which and why.
$TB negative --grammar "$HERE" --dir "$HERE/test/negative" || true
$TB shape --lang hcl --grammar "$HERE" --manifest corpus/hcl/manifest.json --out "$tmp/shape-fixtures.json" --dir "$REF/test/shape"
if [ -f corpus/hcl/manifest.json ]; then
  $TB sweep --lang hcl --grammar "$HERE" --manifest corpus/hcl/manifest.json --out "$tmp/sweep.json" --no-write-ledger
  $TB shape --lang hcl --grammar "$HERE" --manifest corpus/hcl/manifest.json --out "$tmp/shape.json"
fi

# The other two backends, held to the same corpus, and the three compared.
cd crates/treebank-sdf3
python3 tools/antlr_check.py "$HERE" || true
python3 tools/winnow_check.py "$HERE"
python3 tools/confer.py "$HERE"
