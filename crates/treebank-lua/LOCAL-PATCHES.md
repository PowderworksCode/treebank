# Local patches — treebank-lua

Upstream:
[tree-sitter-grammars/tree-sitter-lua](https://github.com/tree-sitter-grammars/tree-sitter-lua)
pinned at `10fe0054734eec83049514ea2e718b2a56acd0c9` (v0.5.0).

Both patches here are packaging, not grammar. **The grammar itself is
unmodified**, which is the measured result recorded in `ledger.json` rather
than an assumption — see *The dialect question* below for what was checked.

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

## Why there is no 0003

The upstream grammar is chosen, not settled for. All three editors that ship
a Lua grammar pin **the same commit** this crate pins — nvim-treesitter,
Helix and Zed all sit on `10fe0054` (v0.5.0). It is also the only Lua grammar
with any current maintenance: the alternatives (`tjdevries/tree-sitter-lua`,
131 stars, last pushed 2024-10; `Azganoth/tree-sitter-lua`, 53 stars, 2022)
are dormant.

Measured on the top-500 LuaRocks corpus — 383 packages, 8,582 `.lua` files:

| | files |
|---|---|
| passed | **8,545** (99.57%) |
| failed | 37 |
| — grammar gaps | **3** |
| — corpus noise | 34 |

The three gaps fall into two classes, and **neither is a `grammar.js` bug**,
which is why no patch ships. Both are diagnosed to a minimal repro rather
than left as a cluster signature.

**1. A quoted string whose content begins with a long-comment opener.**
Repro, ten characters, valid in 5.1/5.4/LuaJIT and rejected by the grammar:

```lua
x = "--[["
```

`comment` is in `extras`, so the external scanner's `_block_comment_start`
stays valid at the start of a string's content; `scan_comment_start` eats
`--` and then `[[` before the content token is tried. It is specific to that
position and to the *external* token — `x = "a--[[b"`, `x = "-- [["` and
`x = "--foo"` all parse, the last of which shows the *internal* comment extra
is correctly not valid there.

Attempted and reverted: making the closing quote `token.immediate`, so the
state after the opening quote admits no extras, is the obvious fix and does
not work — confirmed in the generated `grammar.json`, the gap survives. A
real fix is scanner-side and needs an understanding of why an external extra
outranks immediacy where an internal one does not. Shipping a
half-understood `scanner.c` patch is exactly the accepts-invalid-code drift
`GRAMMARS.md` warns about, so nothing was shipped. The repro is ten
characters and is worth reporting upstream.

**2. Files containing a literal NUL byte** (2 files: net-url's
`query_test.lua`, one inside a string; openssl's `root_ca.lua`, one inside a
comment). Lua reads source as a length-delimited buffer and accepts embedded
NULs; tree-sitter's lexer reserves codepoint 0 for EOF, so the token ends
there and the rest of the file is `ERROR`. Minimal: `x = "a<NUL>b"` is valid
to `luac 5.4` and fails to parse. This is a **tree-sitter core limitation**,
not something `grammar.js` or `scanner.c` can express, and it is recorded as
such rather than counted as a grammar gap it is not. Incidence: 2 of 8,582
files (0.02%).

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
