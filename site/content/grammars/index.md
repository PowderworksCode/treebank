---
title: Grammar reference
description: Every production in every grammar, as EBNF and as a railroad diagram.
order: 50
---

The honest way to document a parser is to render its parse table rather than
describe it.

<link rel="stylesheet" href="/grammar.css">
<div class="status-overview"><p class="grammar-loading">Loading the inventory…</p></div>
<script type="module" src="/status-view.mjs"></script>

Each page here is generated from that grammar's `src/grammar.json` — the
normalised grammar tree-sitter consumes, not the hand-written `grammar.js`.
`grammar.js` is arbitrary JavaScript, and reading it means running it.
`grammar.json` is already an EBNF syntax tree over sixteen node kinds, so
rendering is a fold over sixteen cases rather than a parse, and there is no
per-language code anywhere in it.

That last point is the one worth testing rather than asserting. The renderer
was written when there were three grammars. Six of the nine were written
afterwards — bash, C, C++, Java, Ruby and Zig — and every one of them renders
without a line of the renderer changing.

## What a page shows that a BNF listing cannot

**Precedence**, drawn around the production it applies to as well as
tabulated. EBNF cannot express precedence at all, which is why every language
manual prints it separately.

**Fields**, as captions inside the boxes, so the edge names a query can use
sit next to the shape they attach to.

**Externals**, in plum — the external scanner, the part no diagram can
explain because it is hand-written C.

**Vocabulary**, with productions grouped under the supertype they answer
rather than listed alphabetically.

## Two things deliberately not done

**Hidden rules are not inlined.** It is tempting to fold `_or_test` and
friends away, but that chain *is* the precedence structure — the same thing a
language manual shows as `expr → boolean_primary → predicate → bit_expr →
simple_expr`. Inlining it would delete the most informative part of the page.

**The comma-list idiom is collapsed.** `seq(X, repeat(seq(',', X)))` is
recognised and drawn as a single loop instead of a chain of five boxes.
Without it about half the diagrams are unreadable.
