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
  // ...but python 2 DOES have `...`, in exactly one position: inside a
  // subscript (`x[..., 1]`). It is not an expression there and nowhere
  // else, which is why it is a subscript member rather than a literal.
  subscriptMembers: ['ellipsis'],
  // `a, *b = c` is PEP 3132, python 3.0.
  patternMembers: [],
  branches: [],
  orTestMembers: [],
  primaryExpressions: ['repr_expression'],
  comparisonOperators: ['<>'],
  plainParameters: ['tuple_parameter'],
  // py2.6 added `except E as e`; `except E, e` is the older spelling and
  // both are valid 2.7, which is why this list has two entries where
  // python3's has one. The comma form binds a TARGET rather than a name --
  // `except socket.error, (errno, msg):` is real, and is in CPython's own
  // source three times.
  exceptAliases: ['as', 'py2Comma'],
  raiseTails: ['py2Comma'],
  // `print` is a soft keyword here, not a hard one, and the reason is
  // `from __future__ import print_function` (PEP 3105): with it, `print`
  // is an ordinary name and `print(x, file=f)` is a call. Without it the
  // same text is a print STATEMENT whose operand is parenthesised. Both
  // readings are real python 2 and neither can be chosen without knowing
  // what the file imported, which a parse table cannot know.
  //
  // So both parse, and `print_statement` carries a positive dynamic
  // precedence to win when both do — the no-future-import reading, which
  // is what plain `print(x)` means in a file that says nothing. The forms
  // only the call can read (`file=`, `end=`, `sep=`) reach it because the
  // statement reading simply fails there.
  //
  // This is the fork the python3 variant was freed of, deliberately put
  // back in the one table where it is cheap: there is no modern py3 corpus
  // here for it to slow down, and it buys 12 of the 22 gaps CPython 2.7's
  // own source found. `exec` stays a HARD keyword — it has no future
  // import and no call form.
  softKeywords: ['print'],
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
    commaIterable: true,
  },

  // The one conflict this variant declares, and it is the price of `print`
  // being a soft keyword: a bare `print` is both a complete print statement
  // and a name, and nothing decides which until the next token. GLR carries
  // both and the dynamic precedence in py2-rules.js settles the cases where
  // both complete.
  conflicts: ($) => [
    [$.print_statement, $._soft_keyword],
  ],
});
