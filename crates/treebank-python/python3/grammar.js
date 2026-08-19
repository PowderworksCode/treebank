/**
 * The `python3` variant: Python 3.0 – 3.13.
 *
 * Everything is in `../common/define-grammar.js`; this file is the variant
 * manifest, and it is meant to stay readable as one — what differs from the
 * shared grammar is a list here, not a conditional there (VARIANTS.md §3).
 *
 * The grammar NAME stays `python`, not `python3`, and that is deliberate:
 * it is the C symbol (`tree_sitter_python`) consumers already link and the
 * scope editors already match. Python 3 is what `treebank_python::LANGUAGE`
 * has always meant, so the default keeps its name and the new variant takes
 * a qualified one.
 */

/// <reference types="tree-sitter-cli/dsl" />
// @ts-check

module.exports = require('../common/define-grammar.js')({
  name: 'python',
});
