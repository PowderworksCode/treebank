Shape fixtures for bash. `treebank shape` compares our node BOUNDARIES
against mvdan/sh's over every file here, with a ceiling of ZERO missed
boundaries, and CI runs it on every push — which is where it earns its
keep, because CI has no corpus to sweep.

One caution these fixtures inherit from `shape_policy.toml`: bash's span
oracle is mvdan/sh, an independent reimplementation, NOT the reference —
bash has no AST to ask for. A fixture here records that our grouping and
theirs agree, knowing neither party is automatically right.

Nothing here is a scar. Each file is a construct family the grammar
handles today and that nothing was pinning: parameter expansion in its
dozen groupings, command substitution in both spellings and nested, and
control flow nested inside control flow. The point is the same as a
regression fixture either way. A tree that silently regroups is invisible
to the sweep, the negative corpus and the corpus tests alike, because all
three judge accept-or-reject and a wrong tree is still an accepted one.

Measured when added: 403 oracle nodes across the three files, 0 missed.

Each file must parse cleanly. An ERROR node is not a shape finding, it is
a different bug, and a fixture that errors is measuring nothing.

Deliberately absent, so their absence is a decision and not an oversight:

- **Heredocs.** Every heredoc form misses exactly three boundaries today,
  because the body is not part of the command that introduces it: our
  `command` ends at the delimiter word and `heredoc_body`/`heredoc_end`
  are siblings under `program`, so nothing spans mvdan/sh's `Stmt`,
  `Redirect` or `Word`. That is issue #221, and its fixtures belong with
  its fix — a heredoc fixture cannot sit here at a ceiling of zero, and
  declaring the pairs in `shape_policy.toml` would be recording a missing
  node as a granularity difference, which it is not.
- **Arithmetic.** `$(( ))` has no real grammar yet; `shape_policy.toml`
  carries the declared pairs and says they should be DELETED when it
  does, with the misses they hide as its acceptance test. Pinning
  arithmetic boundaries here would pin the gap instead.

Add to this directory whenever `treebank shape` finds something on the
corpus and it gets fixed, and whenever a construct family lands with
nothing pinning its boundaries.
