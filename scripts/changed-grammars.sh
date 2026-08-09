#!/usr/bin/env bash
# Which grammars does a change need CI to look at?
#
# Answering this in a script rather than inline in a workflow keeps the rule in
# one place, testable from a shell — `scripts/changed-grammars.sh --self-test`
# exercises it — instead of spread across two YAML files where it can only be
# debugged by pushing.
#
# The rule:
#
#   - a change under crates/treebank-<lang>/ concerns that grammar;
#   - a change to anything in CORE concerns all of them, because CORE is what
#     builds, verifies and packages every grammar;
#   - anything else (docs, corpus reports, the daily job) concerns none.
#
# Usage: scripts/changed-grammars.sh <base-ref> [head-ref]
# Prints one JSON object:
#   {"grammars":["treebank-rust"],"core":false,"reason":"..."}
set -euo pipefail

# Changes here can alter what every grammar builds into or how it is checked,
# so they put all grammars back in scope. Keep it tight: this is the difference
# between one CI job and five.
CORE=(
  'scripts/'
  'crates/treebank-cli/'
  'tools/'
  '.github/workflows/'
  '.gitmodules'
  'Cargo.toml'
  'Cargo.lock'
)

all_grammars() {
  local d
  for d in crates/treebank-*/; do
    [ -f "$d/ledger.json" ] || continue    # excludes treebank-cli
    basename "$d"
  done | sort
}

classify() {
  # Reads changed paths on stdin, prints the JSON object.
  local paths core=false hit
  paths=$(cat)

  local p c
  while IFS= read -r p; do
    [ -n "$p" ] || continue
    for c in "${CORE[@]}"; do
      case "$c" in
        */) [ "${p#"$c"}" != "$p" ] && core=true ;;
        *)  [ "$p" = "$c" ] && core=true ;;
      esac
    done
  done <<< "$paths"

  local selected=()
  if [ "$core" = true ]; then
    while IFS= read -r hit; do selected+=("$hit"); done < <(all_grammars)
  else
    while IFS= read -r hit; do
      [ -n "$hit" ] && selected+=("$hit")
    done < <(printf '%s\n' "$paths" \
      | sed -n 's|^crates/\(treebank-[^/]*\)/.*|\1|p' \
      | sort -u \
      | while IFS= read -r g; do [ -f "crates/$g/ledger.json" ] && echo "$g"; done)
  fi

  local reason
  if [ "$core" = true ]; then
    reason="a core path changed — every grammar is in scope"
  elif [ "${#selected[@]}" -eq 0 ]; then
    reason="no grammar or core path changed"
  else
    reason="changed: ${selected[*]}"
  fi

  printf '{"grammars":%s,"core":%s,"reason":%s}\n' \
    "$(printf '%s\n' ${selected+"${selected[@]}"} | jq -R . | jq -sc 'map(select(length>0))')" \
    "$core" \
    "$(printf '%s' "$reason" | jq -Rs .)"
}

if [ "${1:-}" = "--self-test" ]; then
  fail=0
  t() { # t <expected-grammars-json> <expected-core> <paths...>
    local want=$1 wantcore=$2; shift 2
    local got
    got=$(printf '%s\n' "$@" | classify)
    local g c
    g=$(jq -c .grammars <<<"$got"); c=$(jq -r .core <<<"$got")
    if [ "$g" != "$want" ] || [ "$c" != "$wantcore" ]; then
      echo "  FAIL paths=[$*]" >&2
      echo "       want grammars=$want core=$wantcore" >&2
      echo "       got  grammars=$g core=$c" >&2
      fail=1
    else
      echo "  ok   [$*] -> $g core=$c"
    fi
  }
  echo "changed-grammars self-test"
  t '["treebank-rust"]' false 'crates/treebank-rust/patches/0002-x.patch'
  t '["treebank-rust"]' false 'crates/treebank-rust/upstream'
  t '["treebank-java","treebank-rust"]' false 'crates/treebank-rust/ledger.json' 'crates/treebank-java/ledger.json'
  t '[]' false 'README.md' 'PUBLISHING.md'
  t '[]' false 'corpus/rust/reports/REPORT.md'
  # treebank-cli has no ledger.json, so it is core rather than a grammar
  t "$(all_grammars | jq -R . | jq -sc .)" true 'crates/treebank-cli/src/main.rs'
  t "$(all_grammars | jq -R . | jq -sc .)" true 'scripts/materialize.sh'
  t "$(all_grammars | jq -R . | jq -sc .)" true '.github/workflows/publish-grammars.yml'
  t "$(all_grammars | jq -R . | jq -sc .)" true 'tools/consumer-test/src/main.rs'
  t "$(all_grammars | jq -R . | jq -sc .)" true '.gitmodules'
  # core wins even when a single grammar also changed
  t "$(all_grammars | jq -R . | jq -sc .)" true 'crates/treebank-rust/ledger.json' 'scripts/verify.sh'
  [ "$fail" = 0 ] && echo "changed-grammars: self-test ok" || { echo "changed-grammars: self-test FAILED" >&2; exit 1; }
  exit 0
fi

# No diff to work from — a branch's first push, or a manual dispatch. Callers
# use this to fall back to everything, which is the safe direction.
if [ "${1:-}" = "--all" ]; then
  printf '{"grammars":%s,"core":true,"reason":%s}\n' \
    "$(all_grammars | jq -R . | jq -sc .)" \
    "$(printf '%s' "no base to diff against — every grammar is in scope" | jq -Rs .)"
  exit 0
fi

BASE=${1:?usage: changed-grammars.sh <base-ref> [head-ref] | --all | --self-test}
HEAD=${2:-HEAD}
git diff --name-only "$BASE" "$HEAD" | classify
