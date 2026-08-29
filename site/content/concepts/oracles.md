---
title: Reference parsers
description: The language's own toolchain decides who is right.
order: 20
---

When Treebank and a grammar disagree about whether a file is valid, the
language's own toolchain settles it: CPython for Python, `javac` for Java,
`clang` for C and C++, `ruby -c`, `zig ast-check`, `tsc`, `rustc`, `bash -n`.

Anything else would be a second parser's opinion, and a disagreement between
two parsers tells you nothing about which is right.

## What this costs

Each oracle has to be installed, pinned to a version, and run over the whole
corpus. Some languages need more than one. Python's corpus contains code
written for Python 2 and code written for Python 3, and a single oracle would
report one era's syntax as broken. Zig is the same across 0.11 and 0.16.

## What an oracle can be asked

Every oracle answers one question: is this file valid? `treebank oracle`
exposes exactly that and nothing more.

Some can do more, and where they can, more is measured:

- a **span oracle** reports node boundaries, which `treebank shape` compares
  against ours
- a **formatter** or **printer** rewrites the file, which `treebank reformat`
  and `treebank roundtrip` use to check the tree does not move

Zig has no span oracle — its toolchain exposes formatting and a verdict, but
no supported tree dump — so it does not run the shape check. That is recorded
on its page rather than worked around.
