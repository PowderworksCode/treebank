# The replacement invariant

*Status: proposal. Nothing below is implemented. It must be settled before any
owned `grammar.js` is written, because if no replacement invariant can be
constructed that is worth trusting, that finding should stop the programme —
and it is cheaper to find that out now.*

---

## 1. What is being destroyed

`GRAMMARS.md` rests on one sentence:

> upstream submodule @ the ledger's pinned sha, pristine + `patches/` applied
> in order + `tree-sitter generate` at the pinned CLI → `build/`

`scripts/verify.sh` checks it per grammar, CI runs it per grammar, and it is
treebank's entire credibility claim. Owning a grammar destroys it: there is no
upstream to reconstruct from and no patch series to replay.

Before replacing it, it is worth being exact about what it proved, because the
replacement has to cover the same jobs and the temptation is to replace the
*feeling* of it rather than the content.

**What it proved.** That the parser you are running is derivable, by a
published mechanical procedure, from a public commit of someone else's
repository plus a patch series you can read in full. Nothing was inserted that
is not visible. It is **total** — an equality, not a sample — and **cheap**,
seconds per grammar.

**What it never proved.** Anything at all about behaviour. A grammar that
rejects half of real Rust satisfies it perfectly. The invariant is about
*provenance*, and every behavioural claim treebank makes today — the sweeps,
`gap_files`, the negative corpus — sits **outside** it, in `ledger.json`, where
nothing recomputes it. `verify.sh` runs the grammar's own corpus tests and the
negative corpus; it does not run a sweep.

That is the asymmetry the replacement should exploit. The load-bearing
invariant is the one that says the least about whether the parser works.

---

## 2. The four jobs one sentence was doing

| # | job | survives ownership? |
|---|---|---|
| J1 | the generated parser corresponds to a grammar source you can read | **yes**, and more simply |
| J2 | nothing was inserted that is not visible | **yes** — the grammar is in git, not behind a submodule |
| J3 | this parser behaves like the thing the ecosystem trusts | **no** — this is what ownership breaks |
| J4 | changes are attributable and reviewable one at a time | **no** — there is no patch series; commits replace it |

J1 and J2 survive almost unchanged. J3 is the real loss, and it is the one the
replacement must earn back. J4 degrades from "a numbered patch with a ledger
entry" to "a commit", which is weaker in exactly one way — a patch series is
reviewable against a fixed base forever, a commit history is not — and that
loss should be recorded rather than argued away.

---

## 3. The replacement

Not one invariant. Five, because one sentence was doing four jobs and the fifth
is the reason for owning the grammar at all.

### R1 — Reproducible generation (replaces J1, J2)

```
grammar.js + scanner.c (ours, in git)
  + tree-sitter generate at ledger.generate_cli
  -> build/, byte for byte
```

Strictly simpler than today's: no submodule pointer to agree with a ledger, no
patch series to apply, no `npm ci`. Same check, fewer failure modes. Cost:
unchanged, seconds.

### R2 — Differential equivalence against the grammar we replaced (replaces J3)

**The upstream submodule is not deleted when a grammar becomes owned. It is
demoted from dependency to fixture** — moved from `upstream/` to `reference/`,
untouched by `materialize.sh`, pinned forever at the sha it was pinned at on
the day of the replacement.

The claim:

> Over the grammar's full corpus of N files, the owned grammar and the
> reference grammar return the same error/no-error verdict on N−K files. Each
> of the K disagreements is either (a) fixed, or (b) listed in
> `ledger.differential.divergences`, with the construct, the direction, the oracle's adjudication, and the reason.
> **K_unadjudicated = 0** is the invariant. K itself is a number, not a
> threshold.

The unit is the verdict, not the tree, because the trees *will* differ — the
ontology renames supertypes and may split rules. A tree-equality differential
would fail by construction on the first file and would be measuring the wrong
thing.

Every disagreement is adjudicated by the language's existing pinned oracle, so
the two directions have different meanings and different costs:

- **owned rejects, reference accepts** — if the oracle says the file is valid,
  the owned grammar has a gap. This is the dangerous direction and it is what
  the corpus is good at finding.
- **owned accepts, reference rejects** — if the oracle says invalid, the owned
  grammar has widened. Sweeps never catch this; only the negative corpus and
  the conformance battery do, which is why R4 is not optional.

### R3 — Oracle agreement (unchanged, and now load-bearing)

The existing sweep: `gap_files = 0`, or a ledgered list. Ownership changes
nothing about it except that it stops being a report and starts being the
thing standing between the grammar and a consumer.

### R4 — Negative corpus and conformance battery (unchanged, and now a duty)

`test/negative/` already exists and is already checked. Ownership adds an
obligation the vendored model could shrug off: **where an official conformance
suite exists, the owned grammar must run it, with every known failure
ledgered.** Precedent is already in the tree — treebank-toml records
`712/712 against toml-lang/toml-test's TOML 1.1.0 expectations` plus a 21-case
hand-built encoding battery. Under vendoring a conformance failure is
upstream's problem. Under ownership it is ours, and it should be a gate.

### R5 — Ontology conformance (new; the reason for the programme)

`treebank ontology` per `docs/ONTOLOGY.md` §5: the closed vocabulary, public
spellings, total node coverage, declared containments. This is the invariant
that has no analogue today — it is the thing that could not be enforced while
the grammar belonged to someone else, and it is what the other four are in
service of.

---

## 4. Is this stronger for a consumer?

**Yes, on the question a consumer is actually asking, and it is not close.**

Today's invariant answers *where did this parser come from*. It is a
**transitive trust** claim: treebank added nothing you cannot see to a thing
you had already decided to trust. Its force comes entirely from the consumer's
prior trust in upstream — and that prior is often unexamined. `tree-sitter-json`
is 125 lines pinned by nvim-treesitter, Helix and Zed at the same commit; the
reason to trust it is that everyone trusts it.

The replacement answers *will this parse my code*, directly: 5,657 real files,
0 gaps against V8's `JSON.parse`, 32 negative files still rejected, and here
are the exact places it deliberately differs from the grammar every editor
ships, with reasons. That is not a proxy for the question. It is the question.

And it **subsumes the practical content of the old one**: the grammar source is
in git, so "nothing was inserted that you cannot see" holds more directly than
it did through a submodule and a patch series.

### Where it is weaker, stated plainly

**Derivation was total; behaviour is sampled.** An equality over an artifact
says something about every possible input. A differential over 5,657 files says
nothing about the 5,658th. No amount of corpus makes that difference go away,
and three mitigations do not close it either:

1. **The corpus's bias is already declared and the differential inherits it.**
   json's ledger records that 65% of its corpus is `package.json`; toml's
   records 73.6% cargo-normalized `Cargo.toml` and says outright that "a clean
   sweep over this corpus is therefore weak evidence on its own". A
   differential over a monoculture is a differential over one file shape. The
   ledger's existing `blind_to` field is where this is recorded, and every
   differential result must carry it.
2. **The batteries cover what the corpus cannot.** toml-test's 712 cases exist
   precisely because the real corpus contains no BOMs and no lone CRs.
3. **For small frozen languages the differential can approach total.** JSON is
   125 lines against a frozen RFC. Two grammars that size can be compared by
   enumerating the constructs, not only by sampling files — 54 cases already
   did most of that work in json's negative battery. This is an aspiration for
   JSON and TOML specifically and is not claimed for bash.

**J4 degrades.** A patch series is reviewable against a fixed base forever; a
commit history is not. Nothing in the replacement recovers this, and it should
not be papered over: what treebank offers upstream today is a patch file that
applies. An owned grammar offers nothing back.

**R2 has an expiry the others do not.** It is a one-time measurement against a
grammar that will keep moving without us. Pinning `reference/` forever keeps
the measurement *re-checkable*, but three years on it will say "we matched
tree-sitter-json as of 2026", which is a claim about the past. R3, R4 and R5
are the ones that stay live.

---

## 5. Making it as mechanical as `verify.sh` — measured, not asserted

The differential harness is the only genuinely new machinery, so its
feasibility was measured rather than assumed, using the vendored json grammar
and the tbjson corpus.

**Setup.** 5,657 `.json` files (the ledger's corpus, `node_modules` excluded),
three grammar variants built with tree-sitter-cli 0.25.10:

- `hidden` — the vendored grammar as materialized (patch 0003 applied)
- `public` — the same grammar with `_value` renamed to `value`, a behaviourally
  identical edit
- `mutated` — the same grammar with `$.null` removed from the value choice

| variant | files failing | disagreements vs `hidden` |
|---|---|---|
| hidden | 92 | — |
| public | 92 | **0** |
| mutated | 112 | **20** |

Three things this establishes.

- **It reproduces the ledger.** 92 failing files is exactly
  `corpus.sweep_patched.failed` in `crates/treebank-json/ledger.json`, arrived
  at independently.
- **It is falsified, not trusted.** A behaviourally identical grammar gives 0
  disagreements; a one-line mutation gives 20. A differential that cannot
  report non-zero is worth nothing, which is the same argument json's ledger
  already makes about its zero `gap_files`.
- **Cost is not an obstacle.** A full-corpus pass is **2.7 s** wall clock,
  single-threaded, via `tree-sitter parse -q --paths`. Two passes plus the diff
  is under 6 s per grammar — cheaper than the `cargo build` that precedes it,
  and well inside what `verify.sh` already costs.

The 20 disagreements are raw verdict flips, before oracle adjudication; json's
ledger reports 3 *gap files* for the same mutation on its earlier 1,426-file
corpus, which is the adjudicated quantity. Both numbers are correct and they
count different things — R2's K is the raw flip count, and the adjudication is
what moves each flip into "fixed" or "ledgered divergence".

### The shape of the check

```sh
scripts/verify.sh crates/treebank-<lang>     # owned grammar
  ├── generate at pinned CLI, compare byte for byte        (R1)
  ├── treebank differential --grammar build --reference reference
  │     → fails on any flip not in ledger.differential.divergences   (R2)
  ├── treebank sweep …  → fails if gap_files exceeds the ledger      (R3)
  ├── treebank negative --dir test/negative                          (R4)
  ├── conformance suite, where one exists                            (R4)
  └── treebank ontology .                                            (R5)
```

One new subcommand (`differential`), one new checker (`ontology`), and the
rest is wiring. `sweep` already exists and is already the expensive part; R2
costs less than it does.

---

## 6. Blast radius

Two grammars owned and eighteen vendored is the state for a long time, so
**both models must coexist**, and nothing below should be a fork of the
tooling.

| thing | today | change |
|---|---|---|
| `ledger.json` schema (`crates/treebank-cli/src/ledger.rs`) | `upstream` is a **required, non-`Option` field**; `generate_cli`, `generate_dirs`, `patches[]` all assume a submodule | add `"model": "vendored" \| "owned"`, default `vendored`. For `owned`: `upstream` becomes `reference` and keeps `git_url`/`sha` with a new meaning (the grammar we replaced, frozen); `patches[]` must be empty; new `differential` and `ontology` blocks. `treebank ledger` branches on `model` — this is the single largest code change. |
| `scripts/materialize.sh` | submodule + patches + generate | for `owned`, no submodule and no patches: generate in place. Must **refuse** to touch `reference/`. |
| `scripts/verify.sh` | materialize + corpus tests + negative | for `owned`, the R1–R5 sequence in §5. |
| `.gitmodules` | 20 entries at `crates/*/upstream` | owned grammars' entries move to `crates/*/reference`. The submodule is **not removed** — R2 depends on it existing. |
| `GRAMMARS.md` | states the materialization invariant as *the* contract | becomes two contracts side by side, with the vendored one unchanged and explicitly marked as the majority case. |
| `scripts/check.sh` | `git -C build diff > patches/NNNN` is the authoring loop | for owned grammars the authoring loop is an ordinary edit-and-commit; the `build/`-as-throwaway-repo trick, and its `.gitignore` trap, both stop applying. |
| `.github/workflows/verify-grammars.yml` | matrix over `crates/*/ledger.json`, submodule checkout | unchanged in shape; needs the differential's corpus available, which is the one genuinely new CI cost. |
| `publish-grammars.yml` / `PUBLISHING.md` | version derived from upstream's version + build counter | an owned grammar has no upstream version. Needs its own versioning scheme — likely plain semver from 0.1.0 — and the crate identity patch disappears because there is no patch series and no name to avoid. |
| `scripts/grammar-docs.sh` | generates the README/PUBLISHING tables from `ledger.json` + `patches/` + the identity patch | must handle a grammar with zero patches and no upstream version without emitting an empty or misleading row. |
| `tools/consumer-test/grammars.json` | `patched.<ext>` exercises one construct per patch | for owned grammars there are no patches; the fixture becomes "one construct per ledgered divergence", which is the direct analogue. |
| wasm-pack work (PR #31) | builds from `build/` | unaffected in mechanism; owned grammars change `build/`'s provenance, not its shape. |
| `crates/treebank-cli/src/lang/*` | oracle per language | unaffected. The oracles are the part of the system ownership makes *more* important, not less. |

Two consumer-facing consequences that are not internal bookkeeping:

- **Owned grammars are drop-in for parse consumers and breaking for query
  consumers.** Trees are identical under a supertype rename (measured,
  `docs/ONTOLOGY.md` §1.1), so anyone calling `parse()` sees no change; anyone
  with `(_expression)` in a `.scm` file breaks. This needs a decision before
  the first publish, not after — open question 4 in the ontology.
- **The offer to upstream disappears.** `GRAMMARS.md` says "the patches are the
  offer". An owned grammar has nothing to offer back. That is a real cost of
  the direction and belongs in the record.

---

## 7. The condition that should stop this

R2 is the load-bearing new claim, and it has a specific failure mode: an owned
grammar whose disagreement set does not converge — where K stays large, and the
divergences are not principled choices but a long tail of "upstream does
something here and we do not know why".

**JSON is the test.** 125 lines, a frozen spec, an existing corpus, an existing
oracle, and a ledger that already enumerates exactly where the vendored grammar
sits relative to RFC 8259 (`grammar_is_broader_than_the_oracle`: of a 54-case
battery, 14 both accept, 29 both reject, 11 the grammar accepts and the oracle
rejects, 0 the reverse). If an owned JSON grammar cannot reach K = 0 with every
divergence adjudicated, on the language where the whole space is enumerable,
then the replacement invariant is not achievable on any language and the
programme should stop there rather than at bash.
