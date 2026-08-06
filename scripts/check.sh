#!/usr/bin/env bash
# Full verification of a grammar change, one command:
#
#   1. tree-sitter generate (pinned CLI) in each of the ledger's
#      generate_dirs, after npm ci if the ledger declares generate_deps
#   2. grammar corpus tests
#   3. corpus sweep — pass count must beat $PASS_BEFORE, and if $JOB_FILE is
#      set, every file in the job's cluster must now pass
#   4. negative corpus — reference-rejected files must still be rejected
#
# Prints one line per check and "CHECK OK" / "CHECK FAILED" at the end
# (nonzero exit on failure). Results saved to /tmp/agent-check-result.json.
#
# Usage: scripts/check.sh [grammar-dir]   (default: cwd)
# Env overrides (defaults work in the repo checkout):
#   TREEBANK_BIN, MANIFEST, PASS_BEFORE, JOB_FILE (optional cluster file)
set -uo pipefail
cd "${1:-.}"
[ -f ledger.json ] || { echo "agent-check: no ledger.json in $PWD"; exit 2; }
ROOT="$PWD"
LANG_NAME=$(jq -r .grammar ledger.json)
CLI=$(jq -r .generate_cli ledger.json)
NEED_DEPS=$(jq -r '.generate_deps // empty' ledger.json)
GEN_DIRS=()
while IFS= read -r d; do GEN_DIRS+=("$d"); done < <(jq -r '(.generate_dirs // ["."])[]' ledger.json)
TREEBANK_BIN="${TREEBANK_BIN:-$ROOT/../../target/release/treebank}"
MANIFEST="${MANIFEST:-$ROOT/../../corpus/$LANG_NAME/manifest.json}"
PASS_BEFORE="${PASS_BEFORE:-$(jq -r '.corpus.sweep_patched.passed // 0' ledger.json)}"
JOB_FILE="${JOB_FILE:-}"
SWEEP_OUT=/tmp/agent-check-sweep.json
fail=0

if [ -n "$NEED_DEPS" ] && [ ! -d node_modules ]; then
  npm ci --no-audit --no-fund >/dev/null 2>&1
fi

gen_ok=1
for d in "${GEN_DIRS[@]}"; do
  (cd "$d" && npx -y "tree-sitter-cli@$CLI" generate) >/tmp/agent-check-generate.log 2>&1 || gen_ok=0
done
if [ "$gen_ok" = 1 ]; then
  echo "generate ($CLI, ${GEN_DIRS[*]}): ok"
else
  echo "generate ($CLI): FAILED — see /tmp/agent-check-generate.log"
  fail=1
fi

tests_line=$(npx -y "tree-sitter-cli@$CLI" test 2>&1 | grep -E '^Total parses' || true)
tests_failed=$(sed -E 's/.*failed parses: ([0-9]+);.*/\1/' <<<"$tests_line")
if [ -n "$tests_line" ] && [ "$tests_failed" = "0" ]; then
  echo "corpus tests: ok ($tests_line)"
  corpus_tests=pass
else
  echo "corpus tests: FAILED ($tests_line)"
  corpus_tests=fail
  fail=1
fi

# Run the sweep from the repo root: language oracles (e.g. tools/ts-oracle)
# are resolved relative to the cwd.
if (cd "$ROOT/../.." && "$TREEBANK_BIN" sweep --lang "$LANG_NAME" --grammar "$ROOT" --manifest "$MANIFEST" --out "$SWEEP_OUT") >/dev/null 2>&1; then
  passed=$(jq .passed "$SWEEP_OUT")
  failed=$(jq .failed "$SWEEP_OUT")
  if [ "$passed" -gt "$PASS_BEFORE" ]; then
    echo "sweep: ok — $passed passed / $failed failed (baseline $PASS_BEFORE, +$((passed - PASS_BEFORE)))"
  else
    echo "sweep: FAILED — $passed passed does not beat baseline $PASS_BEFORE"
    fail=1
  fi
  cluster_fixed=true
  if [ -n "$JOB_FILE" ]; then
    still_failing=$(jq -r --slurpfile job "$JOB_FILE" \
      '[.clusters[].paths[]] as $f | $job[0].valid_files[] | select(. as $x | $f | index($x))' \
      "$SWEEP_OUT")
    if [ -n "$still_failing" ]; then
      echo "cluster: FAILED — these job files still fail:"
      sed 's/^/  /' <<<"$still_failing"
      cluster_fixed=false
      fail=1
    else
      echo "cluster: ok — all job files now pass"
    fi
  fi
else
  echo "sweep: FAILED to run"
  passed=0; failed=0; cluster_fixed=false
  fail=1
fi

if "$TREEBANK_BIN" negative --grammar "$ROOT/${GEN_DIRS[0]}" --dir "$ROOT/test/negative" >/dev/null 2>&1; then
  echo "negative corpus: ok"
  negative=pass
else
  echo "negative corpus: FAILED — the grammar now ACCEPTS invalid code:"
  "$TREEBANK_BIN" negative --grammar "$ROOT/${GEN_DIRS[0]}" --dir "$ROOT/test/negative" 2>&1 | sed 's/^/  /' || true
  negative=fail
  fail=1
fi

jq -n --argjson pb "$PASS_BEFORE" --argjson pa "${passed:-0}" --argjson fa "${failed:-0}" \
  --arg ct "$corpus_tests" --arg neg "$negative" --argjson cf "$cluster_fixed" '{
    sweep_before: {passed: $pb},
    sweep_after: {passed: $pa, failed: $fa},
    corpus_tests: $ct, negative_tests: $neg, cluster_files_fixed: $cf
  }' > /tmp/agent-check-result.json

if [ "$fail" = 0 ]; then echo "CHECK OK"; else echo "CHECK FAILED"; fi
exit "$fail"
