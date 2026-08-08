#!/usr/bin/env bash
# Materialize a grammar's working tree from its source of truth:
#
#   upstream/  (git submodule, pinned by ledger.json's upstream.sha)
#   + patches/ applied in order
#   (+ npm ci when the ledger declares generate_deps)
#   + tree-sitter generate with the pinned CLI in each generate_dir
#   -> build/  (gitignored)
#
# build/ is the only tree that sweeps, corpus tests, the negative corpus
# and publishing ever see. Nothing generated is committed; the committed
# artifacts are the submodule pointer and patches/, which this script
# asserts agree with ledger.json.
#
# build/ is made a throwaway git repo with a single commit, so a grammar
# agent's edits are always visible as `git -C build diff` and can be
# captured directly as the next patches/NNNN-*.patch.
#
# Usage: scripts/materialize.sh <grammar-dir>
set -euo pipefail
shopt -s nullglob
GRAMMAR_DIR="${1:?usage: materialize.sh <grammar-dir>}"
cd "$GRAMMAR_DIR"
ROOT="$PWD"

SHA=$(jq -r .upstream.sha ledger.json)
CLI_WANT=$(jq -r .generate_cli ledger.json)
NEED_DEPS=$(jq -r '.generate_deps // empty' ledger.json)
GEN_DIRS=()
while IFS= read -r d; do GEN_DIRS+=("$d"); done < <(jq -r '(.generate_dirs // ["."])[]' ledger.json)
TS="npx -y tree-sitter-cli@$CLI_WANT"

if [ ! -e upstream/.git ]; then
  echo "materialize: initializing upstream submodule"
  git submodule update --init -- "$ROOT/upstream"
fi
HEAD=$(git -C upstream rev-parse HEAD)
if [ "$HEAD" != "$SHA" ]; then
  echo "materialize: FAIL — upstream submodule is at $HEAD but ledger.json pins $SHA" >&2
  echo "  fix one side: git -C $ROOT/upstream checkout $SHA, or update ledger.json" >&2
  exit 1
fi
if [ -n "$(git -C upstream status --porcelain)" ]; then
  echo "materialize: FAIL — upstream submodule working tree is dirty; it must stay pristine" >&2
  git -C upstream status --porcelain >&2
  exit 1
fi

rm -rf build
mkdir build
git -C upstream archive HEAD | tar -x -C build

# build/ must be its own git repo BEFORE patches are applied: `git apply`
# run from a subdirectory of a repo resolves patch paths against that repo's
# root and silently IGNORES paths outside the subdirectory — every patch
# would "apply" as a no-op. With build/ as its own repo, paths resolve
# against build/.
git init -q build
git -C build add -A
git -C build -c user.name=treebank -c user.email=treebank@localhost \
  commit -qm "upstream @ $SHA"

for p in "$ROOT"/patches/*.patch; do
  echo "   applying $(basename "$p")"
  git -C build apply --whitespace=nowarn "$p"
done

if [ -n "$NEED_DEPS" ]; then
  (cd build && npm ci --no-audit --no-fund >/dev/null 2>&1)
fi
for d in "${GEN_DIRS[@]}"; do
  (cd "build/$d" && $TS generate)
done

git -C build add -A
git -C build -c user.name=treebank -c user.email=treebank@localhost \
  commit -qm "materialized: upstream $SHA + $(ls "$ROOT"/patches/*.patch 2>/dev/null | wc -l) patches + generate (CLI $CLI_WANT)"

echo "materialize: ok — $GRAMMAR_DIR/build (upstream $SHA, CLI $CLI_WANT)"
