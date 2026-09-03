# The ledger format

A recommendation, not a change. Nothing outside `notes/` has been touched. A
worked conversion of ruby's ledger is in `notes/ledger-format-example/ruby/`.

> "As well, is there a better format for the ledgers?"

Yes — but the container is the second problem, not the first. The first is
that a `ledger.toml` has **no schema**, and that has already cost the project
two silent defects that are live on `main` and on the published site today.
This proposes fixing the schema, evicting the generated numbers, and only then
inverting the prose nesting.

## 1. What a ledger is today

Eleven files, 201 KB. Measured:

| | share |
|---|---|
| Markdown prose inside TOML strings | **89%** (179 KB) |
| everything else — keys, numbers, hashes | 11% |

Of the prose, 70% is free-standing essays (`versions`, `vocabulary_note`,
`ranking_note`, `blind_to`, `*_check`, `sweep_history`) and 30% is a note
attached to one of 93 declared records (`known_gaps`, `known_widenings`,
`deviations`, `oracles`).

There are 33 distinct top-level keys across the eleven files, and **15 of them
appear in exactly one ledger** — `argument_ordering_analysis`,
`dialect_decision`, `inline_suites_and_comment_lines`, `oracle_blind_spot`,
`toolchain_note`, `narrowing`, `patterns_note`, `conformance`, `version_policy`,
`keyword_note` and six more. The top level is not
a schema. It is a namespace of essay headings a human invents as they write.

Which would be fine, except that `treebank status` reads structure out of it.

## 2. What that has already cost

### 2.1 Ruby's declarations vanish, and the site publishes a clean sheet

`crates/treebank-ruby/ledger.toml` line 65 opens `[deviations]`, a table.
Every other ledger writes `[[deviations]]`, an array of tables. Nothing rejects
the difference. `declared_items` (`status.rs:932`) calls `as_array` on a table,
gets `None`, and returns an empty list.

Reproduced on this branch, unmodified:

```
$ treebank status --format markdown
| ruby | ruby | 6,480/6,487 99.89% | 7 | 26/10/2 | 0/0/0 | …
                                                  ^^^^^ known gaps / widenings / deviations
```

```
$ jq '.grammars.ruby | {known_gaps, known_widenings, deviations}' site/public/status.json
{ "known_gaps": [], "known_widenings": [], "deviations": [] }
```

The ruby ledger declares a `heredoc_body`-in-`extras` deviation and a
two-paragraph list of about a dozen known gaps and widenings. On a site whose
whole pitch is *"the gaps, widenings and deviations are the honest half of the
ledger… the reason to publish an inventory rather than a pass rate"*
(`site/tools/build-status.mjs`), **ruby publishes a perfect record it has not
earned**, and has done since the file was written.

This is worse than the failure the brief cites. PR #242's `negative_files 12 →
13` was `site/public/status.json`, a wholly generated file guarded by
regenerate-and-diff in `ci.yml`. That guard **worked**: it caught a fixture
added without regenerating, which is exactly its job. Ruby's is the failure
mode with no guard at all.

### 2.2 `treebank status` reports `ranking_note` as a measurement

`read_evidence` (`status.rs:880`) walks every key of `[corpus]` and treats
anything that is not `sweep` or `gaps` as a measurement performed on the
grammar. So the status table's "Measured" column says, for all eleven grammars:

```
| bash | … | files, packages, ranking_note, source, sweep | …
| c    | … | blind_to, files, packages, ranking_note, source, stand_in_history, … |
```

`ranking_note`, `source`, `files`, `packages`, `blind_to`,
`corpus_composition_note`, `widening_note`, `kinds_coverage` are not
measurements. The same function infers the top-level check list from key
presence — `ledger.get("shape_check").is_some()` — so which checks a grammar
has run is decided by whether someone spelled a heading `shape_check` or
`shape_note`.

### 2.3 The generated block is spliced as text, and the file warns you

`write_ledger_block` (`sweep.rs:335`) does not serialize; it finds the string
`[corpus.sweep]` and replaces everything up to the next `\n[`. The C ledger
carries a hand-written warning about it, in the data file, for the next human:

```toml
[corpus.gaps]
# Everything the sweep's ledger writer does not carry. It owns
# `[corpus.sweep]` above and replaces that block wholesale on every run —
# prose included, and up to the next section header — so anything written
# between the two sections is eaten. Here, past the header, it survives.
```

A file that has to document where it is safe to write has the seam in the
wrong place.

### 2.4 The writer drops four numbers the sweep computes, and one ledger no longer adds up

`sweep::Report` carries `config_files`, `version_files`, `hidden_gap_files` and
`clusters`. `write_ledger_block` writes none of them. Where they appear at all
— `[corpus.gaps]` in c and cpp — a human transcribed them from stdout. That is
precisely the class of drift that put java's ledger at 811 while the truth was
167 (issue #145), which is why the sweep writer exists in the first place.

The consequence is checkable. Two identities should hold on every sweep block:

```
files  == passed + failed
failed == gap_files + config_files + version_files + noise_files
```

Run against all eleven committed ledgers, twelve sweep blocks:

```
python  sweep   files=p+f ok   failed=g+c+n FAIL
        failed=1097  gap=2  config=0  noise=1083  =>  1085
```

Twelve python files are unaccounted for in the machine-readable data. They are
not drift: they are the 12 version-policy rejections, which python's ledger
describes correctly in a prose paragraph and which the writer has no field for.
Nothing checks either identity today.

### 2.5 179 KB of the repository's best prose is invisible to its prose linter

`.vale.ini` scopes its styles to `[*.md]`, and CI runs vale on every PR. Every word of the
ledgers is in `.toml`, so the most argued-over writing in the repository is the
only writing nobody lints, spell-checks, previews, or gets an editor's
Markdown mode for. 273 of the 287 prose blocks use `'''` rather than `"""`
specifically to escape TOML's escapes — the convention exists because the
container fights the content.

## 3. Proposal

**Split by who writes the file, not by what syntax it uses.** Two files per
grammar crate, each with exactly one author.

### `evidence.json` — machine-owned

Everything `treebank sweep` measures, serialized from `Report` rather than
spliced into text. No human opens it; a first key says so.

```json
{
  "GENERATED": "Written by `treebank sweep`. Do not edit: the next sweep overwrites this file wholesale. The prose that explains these numbers is in ledger.md.",
  "schema": 1,
  "language": "ruby",
  "sweeps": [{ "corpus": "ruby", "files": 6487, "passed": 6480, "failed": 7,
               "gap_files": 7, "config_files": 0, "version_files": 0, "noise_files": 0,
               "pass_rate": "99.89%",
               "corpus_lock_sha256": "…", "grammar_sha256": "…", "grammar_revision": "…" }]
}
```

What this buys, in order of importance:

- **"Never hand-edit" becomes structurally true** rather than a rule people
  follow. There is nothing in the file but generated numbers, so there is
  nothing to preserve around them and no splice.
- **The writer emits its whole report.** `config_files`, `version_files` and
  `clusters` stop being stdout a human retypes. §2.4's python discrepancy
  closes by construction, and c's and cpp's `[corpus.gaps]` numbers stop being
  transcription.
- **Both arithmetic identities become enforceable** in `treebank status --check`.
- The existing freshness mechanism is kept unchanged and is the better half of
  the current design: the block already carries `corpus_lock_sha256`,
  `grammar_sha256` and `grammar_revision`, so staleness is *computed* from the
  inputs rather than detected by regenerating. That is stronger than
  `status.json`'s regenerate-and-diff and should not be traded for it.
- JSON rather than TOML because the writer serializes a struct, and because a
  stray hand edit to a machine file should look wrong.

### `ledger.md` — human-owned, TOML frontmatter and a Markdown body

The nesting inverts: prose is the document, structure is a small header.

```
+++
language = 'ruby'
vocabulary = '0.1.0'
generate_cli = '0.26.12'
checks = ['shape']

[corpus]
source = 'locked top-120 RubyGems by downloads (fetched 2026-08-20), …'
files = 6487
packages = 120

[[oracles]]
id = 'cruby'
family = 'ruby'
tool = 'RubyVM::AbstractSyntaxTree.parse_file, via tools/rb-oracle/check.rb'
version = 'CRuby 3.3.6'

[[known_gaps]]
id = 'do-block-subscript'
construct = 'Subscripting a `do`-block call directly'
+++

# ruby — grammar ledger

## Versions
…

## Known gaps

### Subscripting a `do`-block call directly {#do-block-subscript}

`list.map do … end[0]` is a gap, because the chain family carries members and
calls but not subscripts — its cross-recursion multiplied table generation past
use.
```

Three rules, and they are the whole format:

1. **Frontmatter carries only what a machine reads.** Typed, `deny_unknown_fields`,
   deserialized into a real struct exactly as `TermsManifest` already is. Ruby's
   `[deviations]`-versus-`[[deviations]]` becomes a build failure instead of an
   empty list. `checks` is a named list, so §2.2's guessing-from-key-presence
   goes away.
2. **Every paragraph is a body section.** Free essays become `##` headings; the
   note on a declared record becomes a `###` heading under its category. Nothing
   the machine reads is prose, and no prose is inside a string.
3. **A declared record and its prose are joined by an explicit `id`.** The
   frontmatter entry carries `id`, the body heading carries `{#id}`. An `id`
   with no section, or a section with no `id`, fails `treebank status --check`.

TOML frontmatter (`+++`) rather than YAML, for three reasons: arrays of tables
are the shape the declared records actually have; a TOML multi-line string is
not indentation-sensitive the way a YAML block scalar is; and `toml` is already
a dependency while no YAML parser is.

Rule 3 is the one that needs defending, since it splits a record from its
paragraph. It earns that:

- A person reading ruby's gaps reads Markdown, not TOML — which is the point of
  the whole exercise.
- The check that ids and anchors correspond is about twenty lines and fails
  loudly. That is strictly better than today, where a mis-declared record fails
  silently and has for months.
- The id is not overhead. It is the stable anchor the site cannot currently
  produce: a declared deviation has no identity today, so nothing can link to
  `treebank.dev/grammars/ruby#do-block-subscript`.
- All 179 KB of prose comes under vale and an editor's Markdown mode.

The published-artifact story improves too. Grammar crates ship `LEDGER` as
`include_str!`; a consumer wanting a pass rate today must TOML-parse a 32 KB
prose file. Two consts — `LEDGER` (Markdown) and `EVIDENCE` (JSON) — let a
consumer take the numbers without the essays. The same split applies to the
wasm packs.

## 4. The worked example

`notes/ledger-format-example/ruby/` holds `crates/treebank-ruby/ledger.toml`
converted, unabridged. All prose is verbatim from the current file except where
noted below. Checked mechanically:

```
frontmatter parses OK
declared ids   : 11
body anchors   : 11
ids w/o anchor : none
anchors w/o id : none

treebank status would now report for ruby:
  known_gaps = 5   known_widenings = 3   deviations = 2   checks = ['shape']
```

Against `0 / 0 / 0` today.

**One thing the conversion forced, which needs your call rather than mine.**
Ruby's `known_gaps` paragraph mixes two things the repository separates
everywhere else: real gaps (constructs the grammar misses) and deliberate
widenings (constructs it accepts that CRuby rejects). The prose says so
explicitly — *"a widening CRuby rejects, taken so that…"*, *"accepts orderings
CRuby rejects"*, *"which CRuby rejects"* — but the format gave it one heading,
so all of it sat under `known_gaps`. Splitting them into 5 gaps and 3 widenings
is my reading of that prose, and the sentence heads under `### ` are mine;
every paragraph body is ruby's. Confirm or correct the split before this lands.

Two further notes on fidelity: ruby's ledger records no per-gap file counts, so
`files` is omitted on each `[[known_gaps]]` entry — the field stays optional,
and a warning when `Σ known_gaps[].files` exceeds the sweep's `gap_files` is
worth adding separately. And `evidence.json` carries only numbers ruby's ledger
already records; `clusters` and the oracle verdict breakdown are fields the
sweep computes and will fill on its next run, not values invented here.

## 5. Options weighed and not taken

**Keep TOML, change nothing but the schema.** This is genuinely most of the
value — it fixes §2.1 and §2.2 outright, needs no file moves, and could land
this week. Not taken as the *whole* answer only because it leaves §2.3 and §2.4
untouched: generated numbers stay in a hand-edited file, and the writer keeps
splicing text. It is, however, the right **first** PR, and if the format change
is rejected the schema should land anyway.

**Markdown with frontmatter, and no separate evidence file.** Rejected. It
inverts the nesting, which is the smaller win, and leaves generated numbers in
the file a human edits, which is the larger problem. Frontmatter would make it
slightly worse — a machine-written block inside a human-written header.

**One file, JSON with Markdown strings.** Rejected outright: every problem TOML
has, plus escaping, plus no comments.

**Three files (`evidence.json`, `ledger.toml`, `ledger.md`).** Considered:
generated numbers, structured declarations, free prose, each in its natural
format. Rejected for one file too many — two files named "ledger" in one
directory is a question every reader has to answer, and frontmatter already
carries the structured half at no cost.

**Rendering the prose from the site's Markdown pipeline.** Out of scope, but
worth noting that it becomes possible: today `build-status.mjs` deliberately
prunes ledger prose because the pages cannot render it, and the note says the
reasoning *"stays in the repository, where a reader who wants the reasoning can
find it"*. With the prose in Markdown, publishing it is a build step rather than
a project.

## 6. How to land it

Four PRs, one concern each. The first two are worth doing whatever happens to
the format.

1. **Type the ledger.** A `Ledger` struct with `deny_unknown_fields`, a
   `notes: BTreeMap<String, String>` catch-all so essays stay free-form, and an
   explicit `checks` list. Fixes §2.1 and §2.2. Ruby's ledger is corrected in
   the same PR because it will no longer build.
2. **Evict the generated numbers.** `evidence.json` written by
   `treebank sweep` from `Report`; `[corpus.sweep]` and `[corpus.gaps]` deleted
   from all eleven ledgers; `write_ledger_block`'s text splicing deleted with
   them; both arithmetic identities checked in `treebank status --check`. Fixes
   §2.3 and §2.4.
3. **Convert the prose.** `ledger.toml` → `ledger.md`, mechanically, one
   grammar per commit so each diff is readable. Fixes §2.5.
4. **Publish it.** `LEDGER` and `EVIDENCE` consts, pack exports, and the site
   rendering the prose it currently prunes.

Gates on each: `cargo build --workspace`, `cargo test --workspace`,
`cargo fmt --check`, `cargo clippy --all-targets -- -D warnings`,
`treebank verify`, and `treebank status --check`.

## 7. What is right today and should not be traded away

The ledger's freshness design is better than the one `site/public/status.json`
uses, and the split above keeps it. `[corpus.sweep]` records the SHA-256 of the
corpus lock and of the grammar sources it was measured against, so
`bind_evidence` can *compute* whether the evidence still describes the current
grammar and report `current` / `stale` / `unbound`. `status.json` has no such
binding and is guarded by regenerating and diffing in CI — which is what failed
PR #242, correctly.

Committing evidence next to the grammar rather than reporting it is also right,
and none of this changes it. So is TOML's claim in the file header — *"prose
rather than data, which is why it is TOML"* — as far as it goes. The header
identified the problem correctly and then reached for the wrong end of it: if
the file is prose rather than data, the prose should be the file.
