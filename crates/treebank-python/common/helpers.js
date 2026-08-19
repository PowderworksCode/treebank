// Helpers shared by the grammar and by every rule-group module: the
// precedence table, the comma-separated-list combinators, and the
// parameter-list chain. They live here rather than in define-grammar.js
// because a rule group is a separate module and would otherwise have no
// way to reach them.

'use strict';

const PREC = {
  walrus: 1,
  lambda: 2,
  conditional: 0,
  or: 10,
  and: 11,
  not: 12,
  compare: 13,
  bitor: 14,
  bitxor: 15,
  bitand: 16,
  shift: 17,
  plus: 18,
  times: 19,
  unary: 20,
  power: 21,
  await: 22,
  postfix: 23,
};

/**
 * Python's parameter list is ordered, and the order is enforced by the
 * parser rather than by a later semantic pass: `def f(a=1, b)`,
 * `def f(**kw, a)`, `def f(*)` and `def f(a, /, /)` are all SyntaxErrors
 * out of CPython's own grammar. One `_parameter` alternation repeated by
 * commas cannot say any of that, so the list is spelled out as a chain of
 * "what may still follow" rules carrying two bits of state -- whether `/`
 * has been seen, and whether a parameter with a default has been seen --
 * and a separate section after `*`, where a parameter WITHOUT a default may
 * legally follow one with a default (`def f(*, a=1, b)` is valid Python).
 *
 * This is the reason `_parameter` is a facet rather than a supertype in
 * this grammar: the six parameter node types no longer share a derivation,
 * so tree-sitter cannot collect them under one supertype. All six are
 * concrete types that occur nowhere but a parameter list, so type-level
 * facet membership selects exactly the nodes occurrence-level supertype
 * membership would have. `(_parameter)` through treebank-core is unchanged.
 * See roles.json's `demoted` and DESIGN.md section 3.4.
 *
 * @param {string} prefix   rule-name prefix for this family of rules
 * @param {Object} p        the language-level pieces, which differ between
 *                          `def` (annotations allowed) and `lambda` (not)
 */
function parameterRules(prefix, p) {
  const R = (name) => `${prefix}_${name}`;
  // What may follow a parameter: nothing, a trailing comma, or a comma and
  // then whichever continuations `rest` still permits.
  const tail = ($, rest) => optional(seq(',', optional($[R(rest)])));

  return {
    // A list may not open with `/`, which needs something to be positional.
    [R('list')]: $ => choice(
      seq(p.plain($), tail($, 'nodefault_rest')),
      seq(p.withDefault($), tail($, 'default_rest')),
      $[R('star_section')],
    ),

    // No `/` yet, no default yet: everything is still open.
    [R('nodefault_rest')]: $ => choice(
      seq(p.plain($), tail($, 'nodefault_rest')),
      seq(p.withDefault($), tail($, 'default_rest')),
      seq($.positional_separator, tail($, 'slash_nodefault_rest')),
      $[R('star_section')],
    ),

    // A default has been seen: a parameter without one may no longer
    // follow, and `/` does not reset that.
    [R('default_rest')]: $ => choice(
      seq(p.withDefault($), tail($, 'default_rest')),
      seq($.positional_separator, tail($, 'slash_default_rest')),
      $[R('star_section')],
    ),

    // `/` has been seen; a second one is not allowed.
    [R('slash_nodefault_rest')]: $ => choice(
      seq(p.plain($), tail($, 'slash_nodefault_rest')),
      seq(p.withDefault($), tail($, 'slash_default_rest')),
      $[R('star_section')],
    ),

    [R('slash_default_rest')]: $ => choice(
      seq(p.withDefault($), tail($, 'slash_default_rest')),
      $[R('star_section')],
    ),

    // Everything after `*` is keyword-only. A bare `*` is a separator, not
    // a parameter, so it must be followed by at least one keyword-only
    // parameter -- `def f(*)` and `def f(*, **kw)` are both errors -- while
    // `*args` may stand alone. `**kwargs` closes the list.
    [R('star_section')]: $ => choice(
      seq(p.star($), tail($, 'keyword_rest')),
      seq($.keyword_separator, ',', choice(p.plain($), p.withDefault($)), tail($, 'keyword_rest')),
      seq(p.doubleStar($), optional(',')),
    ),

    [R('keyword_rest')]: $ => choice(
      seq(choice(p.plain($), p.withDefault($)), tail($, 'keyword_rest')),
      seq(p.doubleStar($), optional(',')),
    ),
  };
}

function commaSep1(rule) {
  return sep1(rule, ',');
}

function sep1(rule, separator) {
  return seq(rule, repeat(seq(separator, rule)));
}

module.exports = { PREC, parameterRules, commaSep1, sep1 };
