# The YAML oracle: a position, not a fact

Every other grammar in this repo can point at a reference parser and say "that
is what valid means". YAML cannot. This document is the evidence behind
`ledger.json`'s `oracle` block, and the reason that block has an `authority`
field at all.

The short version: **libyaml, go-yaml, js-yaml and eemeli's `yaml` disagree
with each other on 67 of the official conformance suite's 402 cases**, and
they disagree with the suite too — scoring 83.3%, 81.8%, 100% and 99.3%. There
is no appeal. So the ledger declares a choice, records the six alternatives it
was measured against, and states what the verdicts are relative to.

## Three populations, each blind to something

| population | n | what it is | blind to |
|---|---|---|---|
| **corpus** | 3,217 files, 5.87 MB | real `.yml`/`.yaml` from ansible, helm, prometheus, OpenAPI-Spec | the hard syntax tail (libyaml vs go-yaml: 0 here, 14 in the suite) and every control character — 0 files with a NUL, ill-formed UTF-8, or a lone CR |
| **suite** | 402 cases, 94 expected-error | `yaml/yaml-test-suite` `data-2022-01-17` | **encoding entirely** — zero cases begin with a BOM, none carries a NUL |
| **battery** | 24 cases, 21 spec-decidable | hand-built from YAML 1.2.2 §5.1 `c-printable` and §5.2 | whatever its author did not think of; the only population not authored independently of this grammar |

`disagreement` in the ledger holds a list of populations rather than one count
because of the last column. The parallel `tbtoml` session hit the mirror image
— toml-test covers BOMs thoroughly while 0 of its 1,427 real `.toml` files
carry one — so **which population is blind is not predictable per language**,
and the only way to bound the error is to run all of them.

## The candidates

| candidate | suite | accepts-invalid | rejects-valid | battery wrong /21 | s/1000 |
|---|---|---|---|---|---|
| libyaml 0.2.5 `parse` (PyYAML CParser) | 83.3% | 16 | **51** | **0** | 0.14 |
| libyaml 0.2.5 `load` | 76.4% | 15 | 80 | 0 | 0.31 |
| go-yaml v3.0.1 → `yaml.Node` | 81.8% | 15 | 58 | 1 | 0.20 |
| eemeli `yaml` 2.9.0 CST parser | 76.6% | **94** | 0 | — | — |
| eemeli `yaml` 2.9.0 `parseAllDocuments` | 99.3% | 2 | 1 | **8** | 0.77 |
| js-yaml 5.2.3 `loadAll` | 92.0% | 0 | 32 | — | 0.27 |
| js-yaml 5.2.3 `parseEvents`, naive driver | **100%** | 0 | 0 | 4 | 0.13 |
| **js-yaml 5.2.3 `parseEvents` + §5.2 decode layer** | **100%** | **0** | **0** | **1** | **0.13** |

`rejects-valid` is the column that decides it. `validate()` runs only on files
the grammar already failed, so a valid file called invalid is booked as *noise*
and hides a grammar gap. libyaml — which ROADMAP names for this language — does
that to 51 of the suite's 308 valid cases, including tabs in literals,
zero-indented block scalars, empty keys, anchors containing a colon, and eight
of the specification's own examples.

## Four findings that changed the choice

**1. A 100% conformance score is not a safety property.** js-yaml scores 402/402
and rejects real YAML beginning with a UTF-8 BOM. Two-line repro:

```
"﻿a: 1\nb: 2\n"   →  js-yaml: "end of the stream or a document
                              separator is expected (2:1)"
                          libyaml / go-yaml / eemeli: valid
```

YAML 1.2.2 §5.2 permits it. Zero of the 402 cases begin with a BOM, so the
suite cannot see it; it cost 12 of the 15 raw corpus disagreements (helm's
`frobnitz_with_bom` fixtures), all gap-hiding.

**2. The driver owns part of the specification, and that is a liability.**
js-yaml takes a JS string, so byte-to-character decoding happens outside the
parser. `readFileSync(path,'utf8')` substitutes U+FFFD for ill-formed UTF-8,
mojibakes UTF-16, and leaves a leading BOM in the string as content — 4 of 21
battery cases wrong. Implementing §5.2 explicitly (BOM-based UTF-8/16/32
detection, fatal decode, strip exactly one leading BOM) takes it to 1 while
holding 402/402. libyaml scores 0 without any such layer because it does this
in C. **Choosing js-yaml means owning that function**, and the ledger says so.

One warning for whoever maintains it: the leading-BOM strip is correct *here*
and is not portable advice. `tbtoml` measured that the same strip would break
`toml`, which distinguishes leading (valid), doubled (invalid) and mid-stream
(invalid) BOMs. YAML parsers accept a **doubled** BOM — verified across all
three — because U+FEFF is itself inside `c-printable`, which is why stripping
one is verdict-preserving.

**3. Control characters disqualified the runner-up on conformance.** eemeli's
`yaml` is 99.3% conformant and performs no control-character validation at all:
NUL, form feed, DEL, a raw ESC and ill-formed UTF-8 are all accepted, 8 of 21
battery cases. Against a grammar whose lexer treats codepoint 0 as
end-of-input, that is the gap-**manufacturing** direction on files no parser
can handle. Credit to `tbtoml` for the NUL question; nothing else measured here
would have caught it.

**4. The suite cannot be checked out of sample.** Exactly one case has been
added to yaml-test-suite since the `data-2022-01-17` release (`ZYU8`), and it
carries `skip: true`. So 402/402 is partly a home-field score and there is no
newer population to test against. The battery *is* the out-of-sample
population — and it is where this oracle is weakest and libyaml strongest. That
trade is the position.

## The stage matters more than the parser

Measured on the same 3,217 real files:

| pair | corpus | suite |
|---|---|---|
| libyaml `parse` vs go-yaml (different parsers, same stage) | **0** | 14 |
| libyaml `parse` vs libyaml `load` (same parser, different stage) | **73 (2.27%)** | 30 |

The 73 are unresolvable tags (ansible's `!vault`) and duplicate keys. Neither
is a syntax property and a tree-sitter grammar cannot see either, so a
load-stage oracle would reject files for reasons the grammar is not
responsible for. Same axis as ROADMAP §9's python finding, where `ast.parse`
vs `compile(…, 'exec')` moved 11 of 30 supposed gaps.

`%YAML 1.2` is the tidiest single illustration that the parsers, not just the
stages, disagree: a three-line document declaring it is **valid** to js-yaml
and libyaml and **invalid** to go-yaml v3 ("found incompatible YAML
document"). It is deliberately absent from
`tools/consumer-test/fixtures/patched.yaml`, because a positive control should
not assert one side of a live dialect disagreement.

## Cost, and a correction to how this repo measures it

**0.061 s / 1000 files at 45.7 MB/s**, one long-lived process, over the real
ranked corpus (26,634 files, 74.0 MB). Reject path 0.18 s/1000, so there is no
penalty of javascript's 10.6× kind.

An earlier version of this document said 0.13 s/1000 at 17 MB/s. That figure
was measured on a 5.87 MB corpus and it was **wrong by 2.7× in the oracle's
favour to state as throughput**, because a JIT runtime has *two* fixed costs
and only one of them is what `ROADMAP` §9 tells you to subtract:

| slice of the real corpus | best of 3 | MB/s |
|---|---|---|
| 2.0 MB | 0.20 s | 11.6 |
| 6.0 MB | 0.25 s | 27.3 |
| 20.0 MB | 0.49 s | 43.6 |
| 40.0 MB | 0.93 s | 44.6 |
| 74.0 MB | 1.65 s | 45.7 |

Process startup is 0.03 s and constant. **JIT warm-up is not constant** — it is
amortised over the batch, and V8 needs roughly 20 MB of input before it reaches
steady state. §9's method ("run each oracle with empty stdin to get its startup
cost, then subtract") removes the first and leaves the second inside the
per-file number. Every figure in §2 was taken at 1000 files, so the js, ts and
java oracles — all JIT'd — are all likely to be understating their throughput
the same way.

ROADMAP §1 puts Tier A at 0.2–2 s per 1000 files and this lands well under the
floor. Two independent reasons, and they pull in the same direction, so the
number is not comparable to the ones beside it in that table. First, YAML files
are small: mean 2,780 B and median 458 B on the real corpus, so a per-*file*
figure buys a lot less work than it does for java. The same oracle over the 300
largest measurement-corpus files (mean 15.8 KB, java-like) costs **0.93
s/1000**, dead centre of the band. Second, the warm-up above. `s / 1000 files`
is a bytes/second measurement wearing a per-file label, and configuration
languages are where that breaks. `tbtoml` measured the
identical effect independently (48 MB/s, 2.55 KB mean, 0.096 s/1000), so it is
a property of the class rather than of YAML.

## Failing loud, in both directions

| case | this oracle |
|---|---|
| nonexistent path | stderr + **exit 1** |
| a directory | stderr + **exit 1** (Node throws EISDIR) |
| binary file | `invalid` — the read succeeded and the bytes are not YAML |
| ill-formed encoding | `invalid` — §5.2 makes encoding part of stream well-formedness, so this is a verdict, not an I/O error |

The read and the decode are separate `try` blocks in `check.mjs` for exactly
that last distinction. Two failure modes from elsewhere are worth naming
because `reject_statuses` cannot catch either — the tool is answering, just
about the wrong thing:

- **go-yaml's driver books a directory as `invalid`**, because Go's `os.Open`
  succeeds on one and the read error arrives later as a decode error.
- **`taplo lint` exits 0 on a path matching nothing** (`tbtoml`'s finding),
  which is the same defect in the more expensive direction: every grammar
  failure becomes a phantom gap and dispatches a fix agent at a file that does
  not exist.

So `unreadable_note` in the ledger records **which direction** a tool fails in,
not merely that it was handled.

## What the position costs, on 26,634 real files

The suite says the four parsers disagree on 67 of 402. The real corpus says
they disagree on almost nothing — and pins down exactly what "almost" is.

Over all 26,634 ranked-corpus files the oracle rejects **7**. The grammar
rejects 1 of them (booked as noise, correctly) and **accepts the other 6**. But
libyaml and go-yaml call all 6 of those *valid*:

| | js-yaml | libyaml | go-yaml | grammar |
|---|---|---|---|---|
| 6 files | invalid | **valid** | **valid** | accepts |
| `ooapiv6.yaml` | invalid | invalid | invalid | rejects |

All six are one construct: a multi-line flow collection as a block mapping
value whose continuation and closing bracket sit at the *same* indentation as
the key —

```yaml
        daj_util_objects: [
          { "objectType": "TS" }
        ]
```

js-yaml calls that "deficient indentation", and YAML 1.2.2 requires flow
content nested in a block context to be indented past its parent, so js-yaml
appears to be right and the other two lenient. It is a common shape because it
is what pasted JSON looks like.

So: **`grammar accepts-invalid` is 6 under this oracle and would be 0 under
either alternative**, while `gap_files` is 4 under all of them, because every
gap file is one every parser calls valid. The headline number does not depend
on the position; the number a future patch would be judged against does. That
is what `verdicts_are_relative_to` is for, stated as a measurement rather than
as a caveat.

The other half of the same result is worth as much: **the three parsers agree
on 26,628 of 26,634 real files, 99.98%.** The disagreement this whole document
is about is a hard-tail phenomenon. Both numbers are true, and neither is
quotable without the other.

## What the negative corpus is drawn from

`test/negative/` holds 24 files: 20 from the suite's expected-error cases and 4
control-character/encoding cases from the battery, which the suite has none of.
Every one is rejected by the oracle *and* by the grammar today.

Two exclusions worth stating. NUL cases are not there: `a: 1\nb: \0\n` parses
**clean** under the grammar because tree-sitter truncates at codepoint 0, so it
is accepted-invalid and unfixable in `grammar.js` — `Lang::admit` drops such
files from the corpus instead. And the 7 suite cases the grammar currently
accepts are not there either; they are counted in the ledger's
`sweep.grammar_accepts_invalid` instead, because a test that fails on arrival
is a to-do disguised as a test.
