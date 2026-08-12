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

# Three things in this script reach the network — the submodule clone, `npm
# ci`, and npx, which resolves and downloads the pinned CLI because each CI
# job starts with an empty npx cache. They are the only steps here that can
# fail for a reason having nothing to do with the grammar, and they do fail.
# Measured 2026-08-12 (run 31642866147 and the push three minutes before it):
# `npx -y tree-sitter-cli@0.25.10` exited 1 in about a second having printed
# NOTHING AT ALL — no npm line, no npx line, no error — on go, then on
# javascript and typescript, while every other grammar passed. Re-running the
# identical commits passed all twelve.
#
# It is an outage, not a coin flip, and it is worth writing down how narrow
# one is, because that is what sets the retry schedule below. Timestamped
# across one run's twelve jobs: go's npx succeeded at 22:02:02-04, then every
# fresh npx failed from 22:02:04 to 22:02:39 — java at :04/:11/:21, python at
# :07/:14/:24, rust at :20/:28/:38, all three exhausting their retries inside
# the same window — and php and typescript succeeded again from 22:02:48. A
# ~35 second hole. So retries must SPAN more than they COUNT: 5 + 15 + 45
# covers 65 seconds, where the first cut's 5 + 10 covered 16 and rescued
# nobody.
#
# The other half of the bug is the silence, which is worse: a transient
# registry failure was indistinguishable from a real generate error, so the
# log told a grammar agent only that materialize died somewhere after the last
# patch. fetch() fixes both halves — it retries, and it prints what the
# command actually said.
#
# What fetch() must NOT wrap is `tree-sitter generate`, because a generate
# failure is usually the grammar being wrong, and retrying that is 15 seconds
# and three copies of the same error for every typo in grammar.js. So the
# retry goes on a separate one-line probe of the CLI instead, and the probe
# leaves the npx cache warm: measured, `npx -y tree-sitter-cli@0.25.10` with
# an exact version spec is then satisfied entirely from cache and succeeds
# under npm_config_offline. Every later npx in the job — generate here, and
# `tree-sitter test` over in verify.sh — is therefore past the registry
# already, and generate keeps failing on the first try, immediately, with its
# own message intact.
#
# And when a failing npm or npx says nothing at all, fetch() goes and gets the
# debug log npm always writes anyway. That is not a hypothetical politeness:
# the first version of this change made the probe retry in CI and the three
# attempts printed three empty lines, so the retry alone would have left the
# next person exactly as blind as run 31642866147 did.
NPM_LOGS="${npm_config_logs_dir:-${npm_config_cache:-$HOME/.npm}/_logs}"
npm_log_tail() {  # npm_log_tail <marker-file>: what npm wrote down but did not say
  local since=$1 newest
  newest=$(ls -t "$NPM_LOGS"/*-debug-*.log 2>/dev/null | head -1) || return 0
  # Only a log this attempt actually wrote. Any older one belongs to some
  # earlier command and would be a confidently misleading answer.
  if [ -n "$newest" ] && [ "$newest" -nt "$since" ]; then
    echo "   --- $newest (last 40 lines) ---" >&2
    tail -40 "$newest" >&2
  fi
}

fetch() {  # fetch <what> <cmd...>
  local what=$1; shift
  local out rc attempt marker
  # 5 + 15 + 45: four attempts spanning 65 seconds, chosen against a measured
  # outage rather than by taste. See the note above.
  local backoff=(5 15 45)
  marker=$(mktemp)
  trap 'rm -f "$marker"' RETURN
  for attempt in 1 2 3 4; do
    touch "$marker"
    # rc is read in the else branch, not after the `if`: an if compound resets
    # $? to 0 once it is done, so `rc=$?` on the far side reports every failure
    # as "exited 0".
    if out=$("$@" 2>&1); then
      if [ -n "$out" ]; then printf '%s\n' "$out"; fi
      return 0
    else
      rc=$?
    fi
    echo "materialize: $what exited $rc (attempt $attempt/4)" >&2
    if [ -n "$out" ]; then printf '%s\n' "$out" >&2; else echo "   (no output at all)" >&2; fi
    npm_log_tail "$marker"
    if [ "$attempt" != 4 ]; then sleep "${backoff[attempt-1]}"; fi
  done
  echo "materialize: FAIL — $what failed 4 times over 65 s; see its output above" >&2
  rm -f "$marker"
  exit 1
}

if [ ! -e upstream/.git ]; then
  echo "materialize: initializing upstream submodule"
  fetch "submodule clone" git submodule update --init -- "$ROOT/upstream"
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
  # This one used to be `>/dev/null 2>&1`, which threw away the diagnosis of
  # every dependency failure to keep the log tidy. fetch() keeps it tidy the
  # honest way: npm's one-line summary on success, all of it on failure.
  (cd build && fetch "npm ci" npm ci --no-audit --no-fund)
fi
# $TS is deliberately unquoted in both places — it is a command line
# ("npx -y pkg@ver"), not a path.
fetch "fetching tree-sitter-cli@$CLI_WANT" $TS --version
for d in "${GEN_DIRS[@]}"; do
  (cd "build/$d" && $TS generate)
done

git -C build add -A
git -C build -c user.name=treebank -c user.email=treebank@localhost \
  commit -qm "materialized: upstream $SHA + $(ls "$ROOT"/patches/*.patch 2>/dev/null | wc -l) patches + generate (CLI $CLI_WANT)"

echo "materialize: ok — $GRAMMAR_DIR/build (upstream $SHA, CLI $CLI_WANT)"
