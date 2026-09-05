#!/usr/bin/env bash
# Regenerate the cppish parser and hold it to the corpus expectations, then
# check the `carry` post-condition from notes/metagrammar.md §3: the
# ambiguous statement must actually fork. Needs the pinned tree-sitter CLI.
set -euo pipefail

HERE=$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)
ROOT=$(cd "$HERE/../../../.." && pwd)

cd "$ROOT"
# `--generate` lets the lowering ask tree-sitter which conflicts the carry
# needs, and pins them in tree-sitter.conflicts.json beside the module.
cargo run -q -p treebank-sdf3 --example lower -- "$HERE/cppish.sdf3" --generate

cd "$HERE"
cp grammar.json src/grammar.json
tree-sitter test

tmp=$(mktemp -d)
trap 'rm -rf "$tmp"' EXIT
cp grammar.js tree-sitter.json "$tmp/"
(cd "$tmp" && tree-sitter generate >/dev/null)
cmp "$tmp/src/parser.c" src/parser.c
echo "grammar.js and grammar.json generate an identical parser"

# The post-condition: a declared carry that never forks is dead text.
printf 'a < b > c;\n' > "$tmp/carry.cppish"
peak=$(tree-sitter parse --debug=normal "$tmp/carry.cppish" 2>&1 | grep -o 'version_count:[0-9]*' | sort -t: -k2 -n | tail -1)
echo "a < b > c; peak ${peak:-version_count:?}"
case "$peak" in
  version_count:1|"") echo "the carry never forked: the declared conflict is dead text" >&2; exit 1 ;;
esac
printf 'x = a < b > c;\n' > "$tmp/nofork.cppish"
peak=$(tree-sitter parse --debug=normal "$tmp/nofork.cppish" 2>&1 | grep -o 'version_count:[0-9]*' | sort -t: -k2 -n | tail -1)
echo "x = a < b > c; peak ${peak:-version_count:?}"
case "$peak" in
  version_count:1) ;;
  *) echo "the assignment forked, but no declaration is possible there" >&2; exit 1 ;;
esac
