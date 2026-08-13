# Local patches — treebank-erlang

Upstream:
[WhatsApp/tree-sitter-erlang](https://github.com/WhatsApp/tree-sitter-erlang)
pinned at `6ba4c762eb3065495e3db85697ffeecdf364ce35` (0.20).

Three patches: two packaging, and one grammar fix found by the first Hex
sweep and not reported upstream.

## Which upstream, since the editors disagree

nvim-treesitter and Zed ship `WhatsApp/tree-sitter-erlang`; Helix ships
`the-mikedavis/tree-sitter-erlang`. WhatsApp's is the grammar — 97 stars
against 11, pushed within the fortnight against ten months ago, and the crate
on crates.io is built from it. It is also the parser inside WhatsApp's own
Erlang language server, which means someone runs it over a very large private
corpus every day.

The pin is nvim's commit rather than the `0.20` tag. `grammar.js` and
`src/scanner.c` are identical at both — and at Zed's pin two months earlier —
but the tag predates upstream adding `tree-sitter.json`, without which the
pinned CLI cannot run the highlight half of `tree-sitter test`. Same parser,
working test suite.

## 0001 — treebank redistribution notice

The standard warning at the top of `README.md`. Touches no grammar code.

## 0002 — treebank crate identity

Publishes as `treebank-grammar-erlang` with treebank's repository, homepage
and description; `[lib] name` pinned to `tree_sitter_erlang` so the crate
stays a drop-in replacement, since upstream declares no `[lib] name` and
relies on the package name. `include` gains `LOCAL-PATCHES.md`,
`ledger.json`, `patches/*` and `tree-sitter.json`.

Unlike elixir there is no `NOTICE` to rescue: this grammar is MIT with a
single `LICENSE`, already in upstream's `include`.

## 0003 — macro bodies that are not expressions or clauses

```erlang
-define(WITH_STACKTRACE(T, R, S), T:R:S ->).
```

Valid Erlang — it is the standard OTP-21 stacktrace-compatibility idiom, and
`telemetry` and `epgsql` both ship it — and the grammar rejected it.

**A `-define` body only has to make sense where it is *expanded*.** Upstream
already accepts that: `_macro_def_replacement` allows six shapes, including
loose function clauses and bare guard sequences. The corpus simply contains
four more, so this patch extends the same list rather than introducing a new
idea:

| shape | example | from |
|---|---|---|
| `replacement_clause_head` | `T:R:S ->` | telemetry, epgsql |
| `replacement_leading_colon` | `:Var` | brod |
| `replacement_partial_clauses` | `T:R -> S = erlang:get_stacktrace(), ` | epgsql |
| bare `record_name` | `#decode_opt_v2` | jsone |

`replacement_partial_clauses` is spelled out instead of reusing
`clause_body`, which is `prec.right` and swallows the trailing comma while
waiting for another expression. A first cut of the patch needed a declared
conflict between `_cr_clause_or_macro` and `_macro_body_expr`; spelling the
rule out removed the ambiguity, and the CLI confirms no conflict is needed.

**This patch exists only because the oracle is a union.** Every one of these
files is rejected by `epp_dodger`, for exactly the reason the grammar
rejected them — a macro body that is not a form. Under the single-parser
oracle the roadmap specified, all eleven would have been scored *invalid*,
booked as corpus noise, and the gap would have been invisible. The measured
cost of that mistake would have been 6.1% of the corpus. See `ledger.json`'s
`oracle_union_measured`.

Evidence: 6 files fixed, zero regressions — corpus tests 229 → 230, highlight
parses unchanged at 228/228, negative corpus still fully rejected, and every
gap cluster whose signature began `pp_define > ERROR` is gone.

## What is left, and why it is not a grammar patch

16 gap files remain, and they are one family: the macro **expansion** site,
where this patch fixed the **definition** site. `?CATCH(_, _, _)` standing in
for a try clause, `?OPT{undefined_as_null = true}` where a macro supplies a
record name, `?CAPTURE_STACKTRACE->` inside a catch clause. Those files'
*text* is not a parse tree and only becomes one after expansion — the Erlang
analogue of C's conditional compilation, which this repo models with
`treebank_preprocessing` rather than with grammar rules. That is the right
next question for this grammar, and it is a different question from
`grammar.js`.
