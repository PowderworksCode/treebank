---
title: Oracles
description: Why the reference parser is the language's own toolchain, and what that costs.
order: 20
---

When treebank and a grammar disagree about whether a file is valid, something
has to adjudicate. That something is the language's own toolchain — CPython
for Python, `javac` for Java, `clang` for C and C++, `ruby -c`, `zig ast-check`,
`tsc`, `rustc`, `bash -n`.

The alternative — a second parser written for the purpose — is a second
opinion from a worse parser, and when it disagrees you have learned nothing.

This costs more than it sounds. The oracle has to be installed, pinned, and
run over the corpus, and some languages need **more than one**: Python's
corpus contains code written for Python 2 and code written for Python 3, and
a single oracle would mislabel one era's syntax as noise. Zig is the same
across its 0.11 and 0.16 syntax. Those legacy legs are mandatory, not
optional — omitting one makes a broken sweep look healthier than it is.

An oracle answers one question, `valid` or `invalid`, and `treebank oracle`
exposes exactly that call and nothing else. It is the same call `sweep` uses
to adjudicate its failures.

Some oracles can do more. A **span oracle** reports node boundaries, which is
what `treebank shape` compares against; a **formatter** is what `reformat` and
`roundtrip` use. Not every toolchain offers them: Zig exposes formatting and a
verdict but no supported tree dump, so it has no span oracle, and that absence
is recorded rather than worked around.
