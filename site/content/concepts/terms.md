---
title: The vocabulary
description: Structural and nominal terms, and why there are two kinds.
order: 15
---

Treebank gives every grammar a shared vocabulary, so the same query can be
written once and run against several languages. A query for `(_declaration)`
finds declarations in Rust and in Java.

Every term is one of two kinds, and which one is decided by what Tree-sitter
can enforce rather than by what would read nicely.

**Structural** terms are threaded through the productions as real supertypes
and enforced when the parser is generated. A query for `(_expression)` matches
where the parse actually went through it — membership is decided by structure,
not by node type. That lets a structural term say something a list cannot:
that *this position in this production* is an expression.

**Nominal** terms are lists of node types in `terms.json`, expanded into a
concrete alternation when a query loads. Membership is decided by name: a
`function_definition` is `_callable` wherever it occurs. A nominal term cannot
say anything about position, because it does not exist in the parse table.

Where a term can be threaded it is structural. Where it cannot, it is nominal,
because a term that looks enforced and is not is worse than a list that is
honest about being one. The same term can therefore be structural in one
grammar and nominal in another — the parse tables differ.

`treebank terms` checks this: declared supertypes come from the closed list,
every named node is either covered or deliberately uncategorised, and every
member of `terms.json` exists.
