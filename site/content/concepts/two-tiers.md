---
title: The vocabulary
description: Supertypes and facets, and why there are two kinds.
order: 15
---

Treebank gives every grammar a shared vocabulary, so the same query can be
written once and run against several languages. A query for `(_declaration)`
finds declarations in Rust and in Java.

The vocabulary comes in two kinds, and the split is decided by what
Tree-sitter can enforce rather than by what would read nicely.

**Supertypes** are threaded through the productions and enforced when the
parser is generated. A query for `(_expression)` matches where the parse
actually went through it — matching is by derivation, not by node type. That
lets a supertype say something a list cannot: that *this position in this
production* is an expression.

**Facets** are lists of node types in `roles.json`, expanded into a concrete
alternation when a query loads. A facet cannot say anything about position,
because it does not exist in the parse table.

Where a term can be threaded it is a supertype. Where it cannot, it is a
facet, because a role that looks enforced and is not is worse than a list that
is honest about being one. The same term can therefore be a supertype in one
grammar and a facet in another — the parse tables differ.

`treebank roles` checks this: declared supertypes come from the closed list,
every named node is either covered or deliberately uncategorised, and every
member of `roles.json` exists.
