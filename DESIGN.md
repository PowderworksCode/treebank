# Treebank & Package Room by Powderworks

The plans I have in mind involve parsing a lot of code. Tree Sitter is usually
pretty good, but sometimes various grammars fall behind the times. Keeping these
parsers up to do date is thankless, unpaid work and that's not going to change
anytime soon. So, I would like to automate the process of ensuring that there
are Tree Sitter grammars for the languages I care about. 

Treebank will be a repo that contains vendered grammars for various languages.
There are many, good grammars out there that just need a patch here and there to
get them to 100% coverage for what I care about. Package Room will be an online
service that grabs data from various package managers, runs the parsers across
packages and then kicks off an agent in a sandbox if it hits a part of the
language that it cannot parse. That agent will then put up a PR and I'll review
and merge it. On merge, a new release will be published for that treebank
grammar and pushed to cargo. It's a pretty straightforward loop I have in mind
and I'm hoping it can get the grammars I need for less than $50/month.

We are starting with packages now, and one day will move onto artifacts (like
the Linux Kernel, Postgres, Rails, etc.) to make sure we hitting less package
heavy languages like C as well as expanding coverage.

## Core loop

```
registry event (new version of a top-K package)
  → fetch tarball, extract source files
  → parse with our current grammar build
  → all clean? record results, done.
  → error? validity oracle: is the file even valid? (reference parser)
      → invalid: record as corpus noise, done.
      → valid: this is a grammar gap →
          fix agent (diagnose → minimal repro → patch → regenerate → test → sweep)
          → opens PR against the grammar repo
          → adversarial review agent tries to reject it
              → rejected: fix agent gets exactly one fix-up round
              → rejected again: park for human, cap spent
          → accepted: PR awaits human merge (human-in-the-loop for now)
          → on merge: ledger updated, grammar auto-published
```

Bootstraping the parser for a language just involves running this loop from the
top of the package list by descending popularity until it gets far enough down
that we have built up confidence that we can parse everything. Rust is the
initial target language: crates.io is easy to crawl, and Rust has no dialect
ambiguity, so file-to-grammar routing is naive to start (revisit when a
language that needs real routing — C/C++ headers, JS/TS, SQL flavors — comes
online).

### Validity oracle

"Our parser errored" does not mean "grammar bug": corpora are full of test
fixtures, templates, snippets, and other-dialect files. Before dispatching an
agent, adjudicate with the language's reference parser (rustc
`-Zunpretty=ast-tree`, `python -m ast`, `node --check`, `gcc -fsyntax-only`,
...). Files the reference rejects are recorded as noise, not bugs.

## Grammar repos and the patch ledger

One repo per grammar, e.g. `powderworks/treebank-rust`. Each repo records the
upstream grammar's `git_url` and `sha`, vendors the grammar source with all of
our patches already applied, and keeps a `patches/` directory containing every
individual patch it took to get from upstream to our tree. That way an
upstream maintainer who wants any of our fixes can pull the patch files
directly — the patches are the offer, the vendored tree is the product.

Each repo carries a machine-readable ledger (`ledger.json`) and a human log
(`LOCAL-PATCHES.md`):

```json
{
  "upstream": {
    "git_url": "https://github.com/tree-sitter/tree-sitter-rust",
    "sha": "77a3747...",
    "version": "0.24.2"
  },
  "patches": [
    {
      "id": 1,
      "title": "extern types in extern blocks",
      "file": "patches/0001-extern-types-in-extern-blocks.patch",
      "files": ["grammar.js"],
      "origin": "upstream PR #281",
      "evidence": {
        "repro": "extern \"C\" { pub type Foo; }",
        "first_seen": {"package": "web-sys", "version": "0.3.103"},
        "sweep_before": {"passed": 43601, "failed": 1725},
        "sweep_after": {"passed": 45071, "failed": 255}
      }
    }
  ]
}
```

The ledger is the source of truth for materializing any grammar: the
`upstream/` submodule (which must sit exactly at the ledger's pinned `sha`)
+ the `patches/` series applied in order + `tree-sitter generate` with the
ledger's pinned `generate_cli` version produce the gitignored working tree
`build/` that sweeps, tests and publishing consume (CI checks this).

The CLI pin is not bookkeeping: regenerating with a different CLI version can
silently change parsing behavior (found in practice: tree-sitter-cli 0.26.x
ships Unicode identifier tables that drop some XID_Start chars, breaking
`'K'`-style char literals — caught by the corpus sweep). Bumping the pinned
CLI is treated like a patch: full sweep, before/after numbers, ledger entry.

### Publishing

On merge, CI regenerates the parser and publishes the grammar as
`powderworks/treebank-<language>`, versioned as the upstream version plus a
build counter suffix: upstream `0.24.2` + our 7th release on that base →
**`0.24.2-7`**. Consumers pin to upstream semantics and can read our counter
independently.

### Upstream releases

When upstream publishes a new grammar version, automation pulls it, replays
the patch series, regenerates, sweeps, and opens a PR with the result:

- Clean replay + green sweep → PR is a rubber stamp.
- Conflicts or sweep regressions → a rebase agent job (same pipeline as a fix
  job) resolves them; patches upstream has absorbed are retired from the
  ledger (the best outcome — divergence shrinking on its own).

## Agent pipeline

Agents run headless (`claude -p`) from the job queue, using the Claude Max
account. Design constraints that matter:

- **Job classes.** `grammar.js` rule fixes are declarative, verifiable, and
  cheap — route to a mid-tier model. `scanner.c` (external scanner) fixes are
  C programming with state-machine and serialization concerns — route to the
  top model and expect longer sessions. Classify by whether the failing
  region touches an external token.
- **Caps everywhere.** Fix agent: bounded attempts (~3) before parking the
  cluster for a human. Adversarial review: accept/reject with reasons.
  Fix-up: exactly one round, as designed. A pathological cluster must cost a
  bounded amount of usage allowance, never a week of it.
- **PRs are built for 90-second review.** Every PR contains: the minimal
  repro, the patch, regenerated files, the ledger entry, grammar corpus test
  results, and before/after sweep numbers. The human decision should be a
  glance, because human attention is the system's scarcest resource.

The adversarial reviewer's brief is to *refute*: find valid code the patch
breaks, invalid code it newly accepts, or a simpler patch. It runs the same
verification harness independently rather than trusting the fix agent's
numbers.

## Verification (CI, per PR)

Runs on GitHub Actions (free for public repos), so evidence lives in the PR
and merges never depend on our infrastructure being up:

1. `tree-sitter generate` reproduces the committed generated files exactly.
2. Grammar's own corpus tests pass (`tree-sitter test`).
3. Full package-corpus sweep: pass count must not regress; the fix's target
   files must now pass.
4. **Negative tests**: the repro corpus of reference-rejected files must
   still be rejected. Sweeps only catch rejects-valid-code; this catches
   accepts-invalid-code, the direction agents drift when optimizing pass
   rates (found in practice: upstream rust accepted `' a` as a lifetime).

## Infrastructure

- **Runner**: my laptop for now — crawler, job queue, agent sessions, and
  tarball cache all run locally. When this needs to run unattended, it moves
  to Fly.io with agent sessions in Sprites (ephemeral sandboxed VMs spun up
  per job and torn down after). This workload is mostly agents waiting on
  model responses; cores only matter for sweeps.
- **GitHub**: public org `powderworks`, one repo per grammar
  (`treebank-<language>`), Actions for all per-PR verification and publish-on-
  merge.
- **PackageRoom.dev**: static-first on Cloudflare Pages/Workers; sweep
  results and ledgers in R2/D1. Displays per-package parse status and
  history, per-grammar coverage and patch ledger, crawl progress, and the
  parked-for-human queue. (Future: impact analysis across the corpus.)

## Decisions

- Rust first, run from the laptop: registry "events" are just naive polling
  of crates.io. Per-ecosystem feeds come with later languages.
- Dialect routing is naive for now — Rust doesn't need it.
- The ledger lives per grammar repo, keeping PRs self-contained.

