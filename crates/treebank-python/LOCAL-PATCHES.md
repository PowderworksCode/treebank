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

## Known gaps, not fixed

Five files remain, in two families. Both live in the external scanner and the
lexer rather than in `grammar.js`, neither reproduces in a small isolated
snippet — both need the surrounding indentation state — and the blast radius
of a change there is every Python file. Left for a focused pass:

1. **An f-string format specifier beginning with `=`** (1 file):
   `f"{num:=.2Uf}"`, `f"{v:=>10}"`. `:=` is lexed as the walrus operator
   before `format_specifier`'s `:` is considered.
2. **Backslash continuations interacting with indentation** (4 files):
   `else:\` followed by an indented body, and an attribute access split as
   `tester.x. \` across lines. All four are parser/formatter test fixtures
   from ruff and executing.
