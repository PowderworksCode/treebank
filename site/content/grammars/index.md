---
title: Grammar reference
description: Every production in every grammar, as EBNF and as a railroad diagram.
order: 50
---

Every production in all nine grammars, drawn from the parse table.

Each page is generated from that grammar's `src/grammar.json` — the file
Tree-sitter itself consumes, rather than the hand-written `grammar.js`. So the
page cannot drift from the parser: if a production is on the page, the parse
table has it.

Each page opens with that grammar's status — pass rate, known gaps, what it
declares about itself — then its vocabulary, its precedence table, and every
production.

<link rel="stylesheet" href="/grammar.css">
<div class="status-overview"><p class="grammar-loading">Loading the inventory…</p></div>
<script type="module" src="/status-view.mjs"></script>

## What the diagrams show

**Precedence**, drawn around the production it applies to as well as
tabulated. EBNF cannot express precedence, so manuals normally print it
separately.

**Fields**, as captions inside the boxes, so the edge names a query can use
sit next to the shape they attach to.

**Externals**, in plum — the parts handed to the external scanner, which is
hand-written C rather than grammar.

**Vocabulary**, with productions grouped under the supertype they answer.

## Two deliberate choices

**Hidden rules are not inlined.** The `_or_test` → `_and_test` → `_not_test`
chain *is* the precedence structure — the same thing a language manual shows
as `expr → boolean_primary → predicate → bit_expr → simple_expr`. Inlining it
would remove the most informative part of the page.

**Comma-lists are collapsed.** `seq(X, repeat(seq(',', X)))` is drawn as one
loop rather than a chain of five boxes, without which about half the diagrams
are unreadable.
