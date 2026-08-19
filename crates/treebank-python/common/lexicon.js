// The token layer, where the Python variants differ below the parser.
//
// Separated from the rule structure because lexical divergence varies
// independently of syntax: python 2 and 3 agree about what a `for` loop
// looks like and disagree about what `0777` and `10L` are. VARIANTS.md §7.2
// makes the same separation for SQL, where it carries far more.

'use strict';

/**
 * Python 3 integers. No leading zeros on a nonzero decimal
 * (`0777` is "leading zeros in decimal integer literals are not
 * permitted"), no `L` suffix, and PEP 515 underscores throughout.
 *
 * The union grammar accepted both of those py2 forms in py3 code, because
 * one token family had to serve both versions. Splitting the variants is
 * what makes the py3 family able to say no.
 */
const PY3_INTEGERS = [
  /0[xX](_?[0-9a-fA-F])+/,
  /0[oO](_?[0-7])+/,
  /0[bB](_?[01])+/,
  // `0`, `00` and `0_0` are all valid python 3; `01` is not.
  /([1-9](_?[0-9])*|0(_?0)*)[jJ]?/,
];

/**
 * Python 2 integers: old-style octal (`0777`), the long suffix (`10L`),
 * and no underscore separators — PEP 515 is python 3.6.
 *
 * `0777` matches both the octal and the decimal alternative and lands on
 * the same `integer` node either way, so the overlap costs nothing.
 */
const PY2_INTEGERS = [
  /0[xX][0-9a-fA-F]+[lL]?/,
  /0[oO][0-7]+[lL]?/,
  /0[bB][01]+[lL]?/,
  /0[0-7]+[lL]?/,
  /[0-9]+[lLjJ]?/,
];

module.exports = { PY3_INTEGERS, PY2_INTEGERS };
