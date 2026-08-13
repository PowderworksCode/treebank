#!/usr/bin/env bash
# The daily checker. Cron runs the sweep once a day and the reaper often:
#
#   0 6    * * * $HOME/treebank/scripts/daily.sh >> $HOME/treebank/daily.log 2>&1
#   */15 * * * * $HOME/treebank/scripts/daily.sh --reap >> $HOME/treebank/daily.log 2>&1
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
#   2. materialize + sweep — rebuild build/ from the pinned upstream submodule
#      plus patches/, then parse everything, adjudicate failures with the
#      reference parser, write corpus/<lang>/reports/sweep.json + REPORT.md.
#   3. agent  — only if the report shows grammar gaps: a fix agent is started
#      in the herdr fleet as `tbfix-<lang>`, in a git worktree of its own,
#      pointed at REPORT.md. It fixes clusters in build/, loops on
#      scripts/check.sh, captures patches + ledger entries per GRAMMARS.md,
#      and commits. It does not push and does not open PRs.
#   4. handoff — the agent outlives this script. A run waits a short while
#      (TREEBANK_HANDOFF) for the easy case and then leaves it alone; under
#      --permission-mode auto a blocked agent is waiting on a person, not
#      failing. `daily.sh --reap` finishes whatever settled since, and is
#      meant to run on a short cron interval.
#   5. PR — whoever finds the agent settled re-proves the invariant with
#      scripts/verify.sh against what was actually committed, then submits
#      the branch as the next layer of that language's stack and removes the
#      worktree. Merging stays human. On verify failure the worktree is left
#      standing for inspection and nothing goes up.
#
# Stacked PRs, one stack per language. The open fix PRs for a language ARE its
# stack, bottom to top, and a new run branches off the top rather than off the
# trunk. That is what stops day two duplicating day one: the agent starts from
# a tree with yesterday's patches already applied, so its sweep reports only
# what is still broken and its patches/NNNN numbering continues instead of
# colliding. GitHub re-targets the layers above whenever one merges. Because
# stacks merge bottom-up, a stack that nobody reviews blocks the language, so
# TREEBANK_STACK_MAX caps how deep it may get before the job stops adding.
#
# Env: CLAUDE_MODEL (agent model, default sonnet),
#      TREEBANK_LIMIT (packages per ecosystem, default 100),
#      TREEBANK_RANK_K (rank list length, default 1000),
#      TREEBANK_AGENT (1 = run the fix agent and open PRs, 0 = fetch/sweep
#        only; default 1),
#      TREEBANK_FORCE_AGENT (1 = launch even when the stack is already at its
#        depth cap; default 0),
#      TREEBANK_HANDOFF (seconds a run waits for a fresh agent before handing
#        it to --reap, default 600),
#      TREEBANK_STACK_MAX (open fix PRs per language before the job stops
#        adding layers, default 3),
#      TREEBANK_HERDR (herdr binary, default herdr),
#      TREEBANK_NOTIFY_CMD (optional command called with one argument when a
#        run needs a human),
#      TREEBANK_LOCK (lock file, default /tmp/treebank-daily.lock).
set -uo pipefail
cd "$(dirname "$0")/.." || { echo "daily: cannot cd to the repo root"; exit 1; }
ROOT="$PWD"
# cron's PATH is /usr/bin:/bin; none of the toolchain lives there.
export PATH="$HOME/.cargo/bin:$HOME/.local/bin:/usr/local/bin:$PATH"
CLAUDE_MODEL="${CLAUDE_MODEL:-sonnet}"
LIMIT="${TREEBANK_LIMIT:-100}"
RANK_K="${TREEBANK_RANK_K:-1000}"
RUN_AGENT="${TREEBANK_AGENT:-1}"
HANDOFF_WAIT="${TREEBANK_HANDOFF:-600}"
STACK_MAX="${TREEBANK_STACK_MAX:-3}"
HERDR="${TREEBANK_HERDR:-herdr}"
TB="$ROOT/target/release/treebank"
MODE=run
case "${1:-}" in
  "")      ;;
  --reap)  MODE=reap ;;
  *)       echo "daily: unknown argument '$1' (only --reap)"; exit 2 ;;
esac

# Everything branches from the branch this checkout tracks, and this checkout
# never leaves it — the agents work in worktrees.
TRUNK=$(git rev-parse --abbrev-ref HEAD)
[ "$TRUNK" = HEAD ] && { echo "daily: detached HEAD — refusing to run"; exit 1; }

# --- the fix agent, and the fleet it runs in -----------------------------------
#
# The agent no longer runs as a child of this script. It runs as a named agent
# in the herdr fleet, in a git worktree of its own, and it outlives this run:
# under --permission-mode auto it can stop and wait for a human, and that wait
# is measured in your sleep, not in wall-clock this script can afford to hold.
# So a run LAUNCHES, waits a short while for the easy case, and hands off.
# `daily.sh --reap` finishes whatever settled since.
#
# The worktree is what makes that safe. This checkout is what cron depends on
# being on its tracking branch; an agent committing here could leave it parked
# on a fix branch, after which every later `git pull --ff-only` fast-forwards
# the wrong branch and the drift compounds silently. In its own worktree the
# agent cannot touch this checkout at all, and languages stop being serialized.

herdr_up() { command -v "$HERDR" >/dev/null 2>&1 && "$HERDR" status server >/dev/null 2>&1; }

# The live agent's state, or non-zero if there is no such agent.
agent_state() {
  local s
  s=$("$HERDR" agent get "tbfix-$1" 2>/dev/null | jq -r '.result.agent.agent_status // empty') || return 1
  [ -n "$s" ] || return 1
  echo "$s"
}

reldir_for_lang() {
  local l d
  for l in "$ROOT"/crates/treebank-*/ledger.json; do
    if [ "$(jq -r .grammar "$l")" = "$1" ]; then
      d=$(dirname "$l"); echo "${d#"$ROOT"/}"; return 0
    fi
  done
  return 1
}

notify() {
  echo "daily: NOTIFY: $*"
  "$HERDR" notification show "treebank daily" --body "$*" --sound request >/dev/null 2>&1 || true
  [ -n "${TREEBANK_NOTIFY_CMD:-}" ] && "$TREEBANK_NOTIFY_CMD" "$*" >/dev/null 2>&1
  return 0
}

# A fresh worktree is not usable as-is: `git worktree add` checks out tracked
# files only, so the upstream submodule is absent (materialize.sh refuses to
# run without it at the pinned sha) and corpus/ and target/ are absent because
# they are gitignored — but check.sh sweeps the corpus and resolves
# TREEBANK_BIN through target/. Symlinks rather than copies: the corpus is
# gigabytes and the binary is already built.
prepare_worktree() {
  local wt=$1 reldir=$2 common
  git -C "$wt" submodule update --init "$reldir/upstream" >/dev/null 2>&1 || return 1
  ln -sfn "$ROOT/corpus" "$wt/corpus"
  ln -sfn "$ROOT/target" "$wt/target"
  # .gitignore lists `/corpus/` with a trailing slash, which matches only
  # directories — a symlink is a file to git, so without this it shows up as
  # untracked in every status the agent and this script run.
  common=$(git -C "$wt" rev-parse --path-format=absolute --git-common-dir 2>/dev/null) || return 1
  mkdir -p "$common/info"
  grep -qx '/corpus' "$common/info/exclude" 2>/dev/null \
    || printf '/corpus\n/target\n' >> "$common/info/exclude"
}

# The PR body is computed here, from the sweep that actually ran, rather than
# recalled by the agent afterwards.
write_pr_body() {
  local lang=$1 gaps=$2 out=$3
  local report="$ROOT/corpus/$lang/reports/sweep.json"
  cat > "$out" <<BODY
Automated fix run from \`scripts/daily.sh\`.

- Gap files in today's sweep: **$gaps**
- Corpus at sweep time: $(jq .files "$report" 2>/dev/null) files, $(jq .passed "$report" 2>/dev/null) passed / $(jq .failed "$report" 2>/dev/null) failed
- Patches and evidence: \`$(reldir_for_lang "$lang")/patches/\`, \`ledger.json\`, \`LOCAL-PATCHES.md\`
- CI re-proves the materialization invariant, corpus tests and negative corpus.

Generated by the daily job.
BODY
}

fix_prompt() {
  local lang=$1 reldir=$2
  cat <<PROMPT
Read corpus/$lang/reports/REPORT.md and fix ALL of its gap clusters, one at a
time, exactly per the report's instructions. Edit grammar sources in
$reldir/build/ (the materialized tree — see GRAMMARS.md). After each fix, run
../../scripts/check.sh from $reldir until it prints CHECK OK. Capture each fix
as patches/NNNN-*.patch with a ledger.json entry and a LOCAL-PATCHES.md note,
per GRAMMARS.md; keep numbering after the highest patch already there. Update
the ledger's corpus.sweep_patched numbers when done, then run
scripts/verify.sh $reldir from the repo root — it must pass.

Finally: git add $reldir and git commit it, with a message naming the clusters
you fixed. Commit only — do NOT push and do NOT open a PR. The daily job does
that, so the stack wiring stays deterministic.

You are in a git worktree of your own; nothing you do here touches the main
checkout. If a cluster is genuinely beyond a minimal grammar change, skip it
and say so in your final message.
PROMPT
}

launch_fix() {
  local lang=$1 reldir=$2 gaps=$3 branch=$4 base=$5
  local out ws pane wt

  if ! out=$("$HERDR" worktree create --cwd "$ROOT" --branch "$branch" --base "$base" \
              --label "tbfix $lang" --no-focus 2>&1); then
    echo "daily: $lang could not create a worktree on $branch off $base: $(head -c 300 <<<"$out")"
    return 1
  fi
  ws=$(jq -r '.result.workspace.workspace_id' <<<"$out")
  pane=$(jq -r '.result.root_pane.pane_id' <<<"$out")
  wt=$(jq -r '.result.workspace.worktree.checkout_path' <<<"$out")

  if ! prepare_worktree "$wt" "$reldir"; then
    echo "daily: $lang worktree setup failed — removing $wt"
    drop_worktree "$ws" "$wt"
    return 1
  fi
  write_pr_body "$lang" "$gaps" "$ROOT/corpus/$lang/reports/PR-BODY.md"

  if ! "$HERDR" agent start "tbfix-$lang" --kind claude --pane "$pane" \
        -- --model "$CLAUDE_MODEL" --permission-mode auto >/dev/null 2>&1; then
    echo "daily: $lang could not start tbfix-$lang in $ws — worktree left at $wt"
    return 1
  fi
  "$HERDR" agent prompt "tbfix-$lang" "$(fix_prompt "$lang" "$reldir")" \
      --wait --timeout $((HANDOFF_WAIT * 1000)) >/dev/null 2>&1

  case "$(agent_state "$lang" || echo gone)" in
    idle|done) finish_fix "$lang" "$reldir" "$ws" "$wt" "$branch" ;;
    blocked)
      notify "$lang fix agent is blocked on a prompt — answer it in herdr workspace $ws"
      return 0 ;;
    *)
      echo "daily: $lang agent still working after ${HANDOFF_WAIT}s — handed off to --reap (workspace $ws)"
      return 0 ;;
  esac
}

# Take a settled agent's worktree the rest of the way: prove the invariant, put
# the layer on the stack, tear the worktree down. Used both inline (when the
# agent finished inside the handoff window) and by --reap.
finish_fix() {
  local lang=$1 reldir=$2 ws=$3 wt=$4 branch=$5
  local commits prs under
  local -a below

  archive_transcript "$lang" "$wt"

  if [ -n "$(git -C "$wt" status --porcelain -- "$reldir")" ]; then
    echo "daily: $lang agent left uncommitted changes in $wt — no PR, worktree kept"
    return 1
  fi
  commits=$(git -C "$wt" rev-list --count "$TRUNK..$branch" 2>/dev/null || echo 0)
  if [ "$commits" -eq 0 ]; then
    echo "daily: $lang agent committed nothing — removing $wt"
    drop_worktree "$ws" "$wt"
    git branch -D "$branch" >/dev/null 2>&1
    return 0
  fi
  # Trust nothing the agent reported: re-prove materialization, corpus tests
  # and the negative corpus against what it actually committed.
  if ! scripts/verify.sh "$wt/$reldir" >/dev/null 2>&1; then
    echo "daily: $lang verify FAILED on $branch — no PR, worktree kept at $wt for inspection"
    notify "$lang fix failed verification — $branch is not going up"
    return 1
  fi
  echo "daily: $lang verify ok ($commits commit(s) on $branch)"

  prs=$(gh pr list --state open --limit 100 --json number,headRefName 2>/dev/null || echo '[]')
  mapfile -t below < <(jq -r --arg p "grammar-fixes/$lang-" \
    '[.[] | select(.headRefName | startswith($p))] | sort_by(.number) | .[].headRefName' <<<"$prs")

  # The layer immediately below this one, for the fallback path. `set -u` makes
  # a negative subscript on an empty array noisy, so ask the length first.
  if [ "${#below[@]}" -gt 0 ]; then under="${below[-1]}"; else under="$TRUNK"; fi

  # `gh stack submit` opens a full-screen editor on a TTY; --auto is what makes
  # it usable unattended, and --open stops every layer being created as a
  # draft. The extension is v0.1.0, so a plain PR against the layer below is
  # the fallback — a day's work is not worth losing to a young CLI.
  if (cd "$wt" \
        && gh stack init "${below[@]}" "$branch" --base "$TRUNK" >/dev/null 2>&1 \
        && gh stack submit --auto --open >/dev/null 2>&1); then
    echo "daily: $lang submitted $branch onto the stack ($((${#below[@]} + 1)) layer(s))"
  elif (cd "$wt" && git push -q -u origin "$branch" \
        && gh pr create --base "$under" --head "$branch" \
             --title "$(basename "$reldir"): grammar fixes from daily sweep ($(date +%Y-%m-%d))" \
             --body-file "$ROOT/corpus/$lang/reports/PR-BODY.md" >/dev/null); then
    echo "daily: $lang gh stack failed — opened a plain PR for $branch instead"
  else
    echo "daily: $lang could not open a PR — $branch is committed locally in $wt"
    notify "$lang fix is committed but has no PR — see $wt"
    return 1
  fi

  drop_worktree "$ws" "$wt" \
    && echo "daily: $lang worktree removed; $branch stays for the PR"
  return 0
}

# A worktree may outlive its herdr workspace — the server can restart, or the
# workspace can be closed by hand — and then there is no workspace id to remove
# it by. git still knows about it.
drop_worktree() {
  local ws=$1 path=$2
  [ -n "$ws" ] && "$HERDR" worktree remove --workspace "$ws" --force >/dev/null 2>&1 && return 0
  git worktree remove --force "$path" >/dev/null 2>&1
}

# The pane runs on the alternate screen, so its scrollback is not recoverable.
# The session transcript is, and it is the better artifact anyway.
archive_transcript() {
  local lang=$1 sid src
  sid=$("$HERDR" agent get "tbfix-$lang" 2>/dev/null | jq -r '.result.agent.agent_session.value // empty')
  [ -n "$sid" ] || return 0
  src=$(find "$HOME/.claude/projects" -name "$sid.jsonl" -print -quit 2>/dev/null)
  [ -n "$src" ] && cp "$src" "$ROOT/corpus/$lang/reports/agent.jsonl" 2>/dev/null
  return 0
}

# The herdr workspace holding a worktree, if the server is up and knows about
# it. Empty is a normal answer: drop_worktree falls back to plain git.
workspace_for() {
  herdr_up || return 0
  "$HERDR" worktree list --cwd "$ROOT" 2>/dev/null \
    | jq -r --arg b "$1" '.result.worktrees[]? | select(.branch == $b) | .open_workspace_id // empty' \
    | head -1
}

# --reap: finish what settled since the last run. Cheap enough for a short cron
# interval — it starts no agents and spends nothing unless a worktree is
# waiting, and it says nothing at all when there is none.
#
# git, not herdr, is what enumerates: a fix worktree is a git fact, and the
# reaper must still be able to finish one when the herdr server is down. herdr
# is consulted only for the agent's state and its workspace, both optional.
reap() {
  local rc=0 branch path lang reldir state ws found=0
  while IFS=$'\t' read -r branch path; do
    [ -n "$branch" ] || continue
    found=1
    lang=$(sed -E 's#^grammar-fixes/(.+)-[0-9]{8}-[0-9]{4}$#\1#' <<<"$branch")
    reldir=$(reldir_for_lang "$lang") || { echo "reap: $branch — no ledger for '$lang', skipping"; rc=1; continue; }
    ws=$(workspace_for "$branch")
    state=$(agent_state "$lang" || echo gone)
    case "$state" in
      working)
        echo "reap: $lang still working" ;;
      blocked)
        notify "$lang fix agent is blocked — answer it in herdr${ws:+ workspace $ws}" ;;
      *)
        echo "reap: $lang agent $state — finishing $branch"
        finish_fix "$lang" "$reldir" "$ws" "$path" "$branch" || rc=1 ;;
    esac
  done < <(git worktree list --porcelain \
             | awk '/^worktree /{p=substr($0,10)} /^branch refs\/heads\/grammar-fixes\//{print substr($0,19) "\t" p}')
  [ "$found" = 1 ] || return 0
  return "$rc"
}

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
  if [ "$MODE" = reap ]; then
    echo "=== treebank reap: $(date '+%Y-%m-%d %H:%M') — a run holds the lock, skipping ==="
  else
    echo "=== treebank daily: $(date '+%Y-%m-%d %H:%M') — previous run still going, skipping ==="
  fi
  exit 0
fi

if [ "$MODE" = reap ]; then
  # Silent when there is nothing waiting: this runs every 15 minutes, and a
  # log that fills with "nothing to do" is a log nobody reads.
  out=$(reap 9>&-); rc=$?
  [ -n "$out" ] && printf '=== treebank reap: %s ===\n%s\n' "$(date '+%Y-%m-%d %H:%M')" "$out"
  exit "$rc"
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

# A pull that moves an `upstream` submodule pointer does not move the submodule
# working tree, and materialize.sh refuses to run when the checked-out sha is
# not the one ledger.json pins — so without this every grammar fails to
# materialize the day after a submodule bump lands. --init also covers the
# grammar added by someone else since this checkout was made.
git submodule update --init --quiet 9>&- \
  || echo "daily: git submodule update failed — grammars whose upstream moved will fail to materialize"

cargo build --release --quiet 9>&- || { echo "daily: cargo build failed"; exit 1; }

overall=0
for ledger in "$ROOT"/crates/treebank-*/ledger.json; do
  lang=$(jq -r .grammar "$ledger")
  # The directory, not "treebank-$lang": the two disagree (grammar "c-sharp"
  # lives in crates/treebank-csharp), and deriving the path from the language
  # name sends materialize.sh at a directory that does not exist.
  grammar=$(dirname "$ledger")
  reldir=${grammar#"$ROOT"/}          # crates/treebank-csharp
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

  # How many packages this grammar's numbers were measured at. check.sh takes
  # its PASS_BEFORE baseline from the ledger's corpus.sweep_patched.passed, so
  # if the daily corpus is a different size from the one that produced those
  # numbers the comparison is meaningless and the fix agent can never go green
  # — its only way out is to overwrite corpus.sweep_patched with whatever the
  # smaller corpus gave, which silently replaces the recorded evidence.
  # Measured: bash's ledger said 492 packages / 25,662 files / 25,313 passed
  # while the daily job fetched 99 / 9,094 / 9,016, and the agent duly rewrote
  # the ledger to the smaller figures. So the size lives next to the numbers
  # it produced. Grammars that do not set it keep $TREEBANK_LIMIT exactly as
  # before.
  limit=$(jq -r '.corpus.fetch_limit // empty' "$ledger")
  [ -n "$limit" ] || limit="$LIMIT"
  [ "$limit" = "$LIMIT" ] || echo "daily: $lang fetches $limit packages (ledger corpus.fetch_limit)"
  if ! "$TB" fetch --lang "$lang" --limit "$limit" 2>&1 | tail -1; then
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
  if ! herdr_up; then
    echo "daily: $lang has $gaps gap file(s) but the herdr server is unreachable — no agent runtime, see REPORT.md"
    overall=1
    continue
  fi

  # An agent for this language is still live from an earlier run, so its work
  # has not reached a PR yet and a second one would duplicate it. Agent names
  # are unique among live agents so `agent start` would fail anyway; this just
  # says why.
  if state=$(agent_state "$lang"); then
    echo "daily: $lang has $gaps gap file(s) but tbfix-$lang is still live ($state) — leaving it to finish"
    continue
  fi

  if ! prs=$(gh pr list --state open --limit 100 --json number,headRefName,url 2>/dev/null); then
    echo "daily: $lang cannot reach GitHub to size the fix stack — not launching the agent"
    overall=1
    continue
  fi
  stack=$(jq -c --arg p "grammar-fixes/$lang-" \
    '[.[] | select(.headRefName | startswith($p))] | sort_by(.number)' <<<"$prs")
  depth=$(jq length <<<"$stack")
  if [ "$depth" -ge "$STACK_MAX" ] && [ "${TREEBANK_FORCE_AGENT:-0}" != 1 ]; then
    echo "daily: $lang has $gaps gap file(s) but its fix stack is already $depth deep (max $STACK_MAX);"
    echo "daily: $lang stacks merge bottom-up, so nothing more can land until $(jq -r '.[0].url' <<<"$stack") is reviewed"
    continue
  fi
  if [ "$depth" -eq 0 ]; then
    base="$TRUNK"
  else
    base=$(jq -r '.[-1].headRefName' <<<"$stack")
  fi

  branch="grammar-fixes/$lang-$(date +%Y%m%d-%H%M)"
  echo "daily: $lang has $gaps gap file(s) — fixing on $branch off $base (stack $depth -> $((depth + 1)))"
  launch_fix "$lang" "$reldir" "$gaps" "$branch" "$base" || overall=1
done 9>&-

echo "=== treebank daily done ==="
exit "$overall"
