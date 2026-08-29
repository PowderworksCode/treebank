---
title: How it works
description: The corpus, the reference parsers, and what gets measured.
order: 10
---

Every grammar is run over a corpus of real code every day. Files that fail to
parse are handed to the language's own compiler or parser, which decides
whether the file was valid in the first place. A failure the reference parser
also rejects is noise; a failure it accepts is the grammar's problem.

That split is what the numbers on each grammar page mean, and the pages below
are the machinery behind them.
