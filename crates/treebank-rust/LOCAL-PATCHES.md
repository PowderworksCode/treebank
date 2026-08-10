# treebank-rust

Upstream [tree-sitter/tree-sitter-rust](https://github.com/tree-sitter/tree-sitter-rust)
pinned at **0.24.2** (`77a3747266f4d621d0757825e6b11edcbf991ca5`) as the
`upstream/` git submodule. `scripts/materialize.sh` applies the patch series
below and generates the parser into `build/` (gitignored) — nothing generated
is committed. Contract, CLI pin rationale, and workflow: see
[GRAMMARS.md](../../GRAMMARS.md) at the repo root.

## Patches

1. **Treebank redistribution notice** (`0001`) — prepends a warning to
   upstream's `README.md` stating that this tree is an automatically
   generated, patched redistribution maintained by
   [treebank](https://treebank.dev), so the notice travels with every
   materialized/published copy. Applied first; touches no grammar code.

2. **Extern types** (`0002`) — `pub type Foo;` inside `extern` blocks (port of
   upstream PR [#281](https://github.com/tree-sitter/tree-sitter-rust/pull/281)):
   `associated_type` accepts an optional visibility modifier. First seen in
   wasm-bindgen/web-sys generated bindings.

3. **`~` in token trees** (`0003`) — added `~` to
   `TOKEN_TREE_NON_SPECIAL_PUNCTUATION`. Still a valid rustc lexer token and
   used in macro matchers in the wild (anyhow's `ensure!` fuel counters,
   serde_json's ui tests). Unreported upstream as of 2026-08.

4. **Immediate lifetime/label identifiers** (`0004`) — `lifetime` and `label`
   use `token.immediate` after `'`. rustc lexes `'a` as one token, so `' a` is
   not valid Rust; upstream accepted it. Strictness patch: the grammar should
   reject what the reference rejects (see `test/negative/`).

5. **Macros named like primitive types** (`0005`) — `str!["hi"]`, `u32![1]`:
   `macro_invocation` now accepts primitive-type names as the macro name
   (aliased to `identifier`, mirroring `_path`). Found by the top-100 sweep in
   winnow's snapbox `str!` snapshot tests (5 files).

6. **Negative literals in const generic arguments** (`0006`) —
   `ri8<-25, 25>`: `type_arguments` accepts `negative_literal`, mirroring
   `const_parameter` defaults. Found by the top-100 sweep in time 0.3.55's
   deranged ranged-integer type aliases (3 files).

7. **`try!` macro invocation** (`0007`) — `try!(g())`: `try` is only a
   keyword in edition 2018+; as a macro name it is valid 2015-edition code
   and unambiguous (a `try_block` needs `{`). `macro_invocation` accepts
   `try` aliased to `identifier`. Found by the top-100 sweep in autocfg
   1.5.1 (2 files).

8. **Bare `$` in macro token trees** (`0008`) — `($mode:ident, $) => { 1 }`:
   a `$` not starting a metavariable or repetition is a literal token in
   matchers and transcribers. Added `prec(-1, '$')` to `_token_pattern` and
   `_tokens`; the low precedence keeps `$(...)*` parsing as a repetition.
   Found by the top-100 sweep in syn 3.0.3's punctuation macros (2 files).

9. **Unit type in where predicates** (`0009`) — `where (): Target<V>`:
   `where_predicate` accepted `tuple_type` but not `unit_type`, so `()` on
   the left of a bound failed. Found by the top-100 sweep in time 0.3.55 and
   typenum 1.20.1 (2 files).

10. **`safe fn` in unsafe extern blocks** (`0010`) — RFC 3484:
   `unsafe extern "C" { safe fn f() -> u64; }`. The `safe` qualifier is only
   reachable through an `unsafe extern` block body (`_extern_declaration_list`),
   never `function_modifiers` — plain `extern` blocks keep rejecting it
   (`test/negative/safe-in-plain-extern.rs`). Fix shape comes from the
   adversarial review parked at `corpus/rust/reports/parked-safe-fn-analysis/`,
   whose first draft leaked `safe` into plain extern blocks. Found by the
   top-100 sweep in getrandom 0.4.3's WASI backend (1 file).

11. **Attributes on struct pattern fields** (`0011`) —
    `S { #[cfg(x)] inner: a, .. }`: `struct_pattern` fields accept leading
    `attribute_item`s, same idiom as `field_declaration_list`. Found by the
    top-100 sweep in proc-macro2 1.0.107's fallback.rs (1 file).

12. **Unit struct with where clause** (`0012`) —
    `struct _Test where Error: Send + Sync;`: the unit-struct alternative of
    `struct_item` now takes an optional `where_clause` before `;`. Found by
    the top-100 sweep in syn 3.0.3's error.rs (1 file).

13. **Underscore separators in unicode escapes** (`0013`) — `"\u{4_e}"`,
    `'\u{4_e}'`: underscores are digit separators inside `\u{...}` (the
    first character must still be a hex digit). Applied to both
    `escape_sequence` and `char_literal`. Found by the top-100 sweep in
    time 0.3.55's macro tests (1 file).

14. **Treebank crate identity** (`0014`) — packaging, not a grammar change.
    Upstream owns `tree-sitter-rust` on crates.io, so the redistribution
    publishes under its own name (`treebank-grammar-rust`), `repository`,
    `homepage` and `description`. `[lib] name` stays `tree_sitter_rust` so the
    crate is a drop-in: `use tree_sitter_rust::LANGUAGE` still compiles. The
    `include` list gains `LICENSE`, `ledger.json`, `LOCAL-PATCHES.md` and
    `patches/*` so provenance travels inside the published tarball, and
    `Cargo.lock`'s stale `0.24.1` self-version is corrected to match the
    manifest. The published version string is deliberately *not* here — it is
    derived from crates.io at publish time. See
    [PUBLISHING.md](../../PUBLISHING.md).

15. **Turbofish in type position** (`0015`) — `Punctuated::<Ident, Token>`,
    `*const Gray_v09::<T>`, `Result::<Vec::<Tag>, Error>::Ok(..)`: the `::`
    before type arguments is redundant in type position but rustc accepts it,
    and generated code emits it. `generic_type` takes an `optional('::')`
    before its `type_arguments`, mirroring the `generic_type_with_turbofish`
    shape already aliased to `generic_type` in expression position. Found by
    the top-1000 sweep in aws-sdk-s3 1.140.0's `protocol_serde` shapes,
    mockall_derive 0.15.0 and rgb 0.8.53 (37 files).

16. **Multiple attributes on function parameters** (`0016`) —
    `fn f(#[future] #[default(1)] x: u32)`: `parameters` allowed only
    `optional($.attribute_item)` per parameter, so a second attribute landed
    in an ERROR. Attributes on parameters have been stable since Rust 1.39 and
    stack like any other attribute position, so the `optional` becomes a
    `repeat`. Found by the top-1000 sweep in rstest 0.26.1's test resources
    (15 files).

17. **Attributes on tuple expression elements** (`0017`) —
    `(a, #[cfg(unix)] b,)`: `tuple_expression` accepted attributes only ahead
    of the *first* element, so a `#[cfg]`/`#[expect]` on any later member
    landed in an ERROR. Every element now takes a leading `repeat($.attribute_item)`,
    the same shape `arguments` and `array_expression` already use. Found by
    the top-1000 sweep in opentelemetry-otlp 0.32.0, sqlx-mysql 0.9.0,
    block2 0.6.2, zbus 5.18.0 and zerovec 0.11.6 (8 files).

18. **Macro invocation on the left of a where predicate** (`0018`) —
    `where mac!(Self): Send`: `_type` already allowed `macro_invocation`, but
    `where_predicate` spelled its left-hand side out as an explicit type list
    that omitted it, so a macro-generated bound subject failed. Found by the
    top-1000 sweep in pin-project 1.1.13 and async-trait 0.1.91 (3 files).

19. **Indexed fields in struct patterns** (`0019`) — `Tuple { 1: x, .. }`:
    a tuple struct's fields can be matched by index in braced-struct pattern
    syntax, but `field_pattern` accepted only an identifier as the name.
    It now takes `choice($._field_identifier, $.integer_literal)`, the same
    idiom `field_expression` already uses for `x.0`. Surfaced under the
    where-predicate cluster: async-trait 0.1.91's `tests/test.rs` hits both
    this and patch `0018` (1 file).

20. **`<=` after a cast to a non-primitive type** (`0020`) —
    `if a.b(c) as size_t <= length`: `type_arguments` opens with
    `token(prec(1, '<'))`, and tree-sitter's lexer settles ties by token
    precedence *before* match length. After `as size_t` a `type_arguments`
    can legally start, so the high-precedence `<` beat the longer `<=` and
    the operator was split. `<=` is now `token(prec(1, '<='))`, so the two
    sit at the same precedence and longest-match decides. Deliberately not
    applied to `<<`/`<<=`: `d as *mut E<<F as E>::G>` needs `<` to win there.
    Found by the top-1000 sweep in unsafe-libyaml 0.2.11 (2 files).

21. **Turbofish generics in struct patterns** (`0021`) —
    `let Range::<Idx> { start, end } = r;`: pattern position requires the
    turbofish (`Range<Idx> { .. }` is not valid there), but `struct_pattern`
    accepted only a plain or scoped type identifier. It now also takes
    `generic_type_with_turbofish` aliased to `generic_type`, the same
    aliasing used in expression position. Found by the top-1000 sweep in
    serde_with 3.21.0 and pyo3 0.29.2 (2 files).

22. **Generic functions named like primitive types** (`0022`) —
    `f32::<_, E>(Endianness::Big)`: `_expression`, `_path` and
    `macro_invocation` already alias the primitive-type names to `identifier`
    (see patch `0005`), but `generic_function` did not, so a turbofish call to a
    function named `f32`/`u8`/`str` failed. Found by the top-1000 sweep in
    nom 8.0.0's `tests/float.rs` (1 file).

23. **Contextual keyword as a capture name** (`0023`) — `raw @ (U8 | U16)`:
    `_pattern` already accepts `_reserved_identifier` (`default`, `union`,
    `gen`, `raw`) as a binding, but `captured_pattern` insisted on a bare
    `identifier`, so `raw` — a keyword only in `&raw const` — could not be
    the name of an `@` binding. Found by the top-1000 sweep in
    zerocopy-derive 0.8.55's `src/repr.rs` (1 file).

24. **Box patterns** (`0024`) — `box _f: Box<usize>`, `box 0 => {}`: the
    `box_patterns` feature is unstable but `box` is a reserved word, so
    `box PAT` can never be confused with an identifier. New `box_pattern`
    rule in `_pattern`, shaped like `ref_pattern`. Found by the top-1000
    sweep in enum_dispatch 0.3.13's `tests/arg_patterns.rs` (1 file).

25. **Empty trait bounds** (`0025`) — `where C: ,`: rustc accepts a bound
    list with no bounds, which macro-generated `where` clauses emit. The
    bound list in `trait_bounds` becomes `optional(...)`; that makes the end
    of `trait_bounds` ambiguous with the start of a lifetime bound, so
    `trait_bounds` is added to `conflicts` and GLR settles it. Found by the
    top-1000 sweep in combine 4.6.7's `src/stream/decoder.rs` (1 file).

26. **Trailing plus in trait bounds** (`0026`) — `trait N: ops::ShrAssign<i32> + {}`:
    rustc allows a bound list to end on `+`. Spelling that as
    `sepBy1('+', bound)` plus a trailing `optional('+')` regressed ~250 files
    (`where T: Clone + 'a` reduced `trait_bounds` at the `+` and then read
    `'a` as a label), so the list is written as `(bound '+')* bound?`
    instead: every `+` is shifted as a separator and the last bound is simply
    optional. Found by the top-1000 sweep in lexical-util 1.0.7's
    `src/num.rs` (1 file).

27. **Empty type arguments** (`0027`) — `&Thing<>`: rustc parses an empty
    generic argument list, so the argument list inside `type_arguments`
    becomes optional (the trailing-comma option moves inside it, so `<,>` is
    still rejected). Found by the top-1000 sweep in cxx 1.0.198's
    `tests/ui/self_lifetimes.rs` (1 file).
