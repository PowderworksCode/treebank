/**
 * The `python3` variant: Python 3.0 – 3.13.
 *
 * Everything is in `../common/define-grammar.js`; this file is the variant
 * manifest, and it is meant to stay readable as one — what differs from
 * the shared grammar is a list here, not a conditional there
 * (VARIANTS.md §3). Every extension point is spelled out even when empty,
 * so the two variant files diff against each other.
 *
 * The grammar NAME stays `python`, not `python3`, and that is deliberate:
 * it is the C symbol (`tree_sitter_python`) consumers already link and the
 * scope editors already match. Python 3 is what `treebank_python::LANGUAGE`
 * has always meant, so the default keeps its name and the new variant takes
 * a qualified one.
 */

/// <reference types="tree-sitter-cli/dsl" />
// @ts-check

const lexicon = require('../common/lexicon.js');

module.exports = require('../common/define-grammar.js')({
  name: 'python',

  // Nothing to add: python 3 IS the shared grammar. Every list below is
  // empty or single-valued precisely because the py2 forms that used to
  // share this table have gone to the variant that wants them.
  statements: [],
  primaryExpressions: [],
  comparisonOperators: [],
  plainParameters: [],
  exceptAliases: ['as'],
  raiseTails: ['from'],
  softKeywords: ['match', 'case', 'type'],
  integers: lexicon.PY3_INTEGERS,
  ruleGroups: {},
  conflicts: (_) => [],
});
