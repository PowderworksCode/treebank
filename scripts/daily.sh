#!/usr/bin/env bash
# The daily checker. Cron runs this once a day:
#
#   0 6 * * * cd /Users/zackmaril/powderworks/treebank && scripts/daily.sh >> daily.log 2>&1
#
# For every vendored grammar (crates/treebank-*/ledger.json):
#
#   1. fetch  — re-resolve + download the corpus. npm resolves each package's
#      latest version at fetch time, so new releases arrive daily; crates.io
#      versions come from the rank list (refresh that by re-running
#      `treebank rank --lang rust` against a fresh db dump when you care).
#   2. sweep  — parse everything, adjudicate failures with the reference
#      parser, write corpus/<lang>/reports/sweep.json + REPORT.md.
#   3. agent  — only if the report shows grammar gaps: one claude session per
#      language per day (bounded spend), pointed at REPORT.md. It fixes
#      clusters, loops on scripts/check.sh, and captures patches + ledger
#      entries per GRAMMARS.md. It does NOT commit; changes wait in the
#      working tree for human review.
#   4. re-sweep + verify — record what the agent actually achieved.
#
# Env: CLAUDE_BIN (default claude), CLAUDE_MODEL (default sonnet),
#      TREEBANK_LIMIT (packages per ecosystem, default 100).
set -uo pipefail
cd "$(dirname "$0")/.."
ROOT="$PWD"
CLAUDE_BIN="${CLAUDE_BIN:-claude}"
CLAUDE_MODEL="${CLAUDE_MODEL:-sonnet}"
LIMIT="${TREEBANK_LIMIT:-100}"
TB="$ROOT/target/release/treebank"

echo "=== treebank daily: $(date '+%Y-%m-%d %H:%M') ==="
cargo build --release --quiet || { echo "daily: cargo build failed"; exit 1; }

overall=0
for ledger in "$ROOT"/crates/treebank-*/ledger.json; do
  lang=$(jq -r .grammar "$ledger")
  grammar="$ROOT/crates/treebank-$lang"
  report="$ROOT/corpus/$lang/reports/sweep.json"
  echo "--- $lang"

  if ! "$TB" fetch --lang "$lang" --limit "$LIMIT" 2>&1 | tail -1; then
    echo "daily: $lang fetch failed, sweeping the existing corpus anyway"
  fi
  if ! "$TB" sweep --lang "$lang" --grammar "$grammar" 2>&1 | tail -2; then
    echo "daily: $lang sweep FAILED"
    overall=1
    continue
  fi

  gaps=$(jq .gap_files "$report")
  if [ "$gaps" -eq 0 ]; then
    echo "daily: $lang clean — no grammar gaps"
    continue
  fi

  echo "daily: $lang has $gaps gap file(s) — launching fix agent"
  "$CLAUDE_BIN" -p "Read corpus/$lang/reports/REPORT.md and fix ALL of its gap
clusters, one at a time, exactly per the report's instructions. Work in
crates/treebank-$lang. After each fix, run ../../scripts/check.sh from that
dir until it prints CHECK OK. Capture each fix as patches/NNNN-*.patch with a
ledger.json entry and a LOCAL-PATCHES.md note, per GRAMMARS.md. Update the
ledger's corpus.sweep_patched numbers when done, and finish by running
scripts/verify.sh crates/treebank-$lang from the repo root — it must pass.
Do NOT git commit anything. If a cluster is genuinely beyond a minimal
grammar change, skip it and say so in your final message." \
    --model "$CLAUDE_MODEL" --max-turns 200 \
    --permission-mode bypassPermissions \
    > "$ROOT/corpus/$lang/reports/agent.log" 2>&1
  echo "daily: agent finished (log: corpus/$lang/reports/agent.log)"

  # Trust nothing: re-sweep and verify to see what actually happened.
  "$TB" sweep --lang "$lang" --grammar "$grammar" 2>&1 | grep '^sweep:' | head -1
  if scripts/verify.sh "$grammar" >/dev/null 2>&1; then
    echo "daily: $lang verify ok"
  else
    echo "daily: $lang verify FAILED after agent — needs human review"
    overall=1
  fi
  left=$(jq .gap_files "$report")
  if [ "$left" -gt 0 ]; then
    echo "daily: $lang still has $left gap file(s) — parked for human (see REPORT.md)"
  fi
done

echo "=== treebank daily done ==="
exit "$overall"
