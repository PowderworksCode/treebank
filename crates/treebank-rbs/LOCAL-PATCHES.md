# Local patches — treebank-rbs

Upstream: [joker1007/tree-sitter-rbs](https://github.com/joker1007/tree-sitter-rbs)
pinned at `5282e2f36d4109f5315c1d9486b5b0c2044622bb`, the commit tag `v0.2.2`
points at, which is also the default branch head at pin time.

RBS is Ruby's type signature language — a separate language, not a dialect.
`def foo: (Integer) -> String` is RBS and CRuby rejects it; `def foo(a) = a + 1`
is Ruby and `RBS::Parser` rejects it. Both corpora come out of the same gems,
which is exactly what makes crossing the two oracles an easy mistake.

Eleven patches: two packaging, nine grammar. On the 1000-gem, 2,214-file corpus
they take the sweep from 2,062 passing to **2,190 — 93.1% to 98.9%**.

`noise_files` is 0 at every patch level, and for this language that says
something stronger than usual: exactly one file in the whole corpus is
oracle-invalid, and the grammar accepts it (see `ledger.json`'s
`grammar_looseness`). So every failing file is a genuine gap and `gap_files` is
exact rather than a floor.

## Why this upstream, and why not our own

The first grammar vendored here that is not from `tree-sitter/` or
`tree-sitter-grammars/`: 15 stars, one maintainer. That was measured before
being adopted, not after — `ledger.json`'s `grammar_assessment` has the
numbers. The short version: 93% out of the box, **zero** wrong acceptances
across 22 adversarial invalid signatures, trees with real fields and
spec-derived node names, 133 corpus tests of its own, and it generates cleanly
under our pinned CLI. Its failures are "has not caught up with RBS 4.x", not
"designed wrong", which is what the vendor-and-patch model is for.

Writing our own was considered and rejected: RBS's reference parser is 4,258
lines of C against a 940-line spec, upstream covers it in 489 lines of
`grammar.js`, and a treebank-authored grammar is a fork nobody upstreams.

## 0001 — treebank redistribution notice

Prepends the standard warning to `README.md`. Touches no grammar code.

## 0002 — treebank crate identity

Publishes as `treebank-grammar-rbs` with treebank's repository, homepage and
description; `[lib] name` stays `tree_sitter_rbs` so the crate is a drop-in
replacement, and `include` gains `LOCAL-PATCHES.md`, `ledger.json` and
`patches/*`. Upstream ships no `Cargo.lock`, so unlike the other grammars there
is none to rename.

## 0003 — method names ending in bang · 62 files

The single largest gap. RBS's syntax doc gives both `?` and `!` as method-name
suffixes; only `?` was implemented.

The precedence that comes with it is **token-level, not rule-level**, and that
distinction is the whole fix for `alias`. `filter!` is a *lexer* choice between
the immediate suffix and the `!` operator, which is itself a legal method name —
`?` never had the ambiguity because there is no `?` operator. Without it,
`alias filter! select!` lexes as `alias filter !` and loses the rest of the
line. Rule-level `prec` was tried first and does not reach the decision.
`alias ! not_op`, aliasing the operator itself, still parses.

## 0004 — annotations on a nested declaration · 10 files

A module or class body is `repeat(choice(member, _nestable_decls))`. Annotations
were accepted on a top-level declaration and on a member, but not on the nested
branch — so `%a{private}` before a nested `interface`, how rbs's own core
signatures mark an internal interface, failed the whole enclosing module.

## 0005 — trailing comma in type arguments · 10 files

Record and tuple types already allowed one. aws-sdk's generated signatures wrap
long argument lists and leave a comma behind: `Array[\n  { ... },\n]`.

## 0006 — signed integer literal type · 7 files

A type position has no unary minus to fall back on, so the sign has to be part
of the literal. rbs uses it for comparison results (`-> (-1 | 0 | 1)`) and
language_server-protocol declares its LSP error codes as negative constants.

## 0007 — bounded type parameters on a method type · 12 files

A module's type parameters could carry an upper bound; a method's could not.
Variance and `unchecked` are deliberately **not** accepted here — RBS allows
them only on a module's parameters, and `RBS::Parser` rejects
`def f: [out T] () -> T`; `test/negative/` holds that rejection. Inlined rather
than given its own node, so an unbounded parameter still parses to a bare
`type_variable` and upstream's corpus tests keep passing unchanged.

## 0008 — default for a module type parameter · 6 files

`interface _Each[out T, out R = void]`, `class Thread::Queue[E = untyped]`.
Added in RBS 3.7; the grammar predates it.

## 0009 — overload list ending in ellipsis · 10 files

`...` was accepted as a whole method type — "this method's overloads are
elsewhere" — but not as the *terminator* of a list. RBS allows both, and the
terminating form is how aws-sdk's hand-written customizations extend generated
signatures.

## 0010 — ivar name beginning with underscore · 3 files

`@_memoized_method_cache`. Legal in Ruby and in RBS; only a letter was accepted,
so a module declaring one failed entirely.

## 0011 — backtick-quoted parameter name · 8 files

`def add_include: (RDoc::Include \`include\`) -> RDoc::Include`. `method_name`
already accepted the same quoting for a method called `class` or `include`;
`var_name` did not.
