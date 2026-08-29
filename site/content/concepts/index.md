---
title: How it works
description: The machinery — tiers, oracles, sweeps, and why the evidence is committed.
order: 10
---

A grammar is a claim about a language. Treebank's whole design is about making
that claim expensive to make and cheap to check.

Three things follow from taking it seriously. The **vocabulary** has to be
split by what the parse table can actually enforce, not by what would read
nicely. The **oracle** has to be the language's own toolchain, because
anything else is a second opinion from a worse parser. And the **evidence**
has to be committed next to the grammar, because a number in a README is a
number nobody can reproduce.
