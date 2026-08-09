#!/usr/bin/env bash
# Grammar verification (the CI stand-in — also run by verify-grammars.yml).
#
#   0. Ledger: `treebank ledger` — the ledger must describe the tree it
#      claims to (known language, patches on disk matching patches in the
#      file, full upstream sha). Cheap, and it runs first because every
#      later step reads the ledger and would otherwise fail obscurely.
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

echo "== 1/4 ledger"
"$TREEBANK_BIN" ledger "$ROOT"

echo "== 2/4 materialize (submodule @ pinned sha + patches + generate, CLI $CLI_WANT)"
"$SCRIPT_DIR/materialize.sh" "$ROOT"

echo "== 3/4 grammar corpus tests"
# `tree-sitter test` ends with a blank line, so piping to `tail -1` printed an
# empty line and showed nothing. Capture instead: on failure print enough of
# the output to diagnose it, on success print the summary line (a grammar with
# no test/corpus has none, which is not an error — `grep` finding nothing must
# not take the whole script down under `set -e`).
if ! TEST_OUT=$(cd build && $TS test 2>&1); then
  echo "verify: FAIL — grammar corpus tests failed:" >&2
  echo "$TEST_OUT" | tail -30 >&2
  exit 1
fi
echo "   $(grep -E '^Total parses' <<<"$TEST_OUT" || echo '(no corpus tests in test/corpus)')"

echo "== 4/4 negative corpus"
negatives=("$ROOT"/test/negative/*)
if [ "${#negatives[@]}" -gt 0 ]; then
  "$TREEBANK_BIN" negative --grammar "$ROOT/build/${GEN_DIRS[0]}" --dir "$ROOT/test/negative"
else
  echo "   (no negative corpus files yet)"
fi

echo "verify: all checks passed"
