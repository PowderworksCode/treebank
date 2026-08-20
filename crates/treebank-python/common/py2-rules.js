// Rules that exist only in Python 2, as a rule group a variant includes by
// name (VARIANTS.md §3).
//
// What is worth noticing here is what ISN'T: no `prec.dynamic`, and no
// declared conflicts to go with it. In the union grammar `print` and `exec`
// had to stay usable as py3 identifiers, so both statements carried a
// negative dynamic precedence and six conflicts to lose the fork at every
// occurrence. In a variant where they are keywords the forms are
// unambiguous and the rules say only what they mean.

'use strict';

const { commaSep1 } = require('./helpers.js');

module.exports = {
  // Reachable only from a subscript -- see python2/grammar.js.
  ellipsis: _ => '...',

  // prec.dynamic(1): see python2/grammar.js's note on softKeywords. When
  // `print(x)` parses BOTH as this statement and as a call, the statement
  // wins, because that is what the text means in a file with no
  // `from __future__ import print_function`.
  print_statement: $ => prec.dynamic(1, seq(
    'print',
    optional(choice(
      seq('>>', $._expression, optional(seq(',', choice($._expression, $._expression_list_tuple)))),
      choice($._expression, $._expression_list_tuple),
    )),
  )),

  // The code operand is primary-tier so `exec code in g` needs no reduce
  // before the `in` -- which would otherwise lose to the comparison
  // reading statically.
  exec_statement: $ => seq(
    'exec',
    $._primary_expression,
    optional(seq('in', $._expression, optional(seq(',', $._expression)))),
  ),

  repr_expression: $ => seq('`', choice($._expression, $._expression_list_tuple), '`'),

  // `def f((a, b), c):` and `lambda (a, b): ...` -- removed in py3, where
  // the parameter list takes names only.
  tuple_parameter: $ => seq('(', commaSep1(choice($._name, $.tuple_parameter)), optional(','), ')'),
};
