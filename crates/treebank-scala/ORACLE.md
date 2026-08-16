# The Scala oracle: one grammar, two languages

Answer to the first-move question, written before any grammar work:
**what does the oracle assert, when `.scala` covers two languages and
nothing in the file's path says which one it is?**

## What the oracle asserts

> **Claim.** This file contains no Scala *syntax* error, judged by
> scalameta's parser, **in the dialect the file's Maven coordinate
> declares**.

It deliberately does **not** claim:

- that the file compiles — nothing is typed, no symbol is resolved, no
  macro is expanded, and an unresolved import is not an error;
- that the file is valid Scala *in general*. There is no such thing. A file
  is valid Scala 2.13 or valid Scala 3, and 8.6% of this corpus changes
  answer between them;
- that scalameta and `scalac` agree everywhere. They are different
  programs. Where they differ, scalameta is wrong — see
  [Authority](#authority-reference-with-a-proxy) below.

## The dialect is an input, not a guess

`GRAMMARS.md` says the `oracle.flags` field exists because "a version alone
does not always settle the dialect". Scala is the case it was anticipating,
and it is worse than C's: C needs one flag for the whole corpus
(`-std=gnu17`), while Scala needs an answer **per file**.

The roadmap's framing was that "nothing in the path tells you which".
That is true of the path and false of the *package*. A Maven artifact built
for Scala carries its compiler's binary version in the artifact id —
`cats-core_3` is Scala 3, `spark-core_2.11` is Scala 2.11 — and the corpus
directory is named for the coordinate. So `lang/scala.rs` reads the dialect
off the coordinate and passes it to the oracle per file. `dialect_for` has
no default and never will: an unroutable path is an error, not a verdict.

### What the alternatives cost, measured

3,508 files at the time of the decision, all library code that compiles:

| routing | valid files called invalid |
|---|---|
| **declared per package** | **0 / 3508** |
| every file as `Scala213` | 61 (1.7%) |
| every file as `Scala212` | 62 (1.8%) |
| every file as `Scala211` | 226 (6.4%) |
| every file as `Scala3` | 301 (8.6%) |

Both directions are real. 301 files parse under Scala213 and not Scala3 —
procedure syntax, symbol literals, `do`/`while`, from Flink and Akka 2.x.
61 parse under Scala3 and not Scala213 — `given`, `inline`, `using`, from
tapir, upickle and os-lib.

A wrong `invalid` is the expensive direction. `validate()` runs only on
files the grammar already failed, so `invalid` files the case as *corpus
noise* — it does not just lose a verdict, it hides a grammar gap and
flatters the pass rate. Pinning every file to one dialect would have
donated between 1.7% and 8.6% of the corpus to that.

### Why trying both dialects is not a shortcut

Parse under Scala213, and if that fails try Scala3, and call the file valid
if either succeeds. It is the obvious idea and it is dishonest: it makes
**every file valid by construction**, drives `gap_files` toward zero, and
reports a flawless grammar. The union is not a measurement of anything.

The measurement above is also why it buys nothing. The declared dialect
misrouted **zero** files, so the union would add no coverage at all — it
would only cost the meaning of the word `invalid`. If a future corpus ever
does need a fallback, the rule stands: record which dialect succeeded, per
file, or do not do it.

### The corpus has to be built for this to be checkable

The dialect answer is only interesting if both dialects are actually in the
corpus, and the obvious ranking does not put them there. `ecosyste.ms`
ranks Maven artifacts by dependent repositories, which for Scala is a
decade-lagging metric: **zero `_3` coordinates in the top 4,000**,
`spark-core_2.11` at 8,772 against `cats-core_3` at 2. Reused unchanged it
would sweep a Scala-3-era grammar against 2016 Scala 2.11 and never
exercise the split this language was queued for. So the ranking decides
what is popular and the coordinate decides which dialect to sweep; both
lines of a cross-built project are fetched.

## Authority: `reference`, with a proxy

`authority` is `reference` rather than `position`. Scala has an
implementation everyone appeals to — `scalac` — so a gap number here is a
measurement, not a measurement-relative-to-us, and `position` would
overstate the uncertainty. That is the difference from YAML, where no
parser is authoritative and the choice genuinely is ours.

But scalameta is **not** `scalac`. It is the parser Scala's own tooling —
Metals, scalafmt — appeals to in `scalac`'s place, and the two can
disagree. A disagreement is a bug in scalameta, not a matter of opinion.
One is known: scalameta's `Scala213` dialect accepts `def f(using ) = 1`,
which `scalac` 2.13 rejects, because it reads `using` as a clause in every
dialect. `Scala3` rejects it. No corpus file depends on it.

The negative battery found that, not the corpus — which is the general
lesson. Agreement on clean library code is worth nothing; only files that
*should* be rejected test an oracle.

## Cost, and why it does not parallelize

Measured on 17 shared cores under load average 5–20, so these are upper
bounds:

| batch | s / 1000 |
|---|---|
| 100 files | 18.2 |
| 1,000 files | 4.39 – 4.64 |
| 3,508 files | 2.74 – 2.93 |
| marginal, warm | 1.12 – 1.40 |

JVM start alone is 0.23 s and the single-file source launcher adds ~0.6 s
of compile per invocation. The batch-size dependence is the operational
point: **this oracle must be called in batches**, which is how `sweep`
calls it.

Against the roadmap's measured peers — python 1.23, lua 1.7, go 1.94, bash
2.4 forked, php 18.3 serial, C++ 1068 — Scala is firmly in the cheap tier.
The roadmap quotes no figure for it; cost was never this language's
problem.

### The `-P16` lever does not reach here

PHP's roadmap entry generalizes its parallel oracle to "lua, fish, awk, and
a dozen more of the hundred". It does not reach a JVM-resident oracle, and
reaching for it anyway corrupts verdicts.

`scala.meta.internal.tokenizers.PlatformTokenizerCache.megaCache` is a
`ConcurrentHashMap` of `Dialect` → **non-concurrent** `mutable.Map`, so two
threads parsing under the same dialect race and throw
`ConcurrentModificationException`. Measured: at 4 / 8 / 16 threads, 1 / 8 /
15 valid files flipped to `invalid`, **a different set each run**. A
threaded oracle would quietly agree with us, file real gaps as noise, and
do it non-reproducibly.

Process-level parallelism is safe but barely pays — 1/4/8/16 JVMs over
3,508 files ran 11.0 / 8.9 / 12.9 / 18.7 s wall, best case ~1.24×, because
per-process JIT warmup is a large fraction of the job. So: one JVM, serial,
and a single worker thread that exists only for its 512 MB stack. The cache
itself does not leak; it clears per parse, and heap stayed at 6 MB across
10,524 parses.

## Failing loud

Two rules, both from defects this repo has already paid for once.

**An unreadable file is not an invalid file.** A mistyped corpus root would
otherwise turn every grammar failure into noise, drive `gap_files` to zero,
and report a flawless grammar. The oracle exits non-zero on I/O rather than
emitting a verdict, and uses `Runtime.halt` so no shutdown hook can flush a
partial answer.

**An unroutable file is not an invalid file either** — the same argument,
one step earlier. A path with no Maven coordinate gets an error, never a
default dialect. This is the rule the oracle smoke test collided with:
the smoke harness passes repo-relative paths, so
`tools/consumer-test/fixtures/patched.scala` has no dialect and is
correctly refused. The fixtures declare theirs the way a corpus file does,
by living under a directory named for a coordinate, rather than by the
oracle acquiring a default to make a check go green.

**A thrown exception is not a verdict.** scalameta reports a syntax error by
*returning* `Parsed.Error`. Anything thrown out of it is the parser
breaking, and guessing `invalid` there would file a grammar gap as noise.
Nothing in the corpus reaches that path serially.

## The grammar is the union; the oracle is not

Worth stating plainly, because it makes two things look inconsistent when
they are not. `tree-sitter-scala` is **one grammar for two languages** — it
parses Scala 2 and Scala 3 — while the oracle judges each file under one
declared dialect. So:

- `tools/consumer-test/fixtures/patched.scala` is valid under **no** single
  dialect, on purpose. It pins the union the grammar claims.
- `test/negative/` files must be rejected by **both** `Scala213` and
  `Scala3`. A file only one dialect rejects is not a strictness test at
  all — the grammar is supposed to accept it. `def f(using ) = 1` was cut
  from the battery for exactly that reason.

That distinction is why the negative corpus is verified against both
dialects every time it grows.
