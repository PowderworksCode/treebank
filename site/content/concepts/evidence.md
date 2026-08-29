---
title: Sweeps and evidence
description: Why the numbers are committed to the repository instead of reported.
order: 30
---

A sweep parses a corpus of real code and adjudicates every failure against the
oracle. What comes back is a pass rate — and a pass rate published in a README
is a number nobody can reproduce.

So the evidence is committed. Each language has a **lock** pinning the exact
corpus: which packages, which versions, which files. `treebank hydrate`
recreates and verifies that corpus from the lock, so a sweep run today and a
sweep run next year are measuring the same thing. The **ledger** records what
was measured, against which grammar revision, with which oracle.

Two consequences worth stating plainly.

**Regenerating the evidence is mechanical.** Nothing in a sweep block is
hand-written, and hand-editing one is how a repository starts lying about
itself. If a number needs to change, the sweep is re-run.

**The ledger is bound to a grammar revision.** Change the grammar and the
evidence is stale until it is re-run — which `treebank status --check`
reports, so staleness is visible rather than assumed away.

## What the corpus is

The top packages of each language's ecosystem, ranked and fetched rather than
curated. `treebank rank` builds the list, `fetch` downloads and extracts it.
Curating would mean choosing the code the grammar already handles.

Real source has one structural blind spot, and it is worth being explicit
about: **it is all valid**. No quantity of correct code can show that a
grammar rejects what the language rejects. That is why the checks on the next
page exist.
