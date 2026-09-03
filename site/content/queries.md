---
title: Editor queries
description: One highlights.scm for nine languages, generated rather than hand-written.
order: 8
---

Every tree-sitter grammar ships a `queries/highlights.scm` naming its own node
types. There are hundreds of them in the wild, each written by hand, and they
drift — because nothing holds them together beyond the care of whoever last
touched one.

Treebank has something that does hold them together. A supertype is threaded
through the productions and a nominal term is declared in `terms.json`, so
`(_callable)` means the same thing in every grammar. That makes it possible to
write the file once:

```scm
(_comment) @comment
(_string) @string
(_literal) @number
(_identifier) @variable
(_callable) @function
(_invocation) @function.call
(_loop) @keyword.repeat
(_branch) @keyword.conditional
(_jump) @keyword.return
(_directive) @keyword.import
```

Ten patterns, no language-specific node name anywhere, and it captures in
bash, C, C++, Java, Python, Ruby, Rust, TypeScript and Zig.

## Generated, not copied

An editor cannot load that file. A nominal term is not a rule in the parse
table, so
tree-sitter has nothing to match `(_callable)` against. The file above is the
**source**; what ships with each grammar is derived from it, with nominal terms
expanded into that grammar's own members:

```scm
; crates/treebank-python/queries/highlights.scm
[(function_definition) (lambda)] @function
```

```scm
; crates/treebank-rust/queries/highlights.scm
[(function_definition) (closure_expression)] @function
```

`treebank queries` writes them; `treebank queries --check` regenerates and
fails on any difference, so a file edited in place is caught rather than
quietly kept. Both run the same expansion a consumer gets from `Pack::query`.

The check also **compiles** each generated file against its grammar. Matching
the source only proves the copies agree — a source that expanded into an
impossible pattern would regenerate perfectly and be equally broken in all
nine. It is also where a vocabulary mistake surfaces: a role threaded in eight
grammars and forgotten in the ninth is invisible to the generator, because an
absent supertype is just a node name, and only tree-sitter can say it does not
exist.

## What one file covers

Measured over each grammar's own test corpus, as the share of named nodes that
receive a capture:

| grammar | named nodes | captured | |
| --- | --- | --- | --- |
| bash | 216 | 133 | 61.6% |
| c | 559 | 207 | 37.0% |
| cpp | 271 | 112 | 41.3% |
| java | 178 | 84 | 47.2% |
| python | 508 | 238 | 46.9% |
| ruby | 755 | 355 | 47.0% |
| rust | 383 | 186 | 48.6% |
| typescript | 227 | 111 | 48.9% |
| zig | 703 | 352 | 50.1% |

**3800 named nodes, 1778 captured: 46.8%** — from one file, `treebank queries
--coverage`, reported in every CI run. `locals.scm` reaches 41.4% of the same
nodes.

Two honest caveats. These corpora are small, and a number over the locked
corpora would mean more; that is a sweep rather than a check, so it is not run
on every push. And a named node is the unit: keywords and punctuation are
anonymous tokens, which no vocabulary term reaches. Colouring the `if` itself
needs a per-grammar supplement, which is the remaining half of a complete
highlighting file.

## locals.scm

The file that drives rename, go-to-definition and highlight-references is the
hardest of the standard queries to write, because it is about a language's
binding structure rather than its surface. It is normally written once per
language, by someone who knows that language well.

Its three ingredients are in every grammar's vocabulary, so it is written once
here too:

```scm
(_scope) @local.scope
(_callable name: (_) @local.definition.function)
(_binding name: (_) @local.definition.var)
(_parameter name: (_) @local.definition.parameter)   ; where the grammar has one
(_identifier) @local.reference
```

The capture goes on the **name**, not on the construct: an editor renames an
identifier and needs the range of the thing it would rewrite. What that finds,
on the playground's own samples:

| grammar | definitions found |
| --- | --- |
| bash | `greet`, `name` |
| java | `Point`, `x`, `y` |
| python | `greet`, `name` |
| ruby | `Greeter`, `initialize`, `call`, `name` |
| rust | `largest` |
| zig | `std`, `main`, `xs` |

### Where a grammar cannot express a pattern

Two things vary, and both are written into the generated file rather than
left for someone to discover.

**A term the grammar does not have.** bash functions take `$1`, not a named
parameter, so bash has no `_parameter` at all. The pattern is marked
`; treebank: only-if _parameter` and is omitted there — rather than dropped
from the eight grammars that do have one.

**A term whose members do not take the field.** C and Rust have `_parameter`,
but a C parameter is a *declarator* and a Rust one is a *pattern*, so neither
has a `name`. Expansion filters every member away and the pattern is omitted,
with the reason in the file's header.

The same thing costs C most of its definitions. `int counter = 0;` binds
through `declarator`, not `name`, so C's generated file captures scopes and
references but only names macros and enum constants. `(_binding declarator:
(_identifier))` finds exactly `counter, rate, a, b, t` in C — and is a compile
error in C++, whose declarators nest differently. That belongs in a
per-grammar supplement rather than the universal file, alongside the keywords.

This is worth saying plainly: **one file gets a long way, and it does not get
all the way.** The vocabulary makes the common shape writable once; the
declarator languages need help that only they need.
