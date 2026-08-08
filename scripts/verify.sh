#!/usr/bin/env bash
# Grammar verification (the CI stand-in — also run by verify-grammars.yml).
#
#   1. Materialize: upstream submodule (must sit exactly at ledger.json's
#      pinned sha, pristine) + patches/ + npm ci where declared +
#      tree-sitter generate with the pinned CLI -> build/. Every failure
#      mode of the old byte-for-byte reconstruction check is still fatal
#      here: a moved submodule pointer, a patch that no longer applies, a
#      generate error.
#   2. Grammar corpus tests (tree-sitter test, in build/).
#   3. Negative corpus: reference-rejected files must still be REJECTED.
#
# Everything grammar-specific comes from the grammar's ledger.json:
#   generate_cli (pinned CLI), generate_dirs (default ["."]),
#   generate_deps (non-null -> npm ci before generating).
#
# Usage: scripts/verify.sh <grammar-dir>     e.g. scripts/verify.sh crates/treebank-rust
set -euo pipefail
shopt -s nullglob
GRAMMAR_DIR="${1:?usage: verify.sh <grammar-dir>}"
SCRIPT_DIR=$(cd "$(dirname "$0")" && pwd)
cd "$GRAMMAR_DIR"
ROOT="$PWD"
TREEBANK_BIN="${TREEBANK_BIN:-$ROOT/../../target/release/treebank}"

CLI_WANT=$(jq -r .generate_cli ledger.json)
GEN_DIRS=()
while IFS= read -r d; do GEN_DIRS+=("$d"); done < <(jq -r '(.generate_dirs // ["."])[]' ledger.json)
TS="npx -y tree-sitter-cli@$CLI_WANT"

echo "== 1/3 materialize (submodule @ pinned sha + patches + generate, CLI $CLI_WANT)"
"$SCRIPT_DIR/materialize.sh" "$ROOT"

echo "== 2/3 grammar corpus tests"
(cd build && $TS test 2>&1 | grep -E "^Total parses")

echo "== 3/3 negative corpus"
negatives=("$ROOT"/test/negative/*)
if [ "${#negatives[@]}" -gt 0 ]; then
  "$TREEBANK_BIN" negative --grammar "$ROOT/build/${GEN_DIRS[0]}" --dir "$ROOT/test/negative"
else
  echo "   (no negative corpus files yet)"
fi

echo "verify: all checks passed"
