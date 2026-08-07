#!/usr/bin/env bash
# The daily checker. Cron runs this once a day:
#
#   0 6 * * * $HOME/treebank/scripts/daily.sh >> $HOME/treebank/daily.log 2>&1
#
# The script cds to the repo root itself and sets its own PATH, so the crontab
# line needs neither. That PATH bootstrap is load-bearing, not tidiness: cron
# runs with PATH=/usr/bin:/bin, and cargo, node/npm/npx, claude and
# tree-sitter all live outside it.
#
# For every vendored grammar (crates/treebank-*/ledger.json):
#
#   0. rank   — refresh corpus/<lang>/top-k.json. typescript self-serves from
#      npm-high-impact; rust needs the crates.io db dump CSVs in
#      corpus/rust/db (see scripts/bootstrap.sh) and falls back to the
#      existing list when they are absent. A language with neither a fresh
#      rank nor an existing list is skipped loudly rather than swept empty.
#   1. fetch  — re-resolve + download the corpus. npm resolves each package's
#      latest version at fetch time, so new releases arrive daily; crates.io
#      versions come from the rank list.
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
# One fix PR per language at a time. Committing on a branch and checking the
# base back out reverts the working tree, so an unmerged PR means tomorrow's
# sweep sees the same gaps — step 3 therefore refuses to run while a fix PR
# for that language is outstanding, and the run starts with a fast-forward
# pull so a merged one actually lands here.
#
# Env: CLAUDE_BIN (default claude), CLAUDE_MODEL (default sonnet),
#      TREEBANK_LIMIT (packages per ecosystem, default 100),
#      TREEBANK_RANK_K (rank list length, default 1000),
#      TREEBANK_AGENT (1 = run the fix agent and open PRs, 0 = fetch/sweep
#        only; default 1),
#      TREEBANK_FORCE_AGENT (1 = run the agent even when a fix PR for that
#        language is already outstanding; default 0),
#      TREEBANK_AGENT_TIMEOUT (wall-clock seconds per agent session, default
#        3600), TREEBANK_AGENT_BUDGET_USD (dollar cap per session, default 10),
#      TREEBANK_LOCK (lock file, default /tmp/treebank-daily.lock).
set -uo pipefail
cd "$(dirname "$0")/.." || { echo "daily: cannot cd to the repo root"; exit 1; }
ROOT="$PWD"
# cron's PATH is /usr/bin:/bin; none of the toolchain lives there.
export PATH="$HOME/.cargo/bin:$HOME/.local/bin:/usr/local/bin:$PATH"
CLAUDE_BIN="${CLAUDE_BIN:-claude}"
CLAUDE_MODEL="${CLAUDE_MODEL:-sonnet}"
LIMIT="${TREEBANK_LIMIT:-100}"
RANK_K="${TREEBANK_RANK_K:-1000}"
RUN_AGENT="${TREEBANK_AGENT:-1}"
AGENT_TIMEOUT="${TREEBANK_AGENT_TIMEOUT:-3600}"
AGENT_BUDGET="${TREEBANK_AGENT_BUDGET_USD:-10}"
TB="$ROOT/target/release/treebank"

# One run at a time. Without this, a run whose agent hangs is still going when
# tomorrow's cron fires and both would push branches.
#
# fd 9 is closed for every child below (the `9>&-` redirections). A flock lives
# on the open file description, not the process, so any child that inherits fd 9
# keeps the lock alive after this script exits — and `cargo build` spawns the
# sccache daemon, which outlives the run. Without those redirections the first
# run to start sccache wedges the lock and every later run prints "previous run
# still going" and does nothing, silently, exiting 0. Observed, not theoretical.
LOCK="${TREEBANK_LOCK:-/tmp/treebank-daily.lock}"
exec 9>"$LOCK" || { echo "daily: cannot open lock $LOCK"; exit 1; }
if ! flock -n 9; then
  echo "=== treebank daily: $(date '+%Y-%m-%d %H:%M') — previous run still going, skipping ==="
  exit 0
fi

echo "=== treebank daily: $(date '+%Y-%m-%d %H:%M') ==="

# Pull merged work in, so this checkout doesn't drift from the branch it
# tracks. Without it a merged fix never reaches this machine: the sweep keeps
# reporting gaps that main has already fixed, and the outstanding-PR guard
# below never clears. Fast-forward only, and only from a clean tree — an
# agent's unreviewed changes are never discarded to make room for a pull.
if [ -n "$(git status --porcelain --untracked-files=no)" ]; then
  echo "daily: working tree is dirty — skipping the pull, leaving local changes alone"
elif git pull --ff-only --quiet 2>/dev/null; then
  echo "daily: at $(git rev-parse --short HEAD) on $(git rev-parse --abbrev-ref HEAD)"
else
  echo "daily: git pull --ff-only did not apply (diverged, no upstream, or offline) — running against $(git rev-parse --short HEAD)"
fi 9>&-

cargo build --release --quiet 9>&- || { echo "daily: cargo build failed"; exit 1; }

overall=0
for ledger in "$ROOT"/crates/treebank-*/ledger.json; do
  lang=$(jq -r .grammar "$ledger")
  grammar="$ROOT/crates/treebank-$lang"
  report="$ROOT/corpus/$lang/reports/sweep.json"
  echo "--- $lang"

  # fetch reads corpus/<lang>/top-k.json, so a checkout with no corpus/ needs
  # rank first. Refresh it when the language can; fall back to the list on
  # disk; skip the language rather than sweep nothing.
  list="$ROOT/corpus/$lang/top-k.json"
  if "$TB" rank --lang "$lang" --k "$RANK_K" >/dev/null 2>&1; then
    echo "daily: $lang rank refreshed ($RANK_K packages)"
  elif [ -f "$list" ]; then
    echo "daily: $lang rank unavailable — reusing $list"
  else
    echo "daily: $lang has no package list and cannot rank — skipping (run scripts/bootstrap.sh)"
    overall=1
    continue
  fi

  if ! "$TB" fetch --lang "$lang" --limit "$LIMIT" 2>&1 | tail -1; then
    echo "daily: $lang fetch failed, sweeping the existing corpus anyway"
  fi
  if ! scripts/materialize.sh "$grammar" >/dev/null 2>&1; then
    echo "daily: $lang materialize FAILED"
    overall=1
    continue
  fi
  if ! "$TB" sweep --lang "$lang" --grammar "$grammar/build" 2>&1 | tail -2; then
    echo "daily: $lang sweep FAILED"
    overall=1
    continue
  fi

  gaps=$(jq .gap_files "$report")
  if [ "$gaps" -eq 0 ]; then
    echo "daily: $lang clean — no grammar gaps"
    continue
  fi

  if [ "$RUN_AGENT" != 1 ]; then
    echo "daily: $lang has $gaps gap file(s) — agent disabled (TREEBANK_AGENT=$RUN_AGENT), see REPORT.md"
    continue
  fi

  # Never redo work that is already waiting on a human.
  #
  # Step 5 commits the agent's fix onto a branch and then checks the base back
  # out, which reverts the working tree — so until that PR is merged AND pulled,
  # this checkout's grammar is still unfixed and the sweep still reports the
  # same gaps. Without this guard every unmerged day costs another agent
  # session and produces another near-identical PR.
  if [ "${TREEBANK_FORCE_AGENT:-0}" != 1 ]; then
    if ! prs=$(gh pr list --state all --limit 100 --json headRefName,url,state 2>/dev/null); then
      echo "daily: $lang cannot reach GitHub to check for an outstanding fix PR — not launching the agent"
      overall=1
      continue
    fi
    # Most recent PR whose branch is this language's; gh lists newest first.
    prior=$(jq -c --arg p "grammar-fixes/$lang-" \
      '[.[] | select(.headRefName | startswith($p))] | first // {}' <<<"$prs")
    case "$(jq -r '.state // empty' <<<"$prior")" in
      OPEN)
        echo "daily: $lang has $gaps gap file(s) but $(jq -r .url <<<"$prior") is still open — skipping the agent until it merges"
        continue ;;
      CLOSED)
        echo "daily: $lang has $gaps gap file(s); the last fix PR $(jq -r .url <<<"$prior") was closed without merging — not retrying (TREEBANK_FORCE_AGENT=1 to override)"
        continue ;;
    esac
    # A push or gh failure last time leaves the fix committed locally with no PR.
    stale=$(git branch --no-merged HEAD --list "grammar-fixes/$lang-*" | head -1 | tr -d ' *')
    if [ -n "$stale" ]; then
      echo "daily: $lang has an unmerged local branch $stale from an earlier failed push — skipping the agent"
      continue
    fi
  fi

  echo "daily: $lang has $gaps gap file(s) — launching fix agent (<=${AGENT_TIMEOUT}s, <=\$$AGENT_BUDGET)"
  timeout --signal=INT --kill-after=60 "$AGENT_TIMEOUT" \
    "$CLAUDE_BIN" -p "Read corpus/$lang/reports/REPORT.md and fix ALL of its gap
clusters, one at a time, exactly per the report's instructions. Edit grammar
sources in crates/treebank-$lang/build/ (the materialized tree — see
GRAMMARS.md). After each fix, run ../../scripts/check.sh from
crates/treebank-$lang until it prints CHECK OK. Capture each fix as patches/NNNN-*.patch with a
ledger.json entry and a LOCAL-PATCHES.md note, per GRAMMARS.md. Update the
ledger's corpus.sweep_patched numbers when done, and finish by running
scripts/verify.sh crates/treebank-$lang from the repo root — it must pass.
Do NOT git commit anything. If a cluster is genuinely beyond a minimal
grammar change, skip it and say so in your final message." \
    --model "$CLAUDE_MODEL" --max-turns 200 \
    --max-budget-usd "$AGENT_BUDGET" \
    --permission-mode bypassPermissions \
    > "$ROOT/corpus/$lang/reports/agent.log" 2>&1
  rc=$?
  case "$rc" in
    0) echo "daily: agent finished (log: corpus/$lang/reports/agent.log)" ;;
    124|137) echo "daily: agent hit the ${AGENT_TIMEOUT}s timeout and was killed — verifying whatever it left behind" ;;
    *) echo "daily: agent exited $rc (turn/budget cap or error) — verifying whatever it left behind" ;;
  esac

  # Trust nothing: re-sweep and verify to see what actually happened.
  "$TB" sweep --lang "$lang" --grammar "$grammar/build" 2>&1 | grep '^sweep:' | head -1
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
  if [ -z "$base" ]; then
    echo "daily: $lang detached HEAD — refusing to branch, changes left in working tree"
    overall=1
    continue
  fi
  git checkout -qb "$branch" || { echo "daily: $lang could not create $branch"; overall=1; continue; }
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
done 9>&-

echo "=== treebank daily done ==="
exit "$overall"
