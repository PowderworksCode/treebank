---
title: Contributing
description: One concern per pull request, and fresh evidence for grammar changes.
order: 20
---

**One concern per pull request.** Sequential work rebases between pull
requests rather than stacking unrelated changes.

**A grammar change needs fresh evidence.** Change what the grammar accepts and
the committed ledger is stale. Re-run the sweep, let the ledger regenerate,
and never hand-edit a generated block.

**Run the gates first.** `cargo test --workspace` and `treebank status
--check` at minimum; the language's full verification for a grammar change;
`actionlint` for a workflow change. If you touched a ledger, run `bun run
status` in `site/` so the published inventory matches.

**Widening a grammar needs evidence.** A rule that accepts more is a claim
about the language, and the corpus and the reference parser are how that claim
is paid for.
