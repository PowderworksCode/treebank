Every file here is a mis-parse that was found and fixed. The shape check
runs over this directory with a ceiling of ZERO missed boundaries, so any
regression is caught in CI, where there is no corpus to sweep.

Each file must parse cleanly — a mis-parse is the point, an error is not.
Add to this directory whenever `treebank shape` finds something on the
corpus and it gets fixed.
