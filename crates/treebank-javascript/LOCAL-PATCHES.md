# treebank-javascript

Vendored [tree-sitter/tree-sitter-javascript](https://github.com/tree-sitter/tree-sitter-javascript)
at **0.25.0** (`f76aea6aa47322ea5c208c9c2e67f4a350d554f3`). One grammar; it
parses JSX as well as plain JavaScript, so `.jsx` needs no separate routing.
Generation needs no npm deps (`generate_deps` is null — `grammar.js` requires
nothing). Contract, reconstruction invariant, CLI pin rationale, and
workflow: see [GRAMMARS.md](../../GRAMMARS.md) at the repo root.

## Reference parser

`tools/js-oracle` is V8 itself, driven the way Node drives it: `vm`'s CJS
wrapper compile or `SourceTextModule`, picked by Node's own `.mjs`/`.cjs`/
nearest-`package.json` rules. Nothing is linked or evaluated, so corpus code
never runs. Files V8 rejects get one more chance from `@babel/parser` with
**only** the `jsx` plugin, because JSX is not JavaScript but this grammar
parses it and npm ships it.

`tools/ts-oracle` is *not* usable here even though TypeScript is a superset
of JavaScript: it parses with `ScriptKind.TS` and reports no parse errors for
`const x: number = 1`, `interface Foo {}`, `enum E {}`, `x as string` or
`obj!.prop`. Adjudicating JavaScript with it would turn this grammar's
correct rejection of TypeScript into reported "grammar gaps" and point a fix
agent at making the JavaScript grammar accept TypeScript. Measured on a
battery of 18 known-verdict files, ts-oracle scored 7/18, the V8+JSX oracle
18/18; across 20 further early-error cases the babel leg never accepted
anything V8 rejects.

## Patches

1. **Reserved words as exported names** (`0001`) — `export { _import as
   import }`. The exported name in an `export_specifier` (and in
   `export * as X`) is a `ModuleExportName`, i.e. an IdentifierName, so
   reserved words are legal there; it now uses the same
   `reserved('properties', …)` context the grammar already uses for member
   and property names. The local name *before* `as` is deliberately left
   reserved: with no from-clause an early error requires an
   IdentifierReference, so `export { import }` must keep failing. Found by
   the npm top-100 sweep in @babel/types 8.0.4 (1 file).

   The reserved context has to wrap the choice at the use site rather than
   the shared `_module_export_name` rule — a `RESERVED` node wrapping a
   symbol reference does not reach the states of a rule that is also used
   uncontexted elsewhere.

   Still unsupported, and untested by the corpus: `export { class as x } from
   'm'` and `import { class as C } from 'm'`, where the *module-side* name
   may also be an IdentifierName. Both sit in the same parse state as the
   forms that must stay reserved, so allowing them needs more than a
   reserved-context change.

2. **ASI before a subscript or call that cannot continue an expression**
   (`0002`) — `let subnamespace` followed by `[subnamespace, args] = f()` on
   the next line. The scanner refused automatic semicolon insertion before
   `[` and `(` unconditionally, since they usually continue an expression
   (`a\n[0]` is `a[0]`, not two statements). It now refuses only where an
   expression can be continued at all, which it already knows from
   `valid_symbols[LOGICAL_OR]` — the same context signal it uses to decide
   ASI after a comment. Found by the npm top-100 sweep in argparse 3.0.0
   (1 file).

   This also fixes two cases upstream misparsed *silently*, with no ERROR
   node for a sweep to catch: `return\n[1]` parsed as returning the array
   instead of a bare `return`, and `class C { x\n[y] = 1 }` parsed the
   second field's name as a subscript of the first. The corpus test locks
   both directions — the four constructs that must split, and three
   (`a\n[0] = 1`, `let c = b\n(c)`, `f()\n[0]`) that must not.

## Negative corpus

`test/negative/` holds 11 files the reference parser rejects and this
grammar must keep rejecting: eight TypeScript constructs (the direction an
agent optimizing pass rates would drift toward), the two reserved-word
module forms that are early errors, and one plainly broken file.

Five further invalid cases were tried and left out because they are early
errors no context-free grammar can catch: duplicate constructors, a
duplicated regex flag, a getter with a parameter, `if (1) let x = 1;`, and
`a?.b = 1`.
