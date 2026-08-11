# The Zig oracle, and which Zig it speaks for

`check.zig` answers one question per file — does Zig's own parser accept this
text — through `std.zig.Ast.parse(gpa, src, .zig)`, the exact call the
compiler, `zig fmt` and every language server make to turn a file into a
syntax tree. It resolves no `@import`, runs no `comptime`, links nothing: a
file is judged entirely on its own bytes.

Contract, shared with every other oracle under `tools/`: one path per line on
stdin, `"<path>\tvalid|invalid"` per line on stdout.

`explain.zig` takes the same input and emits the first parse error's tag and
line instead of a verdict. Tags rather than rendered messages on purpose —
rendering needs a `Writer` and the writer API is exactly what moved in
0.15/0.16, while the tag is stable and is what clusters.

## Why the version is half the answer

For every other language in this repository "is this file valid?" has one
answer. For Zig it has one answer *per compiler version*. A corpus scraped
from the wild holds files written against four different languages that all
call themselves Zig, so a gap number with no version attached means nothing.

That is not a worry, it is a measurement. Below is the measurement.

## Measured: 11,672 files, six toolchains

Corpus: the top 60 non-archived repositories GitHub classifies as Zig, by
stars, shallow-cloned at HEAD on 2026-08-11 — 11,672 `.zig` files. One
oracle *source*, built by six toolchains, over that identical file list.

| version | valid | invalid | valid % | vs. previous: gained | lost | files whose verdict moved |
|---|---:|---:|---:|---:|---:|---:|
| 0.11.0 | 10916 | 756 | 93.52% | — | — | — |
| 0.12.1 | 11350 | 322 | 97.24% | +436 | -2 | 438 (3.75%) |
| 0.13.0 | 11350 | 322 | 97.24% | +0 | -0 | 0 (0.00%) |
| 0.14.1 | 11446 | 226 | 98.06% | +99 | -3 | 102 (0.87%) |
| 0.15.2 | 11545 | 127 | 98.91% | +179 | -80 | 259 (2.22%) |
| 0.16.0 | 11547 | 125 | 98.93% | +2 | -0 | 2 (0.02%) |

801 files (6.86%) do not get the same verdict from all six. Across the four
current-era releases 0.13–0.16 it is 363 (3.11%).

**The instability is real but it is not diffuse.** Every file falls into one
of nine verdict signatures, and each has a single named cause:

| signature (0.11 → 0.16) | files | cause |
|---|---:|---|
| `V V V V V V` | 10831 | parses everywhere |
| `. V V V V V` | 436 | **destructuring assignment** `const a, const b = ...`, added in 0.12 |
| `. . . . V V` | 179 | two 0.15 changes: `async`/`await` **demoted from keywords to ordinary identifiers**, so `future.await(io)` and `group.async(...)` became legal (54); and `asm` clobbers became a struct literal, `::: .{ .memory = true }` (93) |
| `. . . V V V` | 99 | **labeled switch**, `sw: switch (x)` / `continue :sw v`, added in 0.14 |
| `V V V V . .` | 80 | **`usingnamespace` removed** in 0.15 |
| `. . . . . .` | 40 | invalid in every version — all of them `ziglang/zig`'s own `test/cases/compile_errors/` and `doc/langref/` fixtures, which are deliberately-invalid by construction |
| `V V V . . .` | 3 | 0.14 started rejecting a **tab inside a comment, doc comment or multiline string** |
| `. . . . . V` | 2 | 0.16 allows a **suffix operator on a labelled block**, `blk: { ... }[0..N]` |
| `V . . . . .` | 2 | `ziglang/zig` fixtures tracking their own era |

Two consequences worth stating plainly:

- **0.12 → 0.13 is a zero-file change.** The syntax did not move at all
  between those releases. "Zig changes every version" is not true at the
  granularity that matters here.
- **The cliff is 0.14 → 0.15** (259 files, 2.22%), and it is the only bump
  that *loses* files: `usingnamespace` is the one construct removed rather
  than added. Everywhere else the language grew and the newer parser is a
  superset of the older one.

## Why `Ast.parse` and not `zig ast-check`

`zig ast-check` runs AstGen, one stage past the parser, and is the obvious
analogue of the `compile()`-over-`ast.parse()` choice `py-oracle` made. It
was measured and rejected, for three reasons.

1. **AstGen enforces lint-grade rules, not well-formedness.** On 400
   parser-valid files it rejects 42. The reasons include `local variable is
   never mutated`, `unused function parameter`, `use of undeclared
   identifier`. A file with an unused parameter is unambiguously well-formed
   Zig that `zig fmt` round-trips; a tree-sitter grammar must accept it, and
   should. CPython's post-parse checks are different in kind — `return`
   outside a function is invalid in every program, in every version.
2. **It makes the version problem worse, not better.** 11 of those 42 are
   `invalid builtin function`, i.e. drift in the *builtin set* layered on top
   of drift in the grammar. The parser is the narrower, more stable surface.
3. **It hangs.** `zig ast-check` loops forever on `((1 + 1));` — measured on
   0.13.0, 0.14.1 and 0.15.2, fixed in 0.16.0. The file it hangs on is
   `ziglang/zig`'s own `test/cases/compile_errors/doubled_grouped_expr_as_stmt.zig`,
   whose comment reads "makes sure that the doubled grouped_expression does
   not cause an endless loop in AstGen". A single corpus file wedging the
   sweep is disqualifying on its own. `Ast.parse` returns cleanly on that
   file in all six versions.

## Cross-checked

Against an independent path through the same parser, `zig fmt --check`, on
0.16.0: of the 125 files the oracle calls invalid, `zig fmt` reports a parse
error on **125**. Of a 500-file sample the oracle calls valid, `zig fmt`
reports a parse error on **0**.

## Cost

0.14 s per 1000 files (1.51 s for 11,672), flat at ~56 MB RSS — the arena is
reset after each file. That is roughly 9× faster than `py-oracle` and firmly
Tier A. Older toolchains are slower but in the same class: 0.19 s per 1000 on
0.11.0.

## One source, six toolchains

`check.zig` compiles unmodified on 0.11.0 through 0.16.0. That is not
portability for its own sake — it is what makes a version bump *measurable*
instead of a leap: the same oracle source, built by two toolchains, over the
same corpus, produces the table above.

`Ast.parse`'s signature has not changed across those six releases. Everything
around it has, which is why the I/O is raw syscalls rather than `std.fs` and
`std.io` (both redesigned in 0.15/0.16) and the allocator is `page_allocator`
behind an arena rather than the GPA that was renamed. Two shims carry the
whole difference:

- `std.os` became `std.posix` in 0.12.
- `O` went from a namespace of integer constants to a packed struct in 0.12;
  `open`/`openZ` were dropped in 0.16 in favour of `openat`; `close` and
  `write` left `posix` in 0.16 and are reached one layer down at
  `posix.system`.

Reproduce with `./build.sh ~/opt/zig/*/zig`.
