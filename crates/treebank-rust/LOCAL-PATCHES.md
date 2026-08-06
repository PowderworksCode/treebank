# treebank-rust

Vendored [tree-sitter/tree-sitter-rust](https://github.com/tree-sitter/tree-sitter-rust)
at **0.24.2** (`77a3747266f4d621d0757825e6b11edcbf991ca5`) with the patch
series below applied. Contract, reconstruction invariant, CLI pin rationale,
and workflow: see [GRAMMARS.md](../../GRAMMARS.md) at the repo root.

## Patches

1. **Extern types** (`0001`) — `pub type Foo;` inside `extern` blocks (port of
   upstream PR [#281](https://github.com/tree-sitter/tree-sitter-rust/pull/281)):
   `associated_type` accepts an optional visibility modifier. First seen in
   wasm-bindgen/web-sys generated bindings.

2. **`~` in token trees** (`0002`) — added `~` to
   `TOKEN_TREE_NON_SPECIAL_PUNCTUATION`. Still a valid rustc lexer token and
   used in macro matchers in the wild (anyhow's `ensure!` fuel counters,
   serde_json's ui tests). Unreported upstream as of 2026-08.

3. **Immediate lifetime/label identifiers** (`0003`) — `lifetime` and `label`
   use `token.immediate` after `'`. rustc lexes `'a` as one token, so `' a` is
   not valid Rust; upstream accepted it. Strictness patch: the grammar should
   reject what the reference rejects (see `test/negative/`).

4. **Macros named like primitive types** (`0004`) — `str!["hi"]`, `u32![1]`:
   `macro_invocation` now accepts primitive-type names as the macro name
   (aliased to `identifier`, mirroring `_path`). Found by the top-100 sweep in
   winnow's snapbox `str!` snapshot tests (5 files).

5. **Negative literals in const generic arguments** (`0005`) —
   `ri8<-25, 25>`: `type_arguments` accepts `negative_literal`, mirroring
   `const_parameter` defaults. Found by the top-100 sweep in time 0.3.55's
   deranged ranged-integer type aliases (3 files).

6. **`try!` macro invocation** (`0006`) — `try!(g())`: `try` is only a
   keyword in edition 2018+; as a macro name it is valid 2015-edition code
   and unambiguous (a `try_block` needs `{`). `macro_invocation` accepts
   `try` aliased to `identifier`. Found by the top-100 sweep in autocfg
   1.5.1 (2 files).

7. **Bare `$` in macro token trees** (`0007`) — `($mode:ident, $) => { 1 }`:
   a `$` not starting a metavariable or repetition is a literal token in
   matchers and transcribers. Added `prec(-1, '$')` to `_token_pattern` and
   `_tokens`; the low precedence keeps `$(...)*` parsing as a repetition.
   Found by the top-100 sweep in syn 3.0.3's punctuation macros (2 files).

8. **Unit type in where predicates** (`0008`) — `where (): Target<V>`:
   `where_predicate` accepted `tuple_type` but not `unit_type`, so `()` on
   the left of a bound failed. Found by the top-100 sweep in time 0.3.55 and
   typenum 1.20.1 (2 files).

9. **`safe fn` in unsafe extern blocks** (`0009`) — RFC 3484:
   `unsafe extern "C" { safe fn f() -> u64; }`. The `safe` qualifier is only
   reachable through an `unsafe extern` block body (`_extern_declaration_list`),
   never `function_modifiers` — plain `extern` blocks keep rejecting it
   (`test/negative/safe-in-plain-extern.rs`). Fix shape comes from the
   adversarial review parked at `corpus/rust/reports/parked-safe-fn-analysis/`,
   whose first draft leaked `safe` into plain extern blocks. Found by the
   top-100 sweep in getrandom 0.4.3's WASI backend (1 file).

10. **Attributes on struct pattern fields** (`0010`) —
    `S { #[cfg(x)] inner: a, .. }`: `struct_pattern` fields accept leading
    `attribute_item`s, same idiom as `field_declaration_list`. Found by the
    top-100 sweep in proc-macro2 1.0.107's fallback.rs (1 file).

11. **Unit struct with where clause** (`0011`) —
    `struct _Test where Error: Send + Sync;`: the unit-struct alternative of
    `struct_item` now takes an optional `where_clause` before `;`. Found by
    the top-100 sweep in syn 3.0.3's error.rs (1 file).

12. **Underscore separators in unicode escapes** (`0012`) — `"\u{4_e}"`,
    `'\u{4_e}'`: underscores are digit separators inside `\u{...}` (the
    first character must still be a hex digit). Applied to both
    `escape_sequence` and `char_literal`. Found by the top-100 sweep in
    time 0.3.55's macro tests (1 file).
