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

## Written from scratch

The grammars are not forks. There are no upstream grammar repositories, no
vendored trees and no patch series anywhere in the project — each grammar was
written for Treebank, which is what makes the rest of it possible.

The shared vocabulary is the clearest case. `_declaration` and `_loop` are
threaded through the productions and enforced when the parser is generated,
so a query for them is answered by the parse table rather than by a naming
convention. That is only available to someone who writes the grammar; it
cannot be added to one from outside.

It is also why a pack's provenance is a hash of its source rather than an
upstream commit and a list of patches. There is no upstream to point at.

## Who maintains it

Treebank is part of [Powderworks](https://powderworks.dev), and is MIT
licensed. Most of its code is written by agents working against the gates
described in [How it works](/concepts/) — a grammar change is only accepted
with fresh evidence behind it, a rule that suits a machine contributor and a
human one equally.

Contributions are welcome on
[GitHub](https://github.com/PowderworksCode/treebank). One concern per pull
request, and a grammar change carries a fresh sweep: change what the grammar
accepts and the committed evidence is stale until it is re-run.

If you have found code that a grammar parses wrongly, the
[playground](/playground/) produces a link to the exact parser you were using.
That link is the most useful thing to put in an issue.

This site sets two typefaces under the SIL Open Font License 1.1:
[Fraunces](https://fonts.google.com/specimen/Fraunces) and [IM Fell English
SC](https://fonts.google.com/specimen/IM+Fell+English+SC). Their licences ship
beside them.
