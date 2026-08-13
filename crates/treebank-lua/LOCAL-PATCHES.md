# Local patches — treebank-lua

Upstream:
[tree-sitter-grammars/tree-sitter-lua](https://github.com/tree-sitter-grammars/tree-sitter-lua)
pinned at `10fe0054734eec83049514ea2e718b2a56acd0c9` (v0.5.0).

Five patches: two packaging, one grammar fix taken from an open upstream PR
whose base commit is exactly the sha pinned above, and two of our own.

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

## 0004 — NUL byte inside a quoted string

The last gap file in the sweep corpus, and the correction of a claim this
document used to make.

```lua
x = "a<NUL>b"
```

That is three bytes between the quotes — `a`, a literal NUL, `b`. Lua reads
source as a length-delimited buffer, so an embedded NUL is ordinary string
content and `luac 5.4` accepts the file. The grammar ended the content token
at the NUL and the rest of the file came out `ERROR`
(`string_content > ERROR(ERROR)`, the sweep's cluster signature).

The cause is that tree-sitter's lexer reports `lookahead == 0` for **both** a
NUL byte and end-of-input, and `scan_quote_string_content` — the function
0003 introduced — loops on `lexer->lookahead != 0`. The fix is one condition:

```c
while (!lexer->eof(lexer) && lexer->lookahead != ending_char && lexer->lookahead != '\\') {
```

`lexer->eof(lexer)` is the question an **external** scanner can ask and an
internal one cannot: it is false at a NUL and true only at the real end of
input. So the string half of this was never a core limitation, which is what
this file said before — see "Why there is no 0005" for the half that is.

The EOF path is the thing that condition could plausibly break, so it was
tested rather than assumed: `x = "abc` and a file ending immediately after
the opening quote both still terminate and still report a MISSING closing
quote instead of hanging, and a string mixing escapes with a NUL still comes
out as `string_content` wrapping its `escape_sequence` children — the tree
shape consumers query is unchanged.

| | passed | failed | gaps | clusters |
|---|---|---|---|---|
| before | 1,589 | 9 | 1 | 9 |
| after | **1,590** | **8** | **0** | **8** |

Measured over the 1,598-file LuaRocks sweep corpus — not the 8,582-file fetch
0003's table is measured over; the two are different fetches and their pass
counts are not comparable. Zero gap files remain: all 8 remaining failures
are corpus noise. Upstream's corpus tests go 42/42 → 43/43, the 76 syntax
highlighting and 24 tag assertions are unchanged, and the 16-file negative
corpus is still fully rejected.

One trap this patch leaves behind for the next one. The corpus test carries a
real NUL byte, so **git now treats `test/corpus/expressions.txt` as binary**:
a plain `git -C build diff` prints `Bin 16222 -> 16689 bytes` and the test
silently does not reach the patch file. This patch was captured with
`git diff -a`, which keeps it a readable text patch carrying the raw byte;
`git apply` round-trips it, verified by materializing from scratch and
reading the NUL back out of the materialized file. The same byte makes
`patches/0004-*.patch` itself binary to git, so `git show` renders it as
`Bin` — read it with `cat -v` (or `git show --text`), and diff it the same
way if it ever needs editing.

## 0005 — NUL byte inside a long string or long comment

The other half of 0004, and the one patch here with **no sweep delta at
all** — which is stated plainly rather than dressed up.

```lua
x = [[a<NUL>b]]
--[==[c<NUL>d]==]
```

Both are valid to `luac 5.4`; both failed to parse. The cause is 0004's,
one function over: `scan_block_content` looped on `lexer->lookahead != 0`, so
a NUL ended the content token and the rest of the file became `ERROR`. One
condition fixes **both** constructs, because that function is the single
producer of `BLOCK_STRING_CONTENT` and `BLOCK_COMMENT_CONTENT`:

```c
while (!lexer->eof(lexer)) {
```

| | passed | failed | gaps | clusters |
|---|---|---|---|---|
| before | 1,590 | 8 | 0 | 8 |
| after | 1,590 | 8 | 0 | 8 |

Zero files moved. 0004's ledger entry named this case and deliberately left
it uncaptured for want of corpus evidence; it is captured now on the view
that the defect is one bug across three token types, and leaving two of them
broken is a worse invariant than a patch whose evidence is a repro and an
oracle verdict rather than a sweep delta.

The evidence that it is real, in place of that delta: `[[a<NUL>b]]`,
`--[[a<NUL>b]]`, `[=[a<NUL>b]]c]=]`, `[[a]<NUL>b]]` and `--[==[a<NUL>b]==]`
are all valid to `luac 5.4` and all now parse clean, having all failed
before. The EOF path is the risk this condition carries, so it was tested
from the other side too: an unterminated `[[`, an unterminated `--[[`, and a
file ending immediately after `[[` are each still **rejected**, and `luac
5.4` rejects all three as well (unfinished long string / long comment).
Corpus tests go 43/43 → 45/45, highlight and tag assertions are unchanged,
and the negative corpus is still fully rejected.

One thing found while reading and deliberately **not** changed: the sibling
loop in `scan_comment_content` is unreachable. `scanner->ending_char` is only
ever assigned `0` (by `reset_state`) or restored from a serialized buffer
that can only hold `0`, so the `ending_char == 0` branch above it always
wins. It is vestigial upstream state; deleting it is a separate change with
its own justification, not something to smuggle into a NUL fix.

## Why there is no 0006

The upstream grammar is chosen, not settled for. All three editors that ship
a Lua grammar pin **the same commit** this crate pins — nvim-treesitter,
Helix and Zed all sit on `10fe0054` (v0.5.0). It is also the only Lua grammar
with any current maintenance: the alternatives (`tjdevries/tree-sitter-lua`,
131 stars, last pushed 2024-10; `Azganoth/tree-sitter-lua`, 53 stars, 2022)
are dormant.

The sweep corpus has **no gap files left**: 1,590 of 1,598 parse, and the 8
failures are all invalid Lua that `luac 5.4` rejects too. Upstream at the
pinned sha, with only the two packaging patches applied, passes 1,588 of the
same 1,598 — and its 2 gap files are exactly the first-seen files of 0003 and
0004, which is the independent check that this patch series is what moves the
number.

What remains of the NUL story is the half nothing at this layer can reach. A
NUL is still fatal wherever the **internal** lexer owns the token — a line
comment (`-- a<NUL>b`), which is the shape of the openssl `root_ca.lua` gap
the larger 8,582-file fetch found. There the internal lexer sees codepoint 0,
has no `eof()` to consult, and ends the token; the only fix is moving line
comments into the scanner, which is not a minimal change and which no file in
the current corpus needs. Every token the **scanner** owns is now fixed —
0004 for quoted strings, 0005 for long strings and long comments — so the
remaining exposure is one construct the corpus does not contain.

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
