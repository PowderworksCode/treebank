# Local patches — treebank-python

Upstream: [tree-sitter/tree-sitter-python](https://github.com/tree-sitter/tree-sitter-python)
pinned at `293fdc02038ee2bf0e2e206711b69c90ac0d413f` (v0.25.0).

Both patches here are packaging, not grammar. **The grammar itself is
unmodified**: the top-1000 PyPI sweep found no gap needing one, which is the
result recorded in `ledger.json`.

## 0001 — treebank redistribution notice

Prepends the standard warning to `README.md` so anyone who encounters a
materialized or published copy knows it is a generated redistribution and
where to report problems. Touches no grammar code, applies first.

## 0002 — treebank crate identity

Upstream owns `tree-sitter-python` on crates.io, so the redistribution
publishes as `treebank-grammar-python`, with treebank's repository, homepage
and description. `[lib] name` is pinned to `tree_sitter_python` so the crate
stays a drop-in replacement for upstream's, and `include` gains
`LOCAL-PATCHES.md`, `ledger.json` and `patches/*` so provenance travels
inside the published tarball. `Cargo.lock` gets the matching rename and
nothing else — dependency versions are upstream's.

The published version string is deliberately absent: `publish.sh` derives it
from crates.io at publish time. See `PUBLISHING.md`.

## 0003 — comment at column zero inside a block

The external scanner only treated `#` as starting a comment when `INDENT`,
`DEDENT`, `NEWLINE` or `EXCEPT` was a valid symbol. After a decorator the
parser expects only another decorator or a `def`/`class`, so none of those is
valid; the comment line was not consumed as a comment, and its column-0
indentation was taken as the line's own — dedenting out of the class body and
leaving the decorator in an `ERROR`.

```python
class C:
    @property
# a comment, unindented
    def f(self):
        return 1
```

CPython's tokenizer ignores comment-only lines entirely for indentation
purposes, at any column, so this is valid Python that the grammar rejected.
The fix adds `found_end_of_line` to that condition: once an end of line has
been seen, `#` always starts a comment line. The trailing-comment case the
condition was written for (`foo = bar # comment`, where no end of line has
been seen yet) is unchanged.

Found by the first top-500 PyPI sweep, in a pydantic mypy fixture. One file
in 70,423, with zero regressions — before and after were measured over the
identical corpus with only this patch removed and restored.

## 0004 — slices in a generic type application

`generic_type` took `$.type_parameter`, whose elements are `$.type`, and
`$.type` has no slice. Because `generic_type` carries `prec(1)` it also
shadowed the plain subscript reading, so an annotation like `int[:]` had no
path at all:

```python
a: int32_t[:] = f()
foo: Bar[:, :, :]
def to_chars(s) -> char_type[:]: ...
def compute(x: my_type[:, ::1]): ...
```

A slice inside a subscript is ordinary valid Python — `Subscript(Name,
Slice())` — and Cython's pure-Python mode leans on it for memoryviews.

`generic_type` now has its own argument list that admits `$.slice`, aliased
back to the `type_parameter` node name so the tree shape consumers query is
unchanged. The PEP 695 *declaration* sites (`def f[T]`, `class C[T]`) keep
the strict `$.type_parameter`: `def f[:](): ...` is still rejected.

7 files, all Cython. 19 → 12 gap files.

## 0005 — PEP 646 unpacking beyond a bare name

Two halves of one feature:

```python
def foo(*args: *(int or str)): ...
data[*(x := y)]
u = tuple[*Ts]
```

`splat_type` accepted only a bare `$.identifier`. It now also takes a
parenthesized expression or a generic. Listing alternatives rather than
widening to `$.type` is deliberate: widening wrapped the identifier case in
a `type` node and broke two upstream corpus tests, which is a tree-shape
change consumers' queries would feel.

Separately, `subscript` admitted only an expression or a slice, so `data[*x]`
had no path; it now also takes `$.list_splat`. `data[*]` and
`def f(*args: *): ...` are still rejected.

12 → 7 gap files.

## 0006 — starred expressions in bare tuples and targets

`$.list_splat` is not an `$.expression`, so an element of a bare tuple could
never be a starred expression:

```python
a = *[1],
*[], b = (1,)
```

`a = *b,` only *appeared* to work, because a bare name is also a valid
assignment-target pattern and the parser took the pattern branch. `a = *[1],`
had no path at all. `expression_list` now takes
`choice($.expression, $.list_splat)`, matching CPython's `star_expressions`.

Separately `list_splat_pattern` accepted only identifier/subscript/attribute,
so the legal target `*[], b = (1,)` failed; it now also takes `list_pattern`
and `tuple_pattern`, matching CPython's `target_with_star_atom`.

7 → 5 gap files.

## 0007 — backslash continuation while scanning indentation

A backslash-newline joins two physical lines into one logical line, so the
logical line's indentation is whatever was counted *before* the backslash;
the continuation line's leading whitespace is content, not indentation.

The scanner's indentation loop consumed the backslash and newline and then
kept adding to `indent_length`, so this was read as indent 12 rather than 4
and the block structure came out wrong:

```python
if True:
    \
        1
else:\
    2
```

The loop now stops counting once a continuation has been consumed, and only
when an end of line has already been seen — a backslash mid-line, which is
the ordinary `x = 1 + \` case, is untouched.

2 files, zero regressions. Ordinary continuations were regression-tested
separately: assignments, conditions, parameter lists, return expressions,
implicit string concatenation and attribute chains all still parse. 5 → 3
gap files.

## 0008 — f-string format specifier beginning with an equals sign

`=` is sign-aware zero padding, so a specifier starting with it is ordinary
formatting code:

```python
a = f"{v:=>10}"
b = f"{num:=.2Uf}"
```

The ordinary lexer takes `:=` — one token, longer than `:` — so the
specifier never started and the interpolation came back as an `ERROR`.

The fix adds an **external token** for that colon. The scanner emits it only
when `valid_symbols` says a format specifier may start here *and* the next
character is `=`, which is the sole case the ordinary lexer gets wrong.
`format_specifier` accepts either that token or a plain `':'`, so every other
specifier lexes exactly as before. No new scanner state, so
serialize/deserialize are untouched.

Three alternatives were measured and rejected:

- **Raising the colon's lexical precedence** fixes the same files, but
  tree-sitter merges tokens with the same lexeme so the bump is global. It
  broke `{1, x := 2, 3}` in a set literal — **+4 gap files**.
- **Restricting the f-string expression rules** so a top-level
  `named_expression` is unreachable does not fix it at all, and cascades into
  unresolved conflicts: f-string interpolations share lex states with set and
  dict literals.
- **Emitting the external colon for every `:`**, not just before `=`, steals
  the colons belonging to slices, dicts and lambdas in the same replacement
  field — **+6 gap files**.

3 → 2 gap files.

## Known gaps, not fixed

Two files remain — and they are **one bug**, not two. An earlier revision of
this section described them as separate problems and claimed the second had
no minimal repro. Both statements were wrong; the corrected account follows.

### The bug: a dedent emitted inside brackets

Python ignores indentation inside brackets, so a continuation line may sit at
any column, including below the enclosing block. Both remaining files do
exactly that, and both come back with three `ERROR`s:

```python
# ruff fmt_on_off/indent.py          # executing tests/test_main.py:466
def test():                          class C:
  a                                      def m(self):
  (b +                                       tester = 1
c                                            (tester
   )                                        .
                                            x
                                           ) = 4
```

The `executing` file was previously filed as "pathological backslash
continuations with no minimal repro". It does contain those, but they are not
what fails — the failing region is the parenthesised assignment target above,
and it reproduces in seven lines. The earlier search missed it because it
bisected *contiguous line windows* of the file: every window containing those
lines carries their leading indentation, which is an `IndentationError` at
module level, so the oracle rejected every candidate and the search reported
nothing. The method could not express the reduction, and its silence was read
as evidence.

### Why the existing guard cannot fix it

The scanner has a `within_brackets` guard, but it is a proxy: it asks the
parser whether a *closing* bracket is a valid next token. After `(b +` the
parser wants an operand, so `)` is not valid and the guard reads false.

Measured at the moment of decision, a legitimate dedent and this case are
**indistinguishable**:

| | |
|---|---|
| `def f():` / `  a` / `b` | `indent=0 cur=2 DEDENT=1 NEWLINE=0 within_brackets=0` |
| `(b +` / `c` | `indent=0 cur=2 DEDENT=1 NEWLINE=0 within_brackets=0` |

The parser genuinely requests a dedent in both. Nothing in `valid_symbols`
separates them, so no refinement of the heuristic can work — the scanner must
count brackets itself.

### Why counting them does not work either

Counting exactly requires the scanner to *emit* the brackets; merely watching
`lexer->lookahead` double-counts, because the scanner is invoked repeatedly at
one position. Three variants were built and measured, all reverted:

| variant | result |
|---|---|
| `'('`, `'['`, `'{'` external, depth counter, serialization | fixes both files, **breaks 33 of 123 corpus tests** |
| `'('` only | **breaks the same 33**, and fixes only the first file — the second still has 2 errors |
| `'('` only, skipped during error recovery | **still 33** |

The failures are not confined to error recovery: `Await expressions`, `Named
expressions`, `Yield expressions` and `Default Tuple Arguments` all break, so
making a bracket external changes ordinary parsing, not just recovery. And
since the paren-only variant does not even close the second file, exact paren
depth is not sufficient for this bug — bracket depth alone is not the whole
story.

This is an upstream change to how tree-sitter-python models indentation and
brackets, not a patch this series should carry. The right next step is an
issue against upstream with the seven-line repro and the `valid_symbols`
trace above.
