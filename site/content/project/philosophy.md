---
title: Philosophy
description: Measure the directions that are inconvenient, and commit the result.
order: 10
---

**A pass rate is one direction.** Does the grammar accept valid code? A number
that only moves one way is a number you will eventually optimise toward a
grammar that accepts everything. Treebank measures accepts-valid,
rejects-invalid, and builds-the-right-tree as three separate things, because
they fail independently.

**The oracle is the language's own toolchain.** Any other reference parser is
a second opinion from a worse parser, and when it disagrees you have learned
nothing.

**Evidence is committed, not reported.** A number in a README cannot be
reproduced. A ledger bound to a grammar revision, over a corpus pinned by a
lock, can be — and goes stale visibly when the grammar moves.

**Generated things are generated.** Sweep blocks are mechanically regenerated
and never hand-edited. The CI matrix is derived from the checkout. The grammar
reference is rendered from the parse table. Every hand-maintained list is a
place where the repository can start lying about itself, quietly.

**Lists are discovered.** A directory under `crates/` with a `grammar.js` in it
is a grammar — for the gates, for the wasm packs, and for the reference pages
on this site. A hand-kept list means a new grammar's checks are green because
nothing ran them, which is the one failure mode a gate cannot survive.

**Refusals are recorded, not hidden.** Where a grammar cannot do something —
an unexpanded macro, a toolchain with no supported tree dump — that is written
down as a known deviation rather than worked around until the number looks
better.
