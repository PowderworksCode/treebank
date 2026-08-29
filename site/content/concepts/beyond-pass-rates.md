---
title: Beyond the pass rate
description: The checks that catch what a corpus of valid code structurally cannot.
order: 40
---

A sweep measures one direction: does the grammar accept valid code? Optimising
that number alone drifts toward a grammar that accepts everything. These are
the checks that measure the other directions.

**Negative corpus** (`treebank negative`). Files that must *fail* to parse.
Catches accepts-invalid-code — the direction no corpus of real, valid source
can ever reveal.

**Shape** (`treebank shape`). Every node boundary the reference parser reports
must exist in ours. This is the only check that can see a file parse cleanly
and *wrongly*. A sweep and a negative corpus both judge accept/reject; this
judges the tree. Every fixture under `test/shape/` is a mis-parse that was
found on the corpus and fixed, and the ceiling is zero.

**Errors** (`treebank errors`). When the grammar rejects a file, does it
reject in the *right place*? Compares our first `ERROR` node against where the
reference parser reported its first error.

**Fuzz** (`treebank fuzz`). Derives programs *from* the grammar and asks the
oracle whether they are in the language. Everything above is bounded by what
the corpus contains; this is not — which matters most for accepts-invalid,
since real source is valid. Failures arrive shrunk.

**Reformat** (`treebank reformat`) and **roundtrip** (`treebank roundtrip`).
Run the language's own formatter or printer over the corpus and assert the
tree is unchanged. A formatter preserves the program and rewrites its layout,
so a tree that moves is our bug: a rule reading layout it should not, or a
token that only lexes when it abuts its neighbour.

**Incremental** (`treebank incremental`). Parse, edit, reparse incrementally,
and compare against a fresh parse. tree-sitter's contract is that the two are
indistinguishable. Every other check parses from scratch, so a grammar can
pass all of them and still hand a broken tree to an editor — usually an
external scanner whose serialize and deserialize do not round-trip.

**Recovery** (`treebank recovery`). Delete one token and measure how much of
the file lands inside an `ERROR`. Editors spend most of their time on broken
source, and how much structure survives is a property nothing else here looks
at.

**Kinds** (`treebank kinds`). Counts node kinds over the corpus and reports
which ones real code never produces. Those are the blind spot: no oracle has
been asked about them, because every corpus-driven check starts from code that
does not contain them.

**Rosetta** (`treebank rosetta`). The same program in every owned language
must yield the same role counts. It is the one check that catches a role
threaded in one grammar and forgotten in another — supertype matching is
derivation-based, so a missed thread is silent everywhere else.
