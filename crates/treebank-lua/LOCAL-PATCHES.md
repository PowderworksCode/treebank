# Local patches — treebank-lua

Upstream:
[tree-sitter-grammars/tree-sitter-lua](https://github.com/tree-sitter-grammars/tree-sitter-lua)
pinned at `10fe0054734eec83049514ea2e718b2a56acd0c9` (v0.5.0).

Three patches: two packaging, and one grammar fix taken from an open
upstream PR whose base commit is exactly the sha pinned above.

## 0001 — treebank redistribution notice

Prepends the standard warning to `README.md` so anyone who encounters a
materialized or published copy knows it is a generated redistribution and
where to report problems. Touches no grammar code, applies first.

## 0002 — treebank crate identity

Upstream owns `tree-sitter-lua` on crates.io, so the redistribution publishes
as `treebank-grammar-lua`, with treebank's repository, homepage and
description. `[lib] name` is pinned to `tree_sitter_lua` so the crate stays a
drop-in replacement for upstream's, and `include` gains `LOCAL-PATCHES.md`,
`ledger.json` and `patches/*` so provenance travels inside the published
tarball. `Cargo.lock` gets the matching rename and nothing else — dependency
versions are upstream's.

This patch also corrects one upstream packaging bug in passing: `include`
listed `/LICENSE`, but the file in the tree is `LICENSE.md`, so **the MIT
licence text never reached the published tarball at all**. A redistribution
has to ship its licence, so it is fixed here. It is a one-line change that
would apply cleanly upstream and is worth offering as a standalone PR.

The published version string is deliberately absent: `publish.sh` derives it
from crates.io at publish time. See `PUBLISHING.md`.

## 0003 — long comment opener at the start of a quoted string

Taken from **[upstream PR #80](https://github.com/tree-sitter-grammars/tree-sitter-lua/pull/80)**
(open, by `notpeter`, opened against
[zed-extensions/lua#54](https://github.com/zed-extensions/lua/issues/54)).
Found independently by this crate's first LuaRocks sweep and diagnosed to the
same repro before the PR was discovered. **Retire this patch when it merges.**

```lua
x = "--[["
```

Ten characters, valid in 5.1/5.4/LuaJIT, and the grammar rejected it.
`comment` is in `extras`, so the external scanner's `_block_comment_start`
stays valid at the *start* of string content; `scan_comment_start` eats `--`
and then `[[`, and the string's content is lexed as the beginning of a long
comment. It is specific to that position and to the *external* token —
`x = "a--[[b"`, `x = "-- [["` and `x = "--foo"` all parsed before the fix,
and the last of those is the tell: the *internal* comment extra is correctly
not valid there.

The fix moves quoted-string content into the scanner as two new external
tokens, checked *before* the comment branch, so the contest is settled by
scanner order rather than by token immediacy.

Recorded because it cost time: the obvious grammar-side fix — make the
closing quote `token.immediate`, so the state after the opening quote admits
no extras — was tried first, verified to have reached the generated
`grammar.json`, and **does not work**. An internal token loses to any valid
external one regardless of immediacy, which is exactly why the fix has to be
scanner-side.

Only `grammar.js`, `src/scanner.c` and the two test files are carried. The
PR's regenerated `src/parser.c`, `src/grammar.json` and
`src/tree_sitter/array.h` are not: `materialize.sh` regenerates those with
the pinned CLI, and committing a patch to generated files would defeat the
point.

| | passed | failed | gaps | clusters |
|---|---|---|---|---|
| before | 8,545 | 37 | 3 | 31 |
| after | **8,546** | **36** | **2** | **28** |

One file in 8,582 and zero regressions, measured over the identical corpus
with only this patch removed and restored. Because the patch changes how
*every* quoted string is lexed, it was regression-tested well past the sweep:

- the 2,606-file external corpus (neovim, nvim-treesitter, awesome,
  lua-resty-core, koreader, luvit) fails exactly the same 3 files as before,
  and no others;
- upstream's corpus tests stay 42/42, with highlight assertions rising
  58 → 76;
- the 16-file negative corpus is still fully rejected;
- the oracle's 20-file adversarial battery still agrees 20/20;
- the **tree shape is unchanged** — `string_content` still wraps
  `escape_sequence` children, which is what consumers' queries depend on and
  the thing a lexer change breaks most easily.

## Why there is no 0004

The upstream grammar is chosen, not settled for. All three editors that ship
a Lua grammar pin **the same commit** this crate pins — nvim-treesitter,
Helix and Zed all sit on `10fe0054` (v0.5.0). It is also the only Lua grammar
with any current maintenance: the alternatives (`tjdevries/tree-sitter-lua`,
131 stars, last pushed 2024-10; `Azganoth/tree-sitter-lua`, 53 stars, 2022)
are dormant.

After 0003, **2 gap files remain out of 8,582 (99.58% pass)**, and both are
the same cause: **a literal NUL byte**, one inside a string literal
(net-url's `query_test.lua`) and one inside a comment (openssl's
`root_ca.lua`). Lua reads source as a length-delimited buffer and accepts
embedded NULs; tree-sitter's lexer reserves codepoint 0 for EOF, so the token
ends there and the rest of the file is `ERROR`. Minimal: `x = "a<NUL>b"` is
valid to `luac 5.4` and fails to parse.

This is a **tree-sitter core limitation**, not something `grammar.js` or
`scanner.c` can express, and it is recorded as such rather than counted as a
grammar gap it is not. Note that 0003 does not help here and was never going
to: its `scan_quote_string_content` loop also terminates on `0`.

## The dialect question, and why it is settled the way it is

Lua is the language where "which reference parser?" is not a detail. `goto`
is 5.2+, integer division and bitwise operators are 5.3+, `<const>` and
`<close>` are 5.4, LuaJIT adds 64-bit (`1LL`) and binary (`0b1010`) literals
that no PUC Lua accepts, and Luau is a different language again. So the
oracle's version is recorded in `ledger.json`'s `oracle` field, and
`tools/lua-oracle/check.lua` refuses to run under any other one.

That this matters was measured, not assumed. Running the same 2,606 files
through three interpreters:

| Reference parser | valid | invalid |
|---|---|---|
| Lua 5.1.5 | 2,597 | 9 |
| **Lua 5.4.6 (pinned)** | **2,601** | **5** |
| LuaJIT 2.1 | 2,603 | 3 |

Six files change verdict depending purely on which `lua` is installed — four
koreader files using the `goto continue` idiom (5.2+, which LuaJIT also has)
and two neovim files using LuaJIT's `-1ULL`. That is 0.23% of the corpus
deciding to be valid or invalid based on an unrecorded toolchain choice,
which is exactly the failure the `oracle` field exists to prevent.

**The grammar's own language is the union.** Upstream's README says "Lua 5.x,
LuaJIT 2.x" and `grammar.js` backs it: the number rule carries `U?LL` and a
`0b` binary form no PUC Lua accepts. Measured, the grammar agrees with LuaJIT
on all 2,606 files exactly — the same three rejects, no disagreement in
either direction.

PUC 5.4 is pinned anyway, for three reasons. It is the standard packaged
interpreter (`apt install lua5.4`) rather than a source build. LuaRocks, the
corpus, is overwhelmingly portable code — 4,412 of 5,360 rocks (82%) declare
compatibility with both 5.1 and 5.4. And the resulting asymmetry runs the
safe way: the grammar accepts a superset of what the oracle calls valid, so
the oracle **can never manufacture a gap**, only miss one. The measured cost
of that choice on a deliberately LuaJIT-heavy sample was two files in 2,606.

If a later sweep shows that cost growing, the upgrade is a union oracle —
valid if any of 5.1/5.4/LuaJIT accepts it, which is precisely the language
the grammar targets. It is not built now because on the evidence it would
change two files in 2,606 and would make every CI machine build two extra
interpreters from source.

### What this means for `test/negative/`

The negative corpus must not contain LuaJIT literals, Luau syntax, or
`local goto = 1` (valid 5.1, rejected by 5.4). The grammar accepts those by
design and should; a negative test built from them would be testing the
dialect gap rather than the grammar. The 16 files in `test/negative/` are
therefore restricted to syntax invalid in **every** Lua dialect, and that is
verified rather than asserted: each one is rejected by Lua 5.1.5, Lua 5.4.6
and LuaJIT 2.1, and by the grammar.
