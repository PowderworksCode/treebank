/**
 * The `python2` variant: Python 2.7.
 *
 * Diff this against `../python3/grammar.js` — that is what the variant
 * mechanism is for. Every difference between the two languages that this
 * grammar knows about is one of the lines below.
 *
 * Read the empty lists as claims: no `branches` means no `match`
 * statement, no `orTestMembers` means no walrus, `softKeywords: []` means
 * `print` and `exec` are hard keywords here (and `match`, `case` and `type`
 * are ordinary names needing no special handling at all). What is NOT
 * claimed is in ledger.toml under known_widenings, and the reason it is
 * recorded rather than fixed is that this variant has no oracle yet: there
 * is no python 2 binary in CI, so a removal nothing can adjudicate is a
 * guess, and a guess in a parse table is worse than a written-down gap.
 */

/// <reference types="tree-sitter-cli/dsl" />
// @ts-check

const lexicon = require('../common/lexicon.js');

module.exports = require('../common/define-grammar.js')({
  name: 'python2',

  statements: ['print_statement', 'exec_statement'],
  // `...` is a py3 literal; in py2 it exists only inside a subscript.
  literals: [],
  // `a, *b = c` is PEP 3132, python 3.0.
  patternMembers: [],
  branches: [],
  orTestMembers: [],
  primaryExpressions: ['repr_expression'],
  comparisonOperators: ['<>'],
  plainParameters: ['tuple_parameter'],
  // py2.6 added `except E as e`; `except E, e` is the older spelling and
  // both are valid 2.7, which is why this list has two entries where
  // python3's has one.
  exceptAliases: ['as', ','],
  raiseTails: ['py2Comma'],
  softKeywords: [],
  integers: lexicon.PY2_INTEGERS,
  floats: lexicon.PY2_FLOATS,
  identifier: lexicon.PY2_IDENTIFIER,
  ruleGroups: require('../common/py2-rules.js'),
  features: {
    async: false,
    annotations: false,
    yieldFrom: false,
    exceptStar: false,
    parenthesizedWithItems: false,
  },

  conflicts: (_) => [],
});
