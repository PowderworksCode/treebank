# Local patches — treebank-elixir

Upstream:
[elixir-lang/tree-sitter-elixir](https://github.com/elixir-lang/tree-sitter-elixir)
pinned at `e2d9e6e0e76b0c436fa48a0b8c32a031d0cbdf49` (v0.3.5).

Three patches: two packaging, and one grammar fix found by the first Hex
sweep and not previously reported upstream.

## 0001 — treebank redistribution notice

Prepends the standard warning to `README.md` so anyone who encounters a
materialized or published copy knows it is a generated redistribution and
where to report problems. Touches no grammar code, applies first.

## 0002 — treebank crate identity

Upstream owns `tree-sitter-elixir` on crates.io, so the redistribution
publishes as `treebank-grammar-elixir`, with treebank's repository, homepage
and description. `[lib] name` is pinned to `tree_sitter_elixir` so the crate
stays a drop-in replacement for upstream's — upstream has no `[lib] name` at
all and relies on the package name, so renaming the package without this
would rename the library too and break every consumer's `use`. `include`
gains `LOCAL-PATCHES.md`, `ledger.json` and `patches/*` so provenance travels
inside the published tarball. Upstream ships no `Cargo.lock`, so unlike lua
this patch touches `Cargo.toml` alone.

This patch also fixes an upstream packaging bug in passing. `include` lists
`LICENSE` but not **`NOTICE`** — and tree-sitter-elixir is Apache-2.0, whose
section 4(d) requires every redistribution to carry the NOTICE file. That
file is not decorative here: it is where the MIT terms for the
tree-sitter-generated files in `src/` (Max Brunsfeld) and for the
`test/corpus` fragments (Anantha Kumaran) are stated, so the crate published
on crates.io today ships neither. A redistribution has to carry its licence
notices, so `NOTICE` is added here rather than left for upstream. Like lua's
`LICENSE`/`LICENSE.md` correction it is a one-line change that would apply
cleanly upstream and is worth offering as a standalone PR. `tree-sitter.json`
is added for the same reason — it carries the grammar's own version and
file-type metadata and upstream omits it.

The published version string is deliberately absent: `publish.sh` derives it
from crates.io at publish time. See `PUBLISHING.md`.

## 0003 — backslash pair before a non-interpolating end delimiter

Found by the first Hex sweep. **No upstream issue or PR covers it** (searched
`elixir-lang/tree-sitter-elixir` for backslash/escape/sigil), so this one is
ours to offer.

```elixir
x = ~S(\\)
```

Ten characters, valid Elixir, and the grammar rejected it. Real code hits it
wherever a backslash has to be escaped without interpolation getting in the
way — the corpus instance is
`telemetry_metrics_prometheus_core/lib/core/exporter.ex`, escaping
backslashes for Prometheus exposition format:

```elixir
|> String.replace(~S(\\), ~S(\\\\))
```

**Diagnosis.** In `scan_quoted_content` the scanner returns — ending the
content token — as soon as it sees a backslash *whose next character is the
end delimiter*, so the grammar's `escape_sequence` rule can consume the
escaped delimiter. That is the right behaviour for `~S(\)`, which really is
an unterminated sigil. But it never accounts for a backslash that was itself
escaped: in `~S(\\)` the scanner consumes the first `\`, loops, sees the
second `\` with `)` behind it, and stops — so the grammar reads `\)` as an
escaped delimiter and the sigil never closes.

The fix is three lines in `src/scanner.c`: when a backslash is followed by
another backslash in a *non-interpolating* form, consume the pair, so the
second one cannot begin a new escape.

**Why only non-interpolating forms are affected.** For interpolating strings
and sigils (`~s`, `~r`, `"…"`) the scanner returns after *any* backslash and
the grammar's full `escape_sequence` rule consumes `\\` as one token, so the
pair is already handled. `~s(\\)` always parsed; `~S(\\)` never did.

**The boundary is what makes this safe**, and it is where the patch could
have gone wrong. An *odd* number of backslashes before the delimiter really
does escape it, and must still fail:

| | Elixir | grammar before | grammar after |
|---|---|---|---|
| `~S(\)` | invalid | rejects | rejects |
| `~S(\\)` | valid | **rejects** | parses |
| `~S(\\\)` | invalid | rejects | rejects |
| `~S(\\\\)` | valid | **rejects** | parses |

Verified for all eight delimiters (`( ) [ ] { } < > \| / " '`), for the
interpolating forms that must not change, and for heredocs, which take a
different branch (`\` before a newline is deliberately ignored so the end
delimiter can be recognised) and were unaffected either way. `~S(\\\)` is in
`test/negative/escaped-end-delimiter-sigil.ex` and in the consumer test's
`must-reject.ex` precisely so a future "improvement" cannot quietly widen the
fix onto the odd case.

Semantics were checked against the real tokenizer rather than assumed:
`~S(\\)` evaluates to a two-character string `\\`, so both backslashes are
content and neither is dropped — which is exactly what consuming the pair
produces.

Evidence: one file in 9,423 (the sweep's only gap), zero regressions —
upstream's corpus tests go 302 → 303 with the new case and nothing else
moves, the 23-file negative corpus is still fully rejected, and the sweep's
noise count is unchanged.
