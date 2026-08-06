#!/usr/bin/env bash
# Grammar verification (the CI stand-in — also run by verify-grammars.yml).
#
#   1. Reconstruction: upstream (git_url @ sha from ledger.json, cached under
#      ~/.cache/treebank) + patches/ (+ npm ci when the ledger declares
#      generate_deps) + tree-sitter generate with the pinned CLI in each
#      generate_dir must reproduce the vendored tree exactly.
#   2. Grammar corpus tests (tree-sitter test).
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
cd "$GRAMMAR_DIR"
ROOT="$PWD"
TREEBANK_BIN="${TREEBANK_BIN:-$ROOT/../../target/release/treebank}"
CACHE="${TREEBANK_CACHE:-$HOME/.cache/treebank}/upstream"

SHA=$(jq -r .upstream.sha ledger.json)
URL=$(jq -r .upstream.git_url ledger.json)
CLI_WANT=$(jq -r .generate_cli ledger.json)
NAME=$(jq -r .grammar ledger.json)
NEED_DEPS=$(jq -r '.generate_deps // empty' ledger.json)
GEN_DIRS=()
while IFS= read -r d; do GEN_DIRS+=("$d"); done < <(jq -r '(.generate_dirs // ["."])[]' ledger.json)
TS="npx -y tree-sitter-cli@$CLI_WANT"

UP="$CACHE/tree-sitter-$NAME-${SHA:0:12}"
if [ ! -d "$UP" ]; then
  echo "verify: fetching upstream $URL @ $SHA"
  TMPC=$(mktemp -d)
  git init -q "$TMPC"
  git -C "$TMPC" remote add origin "$URL"
  git -C "$TMPC" fetch -q --depth 1 origin "$SHA"
  git -C "$TMPC" checkout -q FETCH_HEAD
  rm -rf "$TMPC/.git"
  mkdir -p "$CACHE"
  mv "$TMPC" "$UP"
fi

echo "== 1/3 reconstruction (upstream $SHA + patches + generate, CLI $CLI_WANT)"
TMP=$(mktemp -d)
trap 'rm -rf "$TMP"' EXIT
cp -R "$UP" "$TMP/reconstructed"
for p in "$ROOT"/patches/*.patch; do
  echo "   applying $(basename "$p")"
  (cd "$TMP/reconstructed" && git apply "$p")
done
if [ -n "$NEED_DEPS" ]; then
  (cd "$TMP/reconstructed" && npm ci --no-audit --no-fund >/dev/null 2>&1)
fi
for d in "${GEN_DIRS[@]}"; do
  (cd "$TMP/reconstructed/$d" && $TS generate)
done
EXCLUDES=(--exclude=.git --exclude=patches --exclude=scripts --exclude=negative
  --exclude=ledger.json --exclude=LOCAL-PATCHES.md --exclude=node_modules
  --exclude=build --exclude=target)
if ! diff -r "${EXCLUDES[@]}" "$TMP/reconstructed" "$ROOT" >/dev/null; then
  echo "verify: FAIL — reconstructed tree differs from vendored tree:" >&2
  diff -rq "${EXCLUDES[@]}" "$TMP/reconstructed" "$ROOT" >&2 || true
  exit 1
fi
echo "   ok — tree is exactly upstream + patches + generate"

echo "== 2/3 grammar corpus tests"
$TS test 2>&1 | tail -1

echo "== 3/3 negative corpus"
negatives=("$ROOT"/test/negative/*)
if [ "${#negatives[@]}" -gt 0 ]; then
  "$TREEBANK_BIN" negative --grammar "$ROOT/${GEN_DIRS[0]}" --dir "$ROOT/test/negative"
else
  echo "   (no negative corpus files yet)"
fi

echo "verify: all checks passed"
