---
title: Two tiers
description: Why the vocabulary splits into supertypes and facets, and why the parse table decides.
order: 10
---

The vocabulary splits in two, and the split is forced by the parse table
rather than chosen for tidiness.

**Supertypes** are occurrence-level and enforced when the parser is
generated. A supertype is a real rule threaded through the productions, so a
query for `(_expression)` matches where the parse actually went through it.
Matching is by *derivation*, not by node type — which means a supertype can
say something no type-level list can: that this position in this production
is an expression.

**Facets** are type-level. A facet is a list in `roles.json` that expands into
a concrete alternation when a query is loaded. It cannot say anything about
position, because it does not exist in the parse table at all.

The reason for two tiers rather than one is that tree-sitter's supertypes can
only express so much, and the boundary is measurable rather than a matter of
taste. Where a term can be threaded, it is a supertype and the generator
enforces it. Where it cannot, pretending otherwise would mean a role that
looks enforced and is not — which is worse than a facet that is honest about
being a list.

A term's tier is therefore **per grammar**. The same word can be a supertype
in one language and a facet in another, because the parse tables differ.

`treebank roles` checks conformance: declared supertypes come from the closed
table tier, every named node is either covered or deliberately uncategorised,
required containments hold, and every member of `roles.json` actually exists.
