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
