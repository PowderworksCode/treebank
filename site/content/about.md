---
title: About
description: What Treebank is, where it came from, and who maintains it.
order: 3
---

Treebank is a collection of Tree-sitter grammars maintained as one project,
with shared tests, a shared vocabulary, and published evidence for each.

Tree-sitter grammars are usually maintained one repository at a time, each
with its own tests and its own idea of what a node should be called. That
works, and it produces two things Treebank exists to change: the quality of a
grammar is hard to see from outside, and a query written for one language has
to be rewritten for the next.

## What is different here

**Every grammar is measured the same way.** The same corpus construction, the
same reference parsers, the same set of checks, and the same numbers published
for all nine. A grammar's page shows its pass rate, what it gets wrong, and
what it declares about itself.

**The grammars share a vocabulary.** A query for `(_declaration)` finds
declarations in Rust and in Java, because the same roles are threaded through
every grammar and checked by the same gate.

**Every grammar ships the same way.** One WebAssembly file, no dependencies,
usable from any language with a WASI runtime. Not a native module per platform
and not a package per language.

## Where it came from

The grammars began from the upstream Tree-sitter grammars for each language,
which are the work of many people over many years. Treebank owns its copies
now — they have been changed enough that pointing at an upstream commit and a
patch series would describe them less accurately than a hash of the source
does. Each grammar keeps its own licence and records its own provenance.

## Who maintains it

Treebank is part of [Powderworks](https://powderworks.dev), and is MIT
licensed. Most of its code is written by agents working against the gates
described in [How it works](/concepts/) — a grammar change is only accepted
with fresh evidence behind it, which is a rule that suits a machine
contributor and a human one equally.

Bug reports and pull requests are welcome on
[GitHub](https://github.com/PowderworksCode/treebank). If you have found code
that a grammar parses wrongly, the [playground](/playground/) will produce a
link to the exact parser you were using, which is the most useful thing to put
in an issue.
