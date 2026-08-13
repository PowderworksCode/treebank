# Local patches — treebank-toml

Upstream:
[tree-sitter-grammars/tree-sitter-toml](https://github.com/tree-sitter-grammars/tree-sitter-toml)
pinned at `64b56832c2cffe41758f28e05c756a3a98d16f41` (v0.7.0).

Six patches: two packaging, three bringing the grammar to **TOML 1.1.0**, and
one plain defect fix. After them the grammar accepts all 220 files
`toml-test`'s 1.1.0 manifest calls valid (220/220) and the corpus sweep's
`gap_files` is 0.

## 0001 — treebank redistribution notice

Prepends the standard warning to `README.md` so anyone who encounters a
materialized or published copy knows it is a generated redistribution and
where to report problems. Touches no grammar code, applies first.

## 0002 — treebank crate identity

Upstream publishes as `tree-sitter-toml-ng` — note the suffix; it does *not*
own `tree-sitter-toml` on crates.io, which belongs to a third party. The
redistribution publishes as `treebank-grammar-toml`, with treebank's
`repository`, `homepage` and `description`, and `include` extended so
`ledger.json`, `LOCAL-PATCHES.md` and `patches/` travel inside the published
tarball.

`[lib] name` is pinned to `tree_sitter_toml_ng` so the crate stays a drop-in
replacement. This matters more here than for most grammars: upstream declares
no `[lib] name` at all, so cargo derives it from the package name — renaming
the package would silently rename the library and break every
`use tree_sitter_toml_ng::LANGUAGE`.

## 0003 — seconds are optional in times

TOML 1.1.0 makes the seconds field optional in local-time, local-date-time
and offset-date-time. `rfc3339_time` required it. Repro: `lt3 = 07:32`.
Fixes 4 `toml-test` files. Guarded by five negative tests for malformed
times, so the optional group cannot swallow `07:6` or `25:00`.

## 0004 — `\e` and `\x` escape sequences

TOML 1.1.0 adds `\e` (U+001B) and `\xHH` byte escapes to basic strings.
Added to the `escape_sequence` token beside the existing `\u` and `\U`
forms. Repro: `esc = "\e"`, `hex = "\x7f"`. Fixes 3 `toml-test` files, and
is the construct the corpus actually contained — `basic-toml`'s
`tests/invalid/string-byte-escapes.toml`. Guarded by four negative tests.

## 0005 — newlines and trailing commas in inline tables

TOML 1.1.0 permits newlines inside inline tables and a trailing comma. The
rule is restructured into the same shape the `array` rule already used for
exactly this, so the two composite values now handle interior whitespace
identically rather than differently. Fixes 4 `toml-test` files. Guarded by
four negative tests.

## 0006 — line-ending escape followed by whitespace

**Not a 1.1.0 change** — a plain defect, and the only one of the four
invalid under *both* revisions. `_escape_line_ending` required the newline
immediately after the backslash, but TOML says the line-ending backslash may
be followed by whitespace: "when the last non-whitespace character on a line
is an unescaped `\`, it will be trimmed along with all whitespace up to the
next non-whitespace character."

Bisected to this single rule. A bare backslash-newline already worked, and
the empty `""""""` form already worked; only trailing whitespace before the
newline failed. Repro is a backslash, two spaces, then a newline inside a
multi-line basic string. Fixes 3 `toml-test` files. Guarded by a negative
test for non-whitespace after the escape, and `basic-toml`'s own
`tests/invalid/string-bad-line-ending-escape.toml` stays correctly rejected.
