---
title: What else is measured
description: The checks a corpus of valid code cannot provide.
order: 40
---

A pass rate measures one direction: does the grammar accept valid code?
Optimising only that produces a grammar that accepts everything. These measure
the other directions.

**Negative corpus** (`treebank negative`) — files that must *fail* to parse.
Catches accepts-invalid-code, which no corpus of real source can reveal
because real source is valid.

**Shape** (`treebank shape`) — every node boundary the reference parser
reports must exist in ours. The only check that can see a file parse cleanly
and *wrongly*. Every fixture under `test/shape/` is a mis-parse found on the
corpus and fixed.

**Errors** (`treebank errors`) — when the grammar rejects a file, does it
reject in the right place? Compares our first `ERROR` node against the
reference parser's first error.

**Fuzz** (`treebank fuzz`) — derives programs from the grammar and asks the
oracle whether they are in the language. Everything else is bounded by what
the corpus contains; this is not. Failures arrive shrunk.

**Reformat and roundtrip** — run the language's own formatter or printer over
the corpus and check the tree does not move. A formatter preserves the program
and rewrites its layout, so a tree that changes is a bug: a rule reading
layout it should not, or a token that only lexes when it touches its
neighbour.

**Incremental** (`treebank incremental`) — parse, edit, reparse incrementally,
compare against a fresh parse. Every other check parses from scratch, so a
grammar can pass all of them and still hand a broken tree to an editor.

**Recovery** (`treebank recovery`) — delete one token and measure how much of
the file lands inside an `ERROR`. Editors spend most of their time on broken
source.

**Kinds** (`treebank kinds`) — counts node kinds over the corpus and reports
which ones real code never produces. Those are the blind spot: no oracle has
been asked about them.

**Rosetta** (`treebank rosetta`) — the same program in every language must
yield the same role counts. Catches a role threaded in one grammar and
forgotten in another.
