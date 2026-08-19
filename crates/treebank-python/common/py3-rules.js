// Rules that exist only in Python 3, as a rule group a variant includes by
// name (VARIANTS.md §3). They are written here rather than in python3/ so
// the whole grammar stays in one place to read; what the variant file says
// is which groups it takes, not what they contain.
//
// Everything in the match sub-grammar (PEP 634) is here together, because
// the pattern rules are only reachable from `match_statement` -- take that
// out and every `case_*` rule goes with it.

'use strict';

const { PREC, commaSep1 } = require('./helpers.js');

module.exports = {
  ellipsis: _ => '...',


  _match_shape: $ => choice(
    $._name,
    $.member_expression,
    $.class_pattern,
    alias($.case_tuple_pattern, $.tuple_pattern),
    alias($.case_list_pattern, $.list_pattern),
    alias($.case_dict_pattern, $.dictionary_pattern),
  ),

  nonlocal_statement: $ => seq('nonlocal', commaSep1($._name)),

  // The variant's own rules land here, where the py2 statement forms
  // used to be, so a variant grammar.json keeps a readable ordering.
  // PEP 695 `type X = int`; `type` stays a plain name everywhere else.
  type_alias_statement: $ => prec.dynamic(-1, seq(
    'type',
    field('name', $._name),
    field('type_parameters', optional($.type_parameters)),
    '=',
    field('value', $._expression),
  )),

  // ── directives ───────────────────────────────────────────────────
  type_parameters: $ => seq(
    '[',
    commaSep1($.type_parameter),
    optional(','),
    ']',
  ),
  type_parameter: $ => seq(
    optional(choice('*', '**')),
    field('name', $._name),
    optional(seq(':', field('bound', $._expression))),
    optional(seq('=', field('value', $._expression))),
  ),

  // A parameter with no default and one with a default are the same
  // `parameter` node, but they are NOT interchangeable in the list, so
  // they are separate rules. See `parameterRules`.
  match_statement: $ => prec.dynamic(1, seq(
    'match',
    field('subject', choice($._expression, $._expression_list_tuple)),
    ':',
    field('body', alias($.match_block, $.block)),
  )),

  match_block: $ => seq($._newline, $._indent, repeat1($.case_clause), $._dedent),

  case_clause: $ => seq(
    'case',
    $._case_patterns,
    // CPython's grammar reads `guard: 'if' named_expression`, and a
    // named_expression is a full expression -- so a CONDITIONAL is a legal
    // guard: `case y if a if True else b:`. Restricting it here rejected
    // that, and the sweep could not see the gap because the fixture
    // carrying it is also invalid to `compile()` for an unrelated reason.
    optional(seq('if', field('guard', $._expression))),
    ':',
    field('body', $._body),
  ),

  // ── match patterns (PEP 634) ──────────────────────────────────
  // A dedicated sub-grammar rather than the expression rules. Patterns
  // LOOK like expressions — `Point(x=0)` like a call, `[a, *rest]` like
  // a list — and reusing expressions is cheap and right for the common
  // case, but `as` has no expression analogue, so it could only bind at
  // the top level: `case X() as w` parsed and `case [a as b]` did not.
  // 12 corpus files, including real matplotlib and cython code.
  //
  // Node names are shared with the destructuring patterns (aliased, not
  // duplicated) so `(tuple_pattern)` means the same shape in `a, b = x`
  // and in `case (a, b)`, and with rust wherever the construct matches.
  _case_patterns: $ => prec.left(seq(
    choice($._case_pattern, alias($.case_star_pattern, $.star_pattern)),
    repeat(seq(',', choice($._case_pattern, alias($.case_star_pattern, $.star_pattern)))),
    optional(','),
  )),

  _case_pattern: $ => choice($._or_pattern, $.as_pattern),

  as_pattern: $ => seq(
    field('pattern', $._or_pattern),
    'as',
    field('alias', $._name),
  ),

  _or_pattern: $ => choice($._closed_pattern, $.or_pattern),

  or_pattern: $ => prec.left(seq(
    $._closed_pattern,
    repeat1(seq('|', $._closed_pattern)),
  )),

  // The leaves reuse the language's own nodes, exactly as rust's
  // patterns reuse literals and paths: a bare name is an `identifier`
  // (including `_`, which is a real identifier in python), a dotted
  // value pattern is a `member_expression` — which is the occurrence
  // story working as designed, since the same node answers `(_access)`
  // where it is read and `(_pattern)` where it matches.
  // NOT threaded through `_pattern`, and the reason is measured: python's
  // destructuring positions and its match positions admit DIFFERENT
  // member sets (`x[0]` and `*rest` destructure but do not match;
  // `Point(x=0)` and `{"k": v}` match but do not destructure). A
  // supertype's members enter every position that references it, so
  // routing these through `_pattern` made `a, Point(x=0) = z` parse —
  // invalid python. This is the `_clause` law in a second place, and it
  // is why `(_pattern)` covers destructuring here while the match shapes
  // are reachable by their shared node names instead. Rust does not hit
  // this: its `let` and `match` patterns are one set.
  _closed_pattern: $ => choice(
    $._literal_pattern,
    $._match_shape,
    alias($.case_group_pattern, $.parenthesized_expression),
  ),

  // `-1`, `1+2j` and string concatenation are the literal forms PEP 634
  // admits; everything else that looks literal is a value pattern.
  _literal_pattern: $ => choice(
    $._literal,
    $.string,
    $.concatenated_string,
    alias($.case_signed_number, $.unary_expression),
    alias($.case_complex_number, $.binary_expression),
  ),

  case_signed_number: $ => seq(field('operator', '-'), field('operand', choice($.integer, $.float))),
  case_complex_number: $ => seq(
    field('left', choice($.integer, $.float, alias($.case_signed_number, $.unary_expression))),
    field('operator', choice('+', '-')),
    field('right', choice($.integer, $.float)),
  ),

  class_pattern: $ => seq(
    field('class', choice($._name, $.member_expression)),
    '(',
    optional(seq(commaSep1(choice($._case_pattern, $.keyword_pattern)), optional(','))),
    ')',
  ),

  keyword_pattern: $ => seq(
    field('name', $._name),
    '=',
    field('value', $._case_pattern),
  ),

  case_tuple_pattern: $ => seq('(', optional($._case_sequence), ')'),
  case_list_pattern: $ => seq('[', optional($._case_sequence), ']'),

  // A parenthesized single pattern with no comma is a group, not a
  // one-tuple — python's own distinction.
  case_group_pattern: $ => seq('(', $._case_pattern, ')'),

  _case_sequence: $ => seq(
    choice($._case_pattern, alias($.case_star_pattern, $.star_pattern)),
    repeat(seq(',', choice($._case_pattern, alias($.case_star_pattern, $.star_pattern)))),
    optional(','),
  ),

  case_star_pattern: $ => seq('*', $._name),

  case_dict_pattern: $ => seq(
    '{',
    optional(seq(
      commaSep1(choice(
        $.dictionary_pattern_pair,
        alias($.case_dict_splat, $.dictionary_splat_pattern),
      )),
      optional(','),
    )),
    '}',
  ),

  dictionary_pattern_pair: $ => seq(
    field('key', choice($._literal_pattern, $.member_expression)),
    ':',
    field('value', $._case_pattern),
  ),

  case_dict_splat: $ => seq('**', $._name),

  named_expression: $ => prec.right(PREC.walrus, seq(
    field('name', $._name),
    ':=',
    field('value', $._expression),
  )),

  // Operands are `_or_test`, never a bare conditional. CPython's grammar
  // reads `or_test: and_test ('or' and_test)*`, so a conditional can only
  // be an operand of `or` in parentheses -- and it is the LOOSEST operator
  // in the language, `a or b if c else d` being `(a or b) if c else d`.
  // With `$._expression` on the right we produced `a or (b if c else d)`
  // instead: a different program, no error, and no sweep could see it.
  await_expression: $ => prec(PREC.await, seq('await', $._primary_expression)),
};
