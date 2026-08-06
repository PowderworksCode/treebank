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
#      entries per GRAMMARS.md. The agent never touches git.
#   4. re-sweep + verify — record what the agent actually achieved.
#   5. PR — if verify passes and crates/treebank-<lang> changed, the script
#      commits those changes on a fresh branch, pushes, and opens a PR
#      (authorized for the treebank repo only). Merging stays human; main is
#      never committed to directly. On verify failure the changes stay in
#      the working tree, unpushed, flagged for review.
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
  if ! scripts/verify.sh "$grammar" >/dev/null 2>&1; then
    echo "daily: $lang verify FAILED after agent — changes left in working tree, no PR"
    overall=1
    continue
  fi
  echo "daily: $lang verify ok"
  left=$(jq .gap_files "$report")
  [ "$left" -gt 0 ] && echo "daily: $lang still has $left gap file(s) — noted in PR / REPORT.md"

  # Open a PR for whatever the agent changed in this grammar (treebank only,
  # per standing authorization). Merging stays human.
  if [ -z "$(git status --porcelain "crates/treebank-$lang")" ]; then
    echo "daily: $lang agent made no grammar changes — no PR"
    continue
  fi
  passed_now=$(jq .passed "$report")
  failed_now=$(jq .failed "$report")
  branch="grammar-fixes/$lang-$(date +%Y%m%d-%H%M)"
  base=$(git branch --show-current)
  git checkout -qb "$branch"
  git add "crates/treebank-$lang"
  git commit -qm "treebank-$lang: fix grammar gaps from daily sweep

Daily sweep found $gaps gap file(s); after fixes: $passed_now passed / $failed_now failed.
Patches and evidence are in crates/treebank-$lang/patches/ and ledger.json.

Co-Authored-By: Claude Fable 5 <noreply@anthropic.com>"
  if git push -qu origin "$branch" && gh pr create --base main --head "$branch" \
      --title "treebank-$lang: grammar fixes from daily sweep ($(date +%Y-%m-%d))" \
      --body "Automated fix run from \`scripts/daily.sh\`.

- Gap files found by today's sweep: **$gaps**
- After fixes: **$passed_now passed / $failed_now failed**$( [ "$left" -gt 0 ] && echo " ($left gap file(s) skipped — see REPORT.md)" )
- Patch files + ledger entries: \`crates/treebank-$lang/patches/\`, \`ledger.json\`, \`LOCAL-PATCHES.md\`
- Agent log: \`corpus/$lang/reports/agent.log\` (local)
- CI re-proves the reconstruction invariant, corpus tests, and negative corpus.

🤖 Generated with [Claude Code](https://claude.com/claude-code)"; then
    echo "daily: $lang PR opened from $branch"
  else
    echo "daily: $lang PR creation failed — fixes are committed on $branch locally"
    overall=1
  fi
  git checkout -q "$base"
done

echo "=== treebank daily done ==="
exit "$overall"
