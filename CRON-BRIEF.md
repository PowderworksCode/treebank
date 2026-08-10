You are the treebank-cron session. Read this in full before doing anything.

## Your workspace

**`~/treebank` — and this path is permanent.** Cron will point at it, so unlike
the other trees on this box it must not be treated as scratch. Do not move or
rename it. `~/treebank-grammars` is a second checkout where another session is
adding grammars; do not touch it.

This repo is unrelated to the powderworks fleet (entl/infact/straitjacket/
cowbird). Ignore all of that.

## The task

**Install the daily cron job, run it, and confirm the whole loop actually
works.** `todo.txt` lists "Set up cron" and this is it.

Not just "the crontab line exists". The deliverable is evidence that a real run
completes end to end and does something sensible, plus whatever had to be fixed
or installed to get there.

## What the job is

`scripts/daily.sh` is the entrypoint. Per vendored grammar it:

1. **fetch** — re-resolves and downloads the corpus. npm resolves each package's
   latest version at fetch time, so new releases arrive on their own.
2. **sweep** — parses everything, adjudicates failures against the reference
   parser, writes `corpus/<lang>/reports/sweep.json` and an agent-ready
   `REPORT.md`.
3. **agent** — *only if the report shows grammar gaps*, launches one Claude
   session per language per day, pointed at REPORT.md.
4. **re-sweep + verify** — records what the agent actually achieved.

Nothing is committed; changes wait in the working tree for human review.

Env it honours: `CLAUDE_BIN` (default `claude`), `CLAUDE_MODEL` (default
`sonnet`), `TREEBANK_LIMIT` (packages per ecosystem, default 100).

## Read this before you install anything

**Step 3 spends money without a human present.** It launches a Claude session
per language per day. Two grammars are vendored today, so that is up to two
sessions daily, and the grammars session may add three more languages shortly.
Understand the bound before you arm it — read how `daily.sh` invokes the agent,
what limits it sets, and what happens if the agent runs long or loops.

If you judge the spend or the blast radius to be unclear, **install the cron
entry with the agent step disabled or gated first**, prove steps 1, 2 and 4
work, and tell the user exactly what arming step 3 would cost. An unattended
daily agent that nobody has watched run once is not something to switch on
quietly.

## Known obstacles, measured before you launched

- **The crontab line in README.md and daily.sh is macOS**:
  `0 6 * * * cd /Users/zackmaril/powderworks/treebank && scripts/daily.sh >> daily.log 2>&1`
  This box is Linux and the path is `~/treebank`. Both files need correcting,
  and that correction is worth a PR on its own.
- **`tree-sitter` CLI is NOT installed.** `GRAMMARS.md` pins **0.25.10** and is
  emphatic about why: 0.26.x ships Unicode identifier tables that wrongly drop
  some XID_Start characters (U+212A KELVIN SIGN), which broke `'K'`-style char
  literals and was caught only by a corpus sweep. Install exactly 0.25.10.
  `scripts/verify.sh` needs it.
- **`corpus/` does not exist yet**; both languages start from empty.
- **The rust corpus needs a local crates.io db dump.** The README says `rank`
  for rust "needs the extracted crates.io db dump CSVs in `corpus/rust/db/`".
  That dump is large and may not be obtainable here. TypeScript self-serves from
  the npm registry and should work unaided. **If rust cannot rank, say so
  plainly and get the loop working for typescript rather than faking a rust
  corpus.** A daily job that works for one language is a real result; one that
  silently sweeps an empty rust corpus is worse than nothing.
- Present: `cargo`, `node`, `npm`, `jq`. Absent: `tree-sitter`, and no JDK or
  .NET (irrelevant to you unless the grammars session lands Java or C#).

## What "alright" means

Report all of it, with output rather than assertion:

- a full `scripts/daily.sh` run, timed, with what each step did;
- the sweep numbers per language, and whether they match the ledger's recorded
  `sweep_patched` figures — a mismatch is a finding, not a nuisance;
- `scripts/verify.sh` passing per grammar (the reconstruction invariant:
  upstream at the pinned sha + patches + generate == the vendored tree, byte for
  byte);
- whether the incremental sweep cache behaves — a second run with no changes
  should be near-instant, and any grammar change should invalidate it wholly;
- the installed crontab entry, and evidence cron actually fires it, not just
  that `crontab -l` prints it.

## Working practice

- You MAY commit, push and open PRs on `PowderworksCode/treebank`. Do not merge
  your own PRs and do not force-push main.
- This repo is on **plain github.com** — `gh` is authenticated as `zmaril`. Do
  NOT set `GH_HOST=github.int.exe.xyz`; that proxy serves a different org.
- Fixes you make to get the loop running (the macOS path, a missing dependency,
  a script bug) are worth landing. The point of this task is that the next
  person does not repeat it.
- The repo's own CI is `.github/workflows/verify-grammars.yml`. Do not break it.
- Ask the user directly if something is genuinely ambiguous — particularly
  anything about arming the agent step.

## Operational

- Disk ~278G free; corpora will grow it. Check `df -h /` before large fetches.
- Run cargo plainly; sccache is the global rustc-wrapper and incremental is
  disabled in `~/.cargo/config.toml` so that it works.

## First move

Read `README.md`, `DESIGN.md`, `GRAMMARS.md` and `scripts/daily.sh` in full.
Then get one `treebank sweep` working by hand for typescript before you touch
cron at all — if the loop does not work manually, a schedule only hides that.
