#!/usr/bin/env bash
# Smoke test for the ONE oracle property CI cannot otherwise see.
#
# verify.sh runs the ledger, materialize, `tree-sitter test` and the negative
# corpus. It never invokes an oracle. So the whole class of bug fixed in
# "oracles: an unreadable file is not an invalid file" — where an oracle
# answers `invalid` for a file it could not read, the sweep records every
# grammar failure as corpus noise, gap_files goes to zero and the run reports
# a flawless grammar — is invisible to every other check here.
#
# Two assertions per language, and the second matters as much as the first:
#
#   1. UNREADABLE IS FATAL. A path that does not exist must produce a
#      non-zero exit and NO verdict on stdout.
#   2. THE ORACLE STILL WORKS. A real valid file must come back `valid` and
#      a real invalid one `invalid`, both at exit 0.
#
# Without (2), an oracle broken into always failing would pass, which would
# make the guard worse than none.
#
# WHY THIS FILE HAS NO LIST IN IT
#
# The first version had one hand-written block per oracle and covered seven
# of the twelve languages: zig, bash, lua and rust were all added after it
# was written, and nothing noticed. A list somebody has to remember to edit
# is a list that is wrong — the same lesson verify-grammars.yml learned when
# it stopped hard-coding its matrix, and the docs learned when they started
# deriving theirs.
#
# So the languages come from `crates/*/ledger.json`, which is where a new
# language must land anyway, and each ledger's `oracle.smoke` says what its
# oracle needs. Adding language thirteen requires no edit here. If its ledger
# omits `oracle.smoke`, `treebank ledger` refuses the ledger outright, so it
# cannot be forgotten either — the coverage gap that produced this rewrite
# is now a hard error rather than a silent skip.
#
# The single entry point is `treebank oracle --lang <x>`, which is
# `Lang::validate` and nothing else. That is deliberately the call `sweep`
# adjudicates with, rather than each tool invoked directly: the oracles are
# four different shapes (batch stdin, c's three-valued JSON, fork-per-file
# exec oracles, and rust's `syn` in-process with no subprocess at all), and
# testing them through the real path covers the drivers and the wiring too.
# It is also the only way rust can be covered at all — it has no `tools/`
# directory to enumerate, which is exactly why it kept the bug the other
# eleven had fixed.
#
# Usage:
#   scripts/oracle-smoke.sh                 # skip oracles whose runtime is absent
#   scripts/oracle-smoke.sh --require-all   # a skip is a failure (this is CI)
#   scripts/oracle-smoke.sh --lang zig      # one language
set -uo pipefail
cd "$(dirname "$0")/.." || exit 1

REQUIRE_ALL=0
ONLY=""
while [ $# -gt 0 ]; do
  case "$1" in
    --require-all) REQUIRE_ALL=1 ;;
    --lang) ONLY=$2; shift ;;
    *) echo "usage: $0 [--require-all] [--lang <name>]" >&2; exit 2 ;;
  esac
  shift
done

command -v jq >/dev/null || { echo "oracle-smoke: jq is required" >&2; exit 2; }
TREEBANK=${TREEBANK_BIN:-./target/release/treebank}
[ -x "$TREEBANK" ] || { echo "oracle-smoke: no $TREEBANK — cargo build --release" >&2; exit 2; }

TMP=$(mktemp -d); trap 'rm -rf "$TMP"' EXIT
MISSING="$TMP/no/such/file"          # never created, on purpose
pass=0; fail=0; skip=0

ok()   { printf '  \033[32mok\033[0m    %s\n' "$1"; pass=$((pass+1)); }
bad()  { printf '  \033[31mFAIL\033[0m  %s\n' "$1"; fail=$((fail+1)); }
note() { printf '        %s\n' "$1"; }
skipped() {
  if [ "$REQUIRE_ALL" = 1 ]; then
    printf '  \033[31mFAIL\033[0m  %s (skipped, but --require-all)\n' "$1"; fail=$((fail+1))
  else
    printf '  \033[33mskip\033[0m  %s — %s\n' "$1" "$2"; skip=$((skip+1))
  fi
}

for ledger in crates/*/ledger.json; do
  lang=$(jq -r '.grammar' "$ledger")
  [ -n "$ONLY" ] && [ "$ONLY" != "$lang" ] && continue
  jq -e '.oracle.smoke' "$ledger" >/dev/null 2>&1 || continue

  # What must be present first. A bare name is a command on PATH; a
  # $-prefixed one is an environment variable that must be set.
  unmet=""
  while IFS= read -r req; do
    [ -z "$req" ] && continue
    case "$req" in
      \$*) name=${req#\$}; [ -n "${!name:-}" ] || unmet="$req is not set" ;;
      *)   command -v "$req" >/dev/null || unmet="no $req" ;;
    esac
  done < <(jq -r '.oracle.smoke.requires // [] | .[]' "$ledger")
  if [ -n "$unmet" ]; then skipped "$lang" "$unmet"; continue; fi

  build=$(jq -r '.oracle.smoke.build // ""' "$ledger")
  if [ -n "$build" ]; then
    if ! eval "$build" >/dev/null 2>&1; then skipped "$lang" "build failed: $build"; continue; fi
  fi

  good=$(jq -r '.oracle.smoke.valid'   "$ledger")
  evil=$(jq -r '.oracle.smoke.invalid' "$ledger")

  # stderr is kept, not discarded. An oracle that refuses to run usually says
  # exactly why — php's version floor names the package to install — and a
  # guard that swallows that and reports a bare exit status makes the reader
  # go read the source to find out what the tool already told them.
  err="$TMP/$lang.err"

  # 1. unreadable must be fatal, and must not answer
  out=$(echo "$MISSING" | "$TREEBANK" oracle --lang "$lang" 2>"$err"); status=$?
  if [ "$status" -eq 0 ]; then
    bad "$lang: an unreadable file exited 0"
    note "verdict was: ${out:-<none>} — validate() is only called on files the"
    note "grammar already failed, so 'invalid' here records them as noise"
    continue
  fi
  if [ -n "$out" ]; then
    bad "$lang: an unreadable file produced a verdict: $out"
    continue
  fi

  # 2. the oracle still works
  out=$(printf '%s\n%s\n' "$good" "$evil" | "$TREEBANK" oracle --lang "$lang" 2>"$err"); status=$?
  gv=$(grep -F "$good" <<<"$out" | cut -f2)
  ev=$(grep -F "$evil" <<<"$out" | cut -f2)
  if [ "$status" -ne 0 ]; then
    bad "$lang: exited $status on readable files"
    while IFS= read -r line; do note "$line"; done < <(tail -6 "$err")
  elif [ "$gv" != valid ] || [ "$ev" != invalid ]; then
    bad "$lang: expected valid/invalid, got '${gv:-<none>}'/'${ev:-<none>}'"
    note "valid fixture:   $good"
    note "invalid fixture: $evil"
  else
    ok "$lang"
  fi
done

echo
if [ "$fail" -gt 0 ]; then
  echo "oracle smoke: $fail failed, $pass passed, $skip skipped"
  exit 1
fi
echo "oracle smoke: $pass passed, $skip skipped"
