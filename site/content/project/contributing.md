---
title: Contributing
description: One concern per pull request, and every grammar change carries fresh evidence.
order: 20
---

The rules that matter:

**One concern per pull request.** Sequential work rebases between pull
requests rather than stacking unrelated changes.

**Every grammar-input change requires a fresh sweep.** Change what the grammar
accepts and the committed evidence is stale. Re-run the sweep, let the ledger
be regenerated mechanically, and never hand-edit a generated block.

**Run the gates before opening.** `cargo test --workspace` and `treebank
status --check` at minimum; the relevant language's full verification for a
grammar change; `actionlint` for any workflow change.

**Widening a grammar needs evidence.** A rule that accepts more is a claim
about the language, and the corpus and oracle are how that claim gets paid
for.
