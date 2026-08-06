# treebank-typescript

Vendored [tree-sitter/tree-sitter-typescript](https://github.com/tree-sitter/tree-sitter-typescript)
at **0.23.2** (`f975a621f4e7f532fe322e13c4f79495e0a7b2e7`). Two grammars
(`typescript/`, `tsx/`; `.tsx` corpus files route to the second), shared
scanner in `common/`. Contract, reconstruction invariant, CLI pin rationale,
and workflow: see [GRAMMARS.md](../../GRAMMARS.md) at the repo root.

Generation needs `npm ci` first (`generate_deps` in the ledger —
define-grammar.js imports tree-sitter-javascript).

## Patches

1. **`export type * from`** (`0001`) — `export type * from './x.d.ts';` and
   `export type * as ns from './x.d.ts';` (TS 5.0): the `export type`
   branch of `export_statement` accepts the same `*` / `namespace_export`
   variants as the base grammar's `export *`. Found by the npm top-100
   sweep in type-fest 5.8.0 and uuid 14.0.1 (5 files).

2. **`abstract` as a property name** (`0002`) — `abstract?: boolean | null;`
   in an interface: `abstract` is a contextual keyword, valid as an
   identifier/property name; added to the `_reserved_identifier` override
   like `declare`/`override`/`readonly`. `abstract class` and
   `abstract` members are unaffected. Found by the npm top-100 sweep in
   @babel/types 8.0.4 (2 files).
