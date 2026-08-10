# treebank-typescript

Upstream [tree-sitter/tree-sitter-typescript](https://github.com/tree-sitter/tree-sitter-typescript)
pinned at **0.23.2** (`f975a621f4e7f532fe322e13c4f79495e0a7b2e7`) as the
`upstream/` git submodule; `scripts/materialize.sh` applies the patch series
below and generates both parsers into `build/` (gitignored). Two grammars
(`typescript/`, `tsx/`; `.tsx` corpus files route to the second), shared
scanner in `common/`. Contract, reconstruction invariant, CLI pin rationale,
and workflow: see [GRAMMARS.md](../../GRAMMARS.md) at the repo root.

Generation needs `npm ci` first (`generate_deps` in the ledger —
define-grammar.js imports tree-sitter-javascript).

`test/negative/` holds files `tools/ts-oracle` (TypeScript's own parser,
syntax diagnostics only) rejects; the grammar must keep rejecting them. It
guards the accepts-invalid-code direction that a corpus sweep cannot see —
several patches below deliberately keep a construct illegal outside the one
context that allows it.

## Patches

1. **Treebank redistribution notice** (`0001`) — prepends a warning to
   upstream's `README.md` stating that this tree is an automatically
   generated, patched redistribution maintained by
   [treebank](https://treebank.dev), so the notice travels with every
   materialized/published copy. Applied first; touches no grammar code.

2. **`export type * from`** (`0002`) — `export type * from './x.d.ts';` and
   `export type * as ns from './x.d.ts';` (TS 5.0): the `export type`
   branch of `export_statement` accepts the same `*` / `namespace_export`
   variants as the base grammar's `export *`. Found by the npm top-100
   sweep in type-fest 5.8.0 and uuid 14.0.1 (5 files).

3. **`abstract` as a property name** (`0003`) — `abstract?: boolean | null;`
   in an interface: `abstract` is a contextual keyword, valid as an
   identifier/property name; added to the `_reserved_identifier` override
   like `declare`/`override`/`readonly`. `abstract class` and
   `abstract` members are unaffected. Found by the npm top-100 sweep in
   @babel/types 8.0.4 (2 files).

4. **Treebank crate identity** (`0004`) — packaging, not a grammar change.
   Same shape as rust's: publishes as `treebank-grammar-typescript` with our
   `repository`/`homepage`/`description`, keeps `[lib] name =
   tree_sitter_typescript` so the crate stays drop-in, and ships
   `ledger.json`, `LOCAL-PATCHES.md` and `patches/*` inside the tarball. It
   also fixes two defects that would otherwise ship: upstream's `include`
   omits `LICENSE`, which would make this an unlicensed redistribution, and
   the dev-dependency on `tree-sitter` 0.24 cannot load the ABI-15 parsers
   the pinned CLI 0.25.10 generates, so the crate's own tests fail against
   its own parser. See [PUBLISHING.md](../../PUBLISHING.md).

5. **Import types as primary types** (`0005`) — `import("x").Y` was only a
   `type`, so it could not take type arguments or take part in any of the
   type combinators: `import("m").T<A>`, `import("m").T[]`,
   `A | import("m").T`, `Promise<import("m").T>` all failed. The two
   `_type_query_*_in_type_annotation` aliases move from `type` to
   `primary_type` (keeping their `prec(-1)`), and `generic_type` accepts the
   member-expression form as a `name`. Found by the npm top-1000 sweep — the
   single largest gap, ~1050 files across foreground-child 4.0.3, date-fns
   4.4.0, @aws-sdk/nested-clients, rxjs, yaml and many `.d.ts` bundles.

6. **Anonymous default-exported function signature** (`0006`) —
   `export default function (): Promise<void>;`: a bodiless function
   signature exported as default has no name. A hidden
   `_anonymous_function_signature` (aliased to `function_signature`) is
   reachable only from the `export default` branch of `export_statement`,
   so `declare function (): void;` — which the reference parser rejects —
   stays an error (it is in `test/negative/`). Found by the npm top-1000
   sweep in escalade 3.2.0, mime 4.1.0, zod 4.4.3 and diff 9.0.0 (78 files).

7. **Global augmentation without `declare`** (`0007`) — inside an ambient
   module body the enclosing `declare` is implicit, so the augmentation is
   written bare: `declare module "m" { global { … } }`. A new
   `global_augmentation` statement covers it; it is a *statement*, not a
   `declaration`, so `declare global { … }` keeps its single
   `ambient_declaration` parse instead of becoming ambiguous. `global` joins
   the `_reserved_identifier` list so `global.foo = 1` and `const global = 1`
   still parse. Found by the npm top-1000 sweep in @types/node 26.2.0
   (8 files).

8. **Variance annotations on type parameters** (`0008`) — TS 4.7's optional
   `in` / `out` / `in out` modifiers, as in `interface $ZodCheck<in T = never>`
   and `interface ZodType<out Output = unknown, out Input = unknown>`. Two
   optional tokens in `type_parameter`, beside the existing `const` modifier
   and in TypeScript's own order (`const`, then `in`, then `out`). Found by
   the npm top-1000 sweep in zod 4.4.3 (12 files).

9. **Parenthesized import types** (`0009`) — `(import('./types').A |
   import('./types').B)[]`: after `(` in a type position the parser had to
   choose between a parameter pattern (`member_expression`) and an import
   type, and the two `precedences` entries pinning `member_expression` /
   `call_expression` above their `_type_query_*_in_type_annotation`
   counterparts settled it statically for the parameter — so every
   parenthesized import type was a parse error. Those two entries move from
   `precedences` to `conflicts`, letting the parser explore both and keep the
   branch that reaches `)`. Found by the npm top-1000 sweep in yaml 2.9.0 and
   @aws-sdk/core 3.977.6 (7 files).

10. **Contextual keyword as a mapped type key** (`0010`) — `[type in keyof
    AggregateType]?: …`: the mapped-type key is an ordinary binding name, so
    contextual keywords are legal there. `mapped_type_clause`'s `name` accepts
    `_reserved_identifier` (aliased to `type_identifier`), the same allowance
    the `[k: string]` branch of `index_signature` already makes. Found by the
    npm top-1000 sweep in acorn-walk 8.3.5 (2 files).

11. **Space between `?` and `:` in an optional mapped type** (`0011`) —
    `{ [K in keyof T]? : DeepPartial<T[K]> }`. `opting_type_annotation`
    matched the single token `'?:'`, so any whitespace between the two
    characters was a parse error; it is now `seq('?', ':')`, which still
    matches the unspaced form. Found by the npm top-1000 sweep in
    @vitest/spy and @vitest/expect 4.1.10 (2 files).

12. **Optional destructured parameter in a function type** (`0012`) —
    `({ onlyFirst }?: { … }) => RegExp`. External-token fix, in the automatic
    semicolon scanner: it already suppresses ASI for `({a}: T) => …`, where
    inserting one before `}` would destroy the object-*pattern* reading, but
    only when `:` follows the `}`. The same suppression now covers a `?` that
    is itself followed by `:` or `)` — an optional parameter — while a `?`
    followed by anything else stays a ternary. Found by the npm top-1000
    sweep in @isaacs/cliui 9.0.0 (2 files).

13. **Escape sequences in template literal types** (`0013`) — a template
    literal *type* is still a template literal, so it may contain escapes:
    `` `${infer L}$\{${infer R}\}` `` escapes the `${` that would otherwise
    open an interpolation. `template_literal_type` accepts `escape_sequence`
    between its fragments, as `template_string` already does. Found by the
    npm top-1000 sweep in @sinclair/typebox 0.34.52 (2 files).

14. **Generic call signature on the next line** (`0014`) — semicolon-free
    overload lists, where a member's return type ends one line and the next
    member starts with `<`:

    ```ts
    interface I {
      (g: string): number
      <E>(g: string): number
    }
    ```

    External-token fix: the automatic semicolon scanner suppressed insertion
    before a leading `<` unconditionally, which is right for an expression
    (comparison, or type arguments continuing the line) but wrong in a type,
    where `<` can only begin the next object-type member. The `<` case now
    makes the same expression/type test (`valid_symbols[LOGICAL_OR]`) that
    `(` and `[` already make. Found by the npm top-1000 sweep in vite 8.2.1
    and immer 11.1.16 (2 files).
