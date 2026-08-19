/**
 * treebank-python: the shared source for every Python variant, carrying
 * the treebank vocabulary (DESIGN.md §3) in its parse table.
 *
 * This file is the whole grammar. It is not generated on its own — a
 * variant directory (`python3/`, `python2/`) calls it with a descriptor,
 * and each call produces one parse table. Per VARIANTS.md §3 a variant may
 * add members to a DECLARED extension point and may remove them; it may
 * not rewrite the internals of a shared rule. The extension points are the
 * `v.*` lists read below, and they are the complete list:
 *
 *   v.statements           extra `_simple_statement` members
 *   v.primaryExpressions   extra `_primary_expression` members
 *   v.comparisonOperators  extra comparison operators
 *   v.plainParameters      extra plain-parameter forms (`def` and `lambda`)
 *   v.exceptAliases        how `except E <alias>` may bind
 *   v.raiseTails           what may follow `raise E`
 *   v.softKeywords         words that are keywords in one construct only
 *   v.integers             the integer token family (see lexicon.js)
 *   v.ruleGroups           rule definitions this variant includes
 *   v.conflicts            GLR conflicts only this variant needs
 *
 * Adding an extension point is a deliberate change to this file, reviewed
 * as one. That is what stops the parameterization from turning into a
 * second grammar language.
 *
 * Threaded table-tier roles (18): _statement _expression _declaration
 * _pattern _name _literal _parameter _argument _member _clause? no — see
 * ledger. Actual list: see `supertypes` below; omissions and the reasons
 * for them are in ledger.toml's roles_note.
 */

/// <reference types="tree-sitter-cli/dsl" />
// @ts-check

const tb = require('../../treebank-core/vocabulary/supertypes.js');
const lexicon = require('./lexicon.js');

/**
 * A name position: a plain identifier, plus this variant's soft keywords —
 * words that are keywords inside one construct and ordinary names
 * everywhere else. A variant with none (python 2, where `print` and `exec`
 * are hard keywords and `match`/`case`/`type` are just names) omits the
 * `_soft_keyword` rule entirely rather than carrying an empty choice.
 *
 * @param {any} v
 * @param {any} $
 */
function nameChoices(v, $) {
  return v.softKeywords.length
    ? [$.identifier, alias($._soft_keyword, $.identifier)]
    : [$.identifier];
}

/**
 * What may follow `raise E`. Two shapes rather than two keywords, so the
 * variant selects by name instead of the grammar branching on a flag.
 */
const RAISE_TAILS = {
  from: $ => seq('from', field('cause', $._expression)),
  py2Comma: $ => seq(',', $._expression, optional(seq(',', $._expression))),
};

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

module.exports = (v) => grammar({
  name: v.name,

  word: $ => $.identifier,

  extras: $ => [
    $.comment,
    /[\s\f﻿⁠​]|\\\r?\n/,
  ],

  externals: $ => [
    $._newline,
    $._indent,
    $._dedent,
    $.string_start,
    $.string_content,
    $.string_end,
    $._line_start,
  ],

  supertypes: $ => tb.assertTableTerms([
    '_statement',
    '_expression',
    '_declaration',
    '_pattern',
    '_name',
    '_literal',
    // `_parameter` is demoted to the facet tier in this grammar: Python's
    // parameter list is ordered, so the six parameter node types no longer
    // share one derivation. See `parameterRules` below and roles.json.
    ...tb.assertDemotable([]),
    '_argument',
    '_member',
    '_directive',
    '_body',
    '_branch',
    '_loop',
    '_jump',
    '_assignment',
    '_invocation',
    '_access',
    '_attribute',
    '_interpolation',
  ]).map((name) => $[name]),

  conflicts: $ => [
    [$._augmented_target, $._pattern, $._access],
    [$._augmented_target, $._name, $._pattern],
    [$._comma_expressions],
    [$._no_conditional_expression, $.conditional_expression],
    [$._patterns_comma, $._closed_pattern],
    [$._match_shape, $._access],
    [$._match_shape, $._primary_expression],
    [$.case_dict_splat, $._primary_expression],
    [$.case_star_pattern, $._primary_expression],
    [$.dictionary_pattern_pair, $._access],
    [$.case_dict_pattern, $.dictionary],
    [$.case_signed_number, $._literal],
    [$.case_list_pattern, $.list],
    [$.case_group_pattern, $._case_sequence],
    [$.case_tuple_pattern, $.tuple],
    [$._literal_pattern, $._primary_expression],
    [$._closed_pattern, $._access],
    [$.class_pattern, $._access],
    [$._closed_pattern, $._primary_expression],
    [$.class_pattern, $._primary_expression],
    [$.case_complex_number, $._literal],
    // `a, b` at statement start: tuple until `=` proves pattern_list.
    [$.tuple, $.tuple_pattern],
    [$.list, $.list_pattern],
    // `a` / `a.b` / `a[i]` at a left-hand position: expression until the
    // context decides pattern. The shared nodes are the point (§4.1): the
    // same member_expression is _pattern on the left, _access on the right.
    [$._pattern, $._name],
    [$._pattern, $._access],
    // `with (a, b):` — parenthesized with-items vs a parenthesized tuple.
    [$.with_item, $._collection_elements],
    // In `def f(x: int)` the colon is an annotation; in `lambda x: y` it is
    // the body. Same parameter rule, GLR decides per context.
    [$.parameter],
    [$.star_parameter],
    [$.double_star_parameter],
    [$.match_statement, $._soft_keyword],
    [$.type_alias_statement, $._soft_keyword],
    // A conditional's consequence and a completed construct are
    // indistinguishable until the `if`/`else` arrives; GLR forks and the
    // reading without an `else` dies. Generate marks some of these
    // unnecessary, but with dynamic precedences in play its analysis is
    // unreliable — removing them broke `exec a in b, c` at runtime.
    [$.conditional_expression, $.expression_statement],
    [$.delete_statement, $.conditional_expression],
    [$.assert_statement, $.conditional_expression],
    [$._right_hand_side, $.conditional_expression],
    [$.assignment, $.conditional_expression],
    [$.conditional_expression, $._comma_expressions],
    [$.type_alias_statement, $.conditional_expression],
    [$._case_patterns, $.conditional_expression],
    ...v.conflicts($),
  ],

  rules: {
    module: $ => repeat($._line),

    // ── the statement tier ───────────────────────────────────────────
    // A supertype must always yield exactly one visible node, so the
    // physical line (`a = 1; b = 2`) is a hidden wrapper OUTSIDE
    // `_statement`, and every statement occurrence still derives through
    // `_statement` — which is what makes `(_statement)` match each one.
    // A compound statement in a `;`-line rejects itself: its suite has
    // already consumed the newline the line wrapper requires.
    // Every logical line begins with the scanner's zero-width _line_start,
    // which exists only at genuine line starts — it is what stops a simple
    // statement from slipping through the compound alternative without its
    // NEWLINE and letting `x = 1 y = 2` parse as two statements.
    _line: $ => seq(
      $._line_start,
      choice(
        seq($._statement, repeat(seq(';', $._statement)), optional(';'), $._newline),
        $._statement,
      ),
    ),

    _statement: $ => choice(
      $._simple_statement,
      $._compound_statement,
    ),

    _simple_statement: $ => choice(
      $.expression_statement,
      $._assignment,
      $._jump,
      $._directive,
      $.pass_statement,
      $.delete_statement,
      $.assert_statement,
      $.global_statement,
      $.nonlocal_statement,
      ...v.statements.map((name) => $[name]),
      $.type_alias_statement,
    ),

    _compound_statement: $ => choice(
      $._declaration,
      $._branch,
      $._loop,
      $.try_statement,
      $.with_statement,
    ),

    expression_statement: $ => choice(
      $._expression,
      $._expression_list_tuple,
      $.yield_expression,
    ),

    // ── assignment ───────────────────────────────────────────────────
    _assignment: $ => choice(
      $.assignment,
      $.augmented_assignment,
    ),

    assignment: $ => seq(
      field('left', $._left_hand_side),
      choice(
        seq('=', field('right', $._right_hand_side)),
        seq(':', field('type', $._expression), optional(seq('=', field('right', $._right_hand_side)))),
      ),
    ),

    // A single target, never a list: `a, b += 1` and `[a] += 1` are both
    // `illegal expression for augmented assignment`. Plain assignment takes
    // `_left_hand_side`; this one may not. Parentheses ARE allowed, and
    // nest -- `(o.a) += v` and `((a)) += 1` are both valid -- so the target
    // is recursive through them.
    augmented_assignment: $ => seq(
      field('left', $._augmented_target),
      field('operator', choice(
        '+=', '-=', '*=', '/=', '//=', '%=', '@=', '**=',
        '>>=', '<<=', '&=', '^=', '|=',
      )),
      field('right', $._right_hand_side),
    ),

    _left_hand_side: $ => choice(
      $._pattern,
      $.pattern_list,
    ),

    // Parentheses come in as `parenthesized_expression` rather than as part
    // of this rule. Spelling them out here means `( identifier )` cannot be
    // told from a name, a pattern or an access until the `+=` arrives, and
    // paying for that with four declared conflicts -- in a grammar where a
    // declared conflict also switches static precedence off -- is not worth
    // one form. The cost is that `(a + b) += 1` is still accepted, which
    // CPython calls `'BinOp' is an illegal expression`; `a, b += 1` and
    // `[a] += 1` are now rejected, which is what was reported.
    _augmented_target: $ => choice(
      ...nameChoices(v, $),
      $.member_expression,
      $.subscript_expression,
      // Parens recurse through the TARGET, not through expression:
      // `(o.a) += v` and `((a)) += 1` are valid, but `( yield ) += x` and
      // `( a if b else c ) += x` are `illegal expression for augmented
      // assignment` -- the full parenthesized_expression here was 10 fuzz
      // seeds of exactly those.
      alias($._paren_augmented_target, $.parenthesized_expression),
    ),

    _paren_augmented_target: $ => seq('(', $._augmented_target, ')'),

    _right_hand_side: $ => choice(
      $._expression,
      $._expression_list_tuple,
      $.yield_expression,
      $.assignment,           // a = b = c
    ),

    // ── patterns (destructuring positions) ───────────────────────────
    _pattern: $ => choice(
      ...nameChoices(v, $),
      $.member_expression,
      $.subscript_expression,
      $.star_pattern,
      $.tuple_pattern,
      $.list_pattern,
    ),

    // The match-pattern shapes, named as the destructuring ones so the
    // node vocabulary does not fork: `(tuple_pattern)` and `(list_pattern)`
    // mean the same shape in `a, b = x` and in `case (a, b)`.
    _match_shape: $ => choice(
      $._name,
      $.member_expression,
      $.class_pattern,
      alias($.case_tuple_pattern, $.tuple_pattern),
      alias($.case_list_pattern, $.list_pattern),
      alias($.case_dict_pattern, $.dictionary_pattern),
    ),

    pattern_list: $ => prec.right(seq(
      $._pattern,
      choice(
        ',',
        seq(repeat1(seq(',', $._pattern)), optional(',')),
      ),
    )),

    tuple_pattern: $ => seq('(', optional($._patterns_comma), ')'),
    list_pattern: $ => seq('[', optional($._patterns_comma), ']'),
    _patterns_comma: $ => prec.right(seq($._pattern, repeat(seq(',', $._pattern)), optional(','))),
    star_pattern: $ => prec(1, seq('*', $._pattern)),

    // ── simple statements ────────────────────────────────────────────
    pass_statement: _ => 'pass',

    _jump: $ => choice(
      $.return_statement,
      $.break_statement,
      $.continue_statement,
      $.raise_statement,
    ),

    // `yield_expression` stays, and `return yield x` stays accepted. Taking
    // it out does not reject that program: tree-sitter prefers an extracted
    // keyword only where the keyword is VALID, so with `yield` no longer a
    // legal continuation of `return` the lexer simply falls back to
    // `identifier` and the program parses as returning a variable called
    // `yield` -- the same acceptance wearing a worse tree. Rejecting it
    // needs `yield` to be unable to lex as an identifier at all, which is
    // what tree-sitter's `reserved` word sets are for and is a change of a
    // different size. Queued in ledger.toml with that diagnosis.
    return_statement: $ => prec.right(seq('return', optional(choice($._expression, $._expression_list_tuple, $.yield_expression)))),
    break_statement: _ => 'break',
    continue_statement: _ => 'continue',

    raise_statement: $ => prec.right(seq(
      'raise',
      optional(seq(
        $._expression,
        optional(choice(...v.raiseTails.map((name) => RAISE_TAILS[name]($)))),
      )),
    )),

    // `del` takes TARGETS, not expressions: `del a if b else c` is
    // `cannot delete conditional expression` to CPython, and admitting the
    // whole expression tier here is what let it through.
    // A star is not deletable: `del *a` is `cannot delete starred`. The
    // targets are otherwise the assignment ones.
    delete_statement: $ => seq('del', choice(
      $._del_target,
      alias($._del_target_list, $.pattern_list),
    )),

    _del_target: $ => choice(
      ...nameChoices(v, $),
      $.member_expression,
      $.subscript_expression,
      $.tuple_pattern,
      $.list_pattern,
    ),
    _del_target_list: $ => seq(
      $._del_target,
      choice(',', seq(repeat1(seq(',', $._del_target)), optional(','))),
    ),

    assert_statement: $ => seq('assert', $._expression, optional(seq(',', $._expression))),

    global_statement: $ => seq('global', commaSep1($._name)),
    nonlocal_statement: $ => seq('nonlocal', commaSep1($._name)),

    // The variant's own rules land here, where the py2 statement forms
    // used to be, so a variant grammar.json keeps a readable ordering.
    ...v.ruleGroups,

    // PEP 695 `type X = int`; `type` stays a plain name everywhere else.
    type_alias_statement: $ => prec.dynamic(-1, seq(
      'type',
      field('name', $._name),
      field('type_parameters', optional($.type_parameters)),
      '=',
      field('value', $._expression),
    )),

    // ── directives ───────────────────────────────────────────────────
    _directive: $ => choice(
      $.import_statement,
      $.import_from_statement,
    ),

    import_statement: $ => seq('import', commaSep1(choice($.dotted_name, $.aliased_import))),

    import_from_statement: $ => seq(
      'from',
      field('module', choice($.relative_import, $.dotted_name)),
      'import',
      choice(
        $.wildcard_import,
        commaSep1($._imported_name),
        seq('(', commaSep1($._imported_name), optional(','), ')'),
      ),
    ),

    // What `from x import …` may name is a plain identifier, never a dotted
    // path: `from a import b.c` is `invalid syntax`. Only the MODULE half
    // takes dots. The node types are unchanged -- a single-identifier
    // `dotted_name` is exactly what `from a import b` already produced.
    _imported_name: $ => choice(
      alias($._single_dotted_name, $.dotted_name),
      alias($._imported_aliased, $.aliased_import),
    ),
    _single_dotted_name: $ => seq($.identifier),
    _imported_aliased: $ => seq(
      field('name', alias($._single_dotted_name, $.dotted_name)),
      'as',
      field('alias', $._name),
    ),

    relative_import: $ => seq(repeat1('.'), optional($.dotted_name)),
    aliased_import: $ => seq(field('name', $.dotted_name), 'as', field('alias', $._name)),
    wildcard_import: _ => '*',
    dotted_name: $ => prec.right(sep1($.identifier, '.')),

    // ── declarations ─────────────────────────────────────────────────
    _declaration: $ => choice(
      $.function_definition,
      $.class_definition,
    ),

    _attribute: $ => choice($.decorator),

    decorator: $ => seq('@', $._expression, $._newline),

    function_definition: $ => seq(
      repeat($._attribute),
      optional('async'),
      'def',
      field('name', $._name),
      field('type_parameters', optional($.type_parameters)),
      field('parameters', $.parameters),
      optional(seq('->', field('return_type', $._expression))),
      ':',
      field('body', $._body),
    ),

    parameters: $ => seq('(', optional($._params_list), ')'),

    // `def f(...)`: annotations allowed.
    ...parameterRules('_params', {
      plain: $ => choice($.parameter, ...v.plainParameters.map((name) => $[name])),
      withDefault: $ => alias($._parameter_with_default, $.parameter),
      star: $ => $.star_parameter,
      doubleStar: $ => $.double_star_parameter,
    }),

    // `lambda ...`: annotations forbidden -- see the note on
    // `_lambda_plain_parameter`.
    ...parameterRules('_lambda_params', {
      plain: $ => choice(
        alias($._lambda_plain_parameter, $.parameter),
        ...v.plainParameters.map((name) => $[name]),
      ),
      withDefault: $ => alias($._lambda_parameter_with_default, $.parameter),
      star: $ => alias($._lambda_star_parameter, $.star_parameter),
      doubleStar: $ => alias($._lambda_double_star_parameter, $.double_star_parameter),
    }),

    // PEP 695: `def f[T](...)`, `class C[T]:`, `type X[T] = ...`.
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
    parameter: $ => seq(
      field('name', $._name),
      optional(seq(':', field('type', $._expression))),
    ),
    _parameter_with_default: $ => seq(
      field('name', $._name),
      optional(seq(':', field('type', $._expression))),
      '=',
      field('value', $._expression),
    ),

    // PEP 646: a variadic parameter's annotation may itself be starred --
    // `def fn(*args: *tuple[*A, B])`. It is the only annotation position
    // that takes a star; `def f(x: *a)` is not valid.
    star_parameter: $ => seq(
      '*',
      field('name', $._name),
      optional(seq(':', field('type', choice($._expression, $.starred_expression)))),
    ),
    double_star_parameter: $ => seq('**', field('name', $._name), optional(seq(':', field('type', $._expression)))),
    keyword_separator: _ => '*',
    positional_separator: _ => '/',

    class_definition: $ => seq(
      repeat($._attribute),
      'class',
      field('name', $._name),
      field('type_parameters', optional($.type_parameters)),
      field('arguments', optional($.arguments)),
      ':',
      field('body', alias($.class_block, $.block)),
    ),

    // A class body is the same suite shape, threaded through `_member` so
    // `(_member)` matches exactly the statements that are members. The
    // node is aliased to `block` so trees stay uniform.
    class_block: $ => choice(
      seq($._member, repeat(seq(';', $._member)), optional(';'), $._newline),
      seq($._newline, $._indent, repeat1($._member_line), $._dedent),
    ),
    _member_line: $ => seq(
      $._line_start,
      choice(
        seq($._member, repeat(seq(';', $._member)), optional(';'), $._newline),
        $._member,
      ),
    ),
    _member: $ => choice($._statement),

    // ── control flow ─────────────────────────────────────────────────
    _branch: $ => choice($.if_statement, $.match_statement),

    if_statement: $ => seq(
      'if',
      field('condition', $._expression),
      ':',
      field('body', $._body),
      repeat(field('alternative', $.elif_clause)),
      optional(field('alternative', $.else_clause)),
    ),

    elif_clause: $ => seq('elif', field('condition', $._expression), ':', field('body', $._body)),

    // PEP 634. `match` and `case` stay usable as plain names everywhere
    // else — they are in _soft_keyword.
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

    else_clause: $ => seq('else', ':', field('body', $._body)),

    _loop: $ => choice($.while_statement, $.for_statement),

    while_statement: $ => seq(
      'while',
      field('condition', $._expression),
      ':',
      field('body', $._body),
      optional(field('alternative', $.else_clause)),
    ),

    for_statement: $ => seq(
      optional('async'),
      'for',
      field('left', $._left_hand_side),
      'in',
      field('right', choice($._expression, $._expression_list_tuple)),
      ':',
      field('body', $._body),
      optional(field('alternative', $.else_clause)),
    ),

    try_statement: $ => seq(
      'try',
      ':',
      field('body', $._body),
      choice(
        seq(
          repeat1($.except_clause),
          optional($.else_clause),
          optional($.finally_clause),
        ),
        $.finally_clause,
      ),
    ),

    except_clause: $ => seq(
      'except',
      optional('*'),                      // py3.11 except*
      optional(seq(
        $._expression,
        optional(choice(...v.exceptAliases.map((kw) => seq(kw, field('alias', $._name))))),
      )),
      ':',
      field('body', $._body),
    ),

    finally_clause: $ => seq('finally', ':', field('body', $._body)),

    with_statement: $ => seq(
      optional('async'),
      'with',
      choice(
        commaSep1($.with_item),
        $._parenthesized_with_items,      // py3.10
      ),
      ':',
      field('body', $._body),
    ),

    _parenthesized_with_items: $ => seq('(', commaSep1($.with_item), optional(','), ')'),

    with_item: $ => seq(
      field('value', $._expression),
      optional(seq('as', field('alias', $._pattern))),
    ),

    // ── suites ───────────────────────────────────────────────────────
    _body: $ => choice($.block),

    block: $ => choice(
      // The trailing `optional($._newline)` absorbs the SECOND newline that a
      // COMMENT line after an inline suite produces:
      //
      //     elif n == 4: underline = True
      //     # Code 5: blinking
      //     elif n == 5: bold = True
      //
      // is ordinary Python and appears in real code (fastcore, dill). A
      // block suite absorbs the extra into its DEDENT; an inline suite opens
      // no indent level, so there is nowhere else for it to go. Bounded at
      // one, not `repeat1`: the scanner can emit a zero-width NEWLINE at
      // EOF, and an unbounded repeat over it hangs the parser.
      // `prec.right` resolves the shift/reduce on that second newline toward
      // consuming it here. Safe because the tolerance is bounded at one; the
      // same shape with `repeat1` hangs the parser.
      prec.right(seq($._statement, repeat(seq(';', $._statement)), optional(';'), $._newline, optional($._newline))),
      seq($._newline, $._indent, repeat1($._line), $._dedent),
    ),

    // ── expressions ──────────────────────────────────────────────────
    _expression: $ => choice(
      $.conditional_expression,
      $._no_conditional_expression,
    ),

    // `*x` is NOT an expression in python; it is an element of something
    // list-shaped. CPython admits it in a call, a display, a subscript and a
    // comma-separated list, and nowhere else -- `assert *x`, `if *x:`,
    // `while *x:` and `*x if c else y` are all `invalid syntax` from its
    // parser, and having it inside `_expression` made every one of them
    // parse. So `starred_expression` is written out at each position that
    // takes it, rather than hidden behind a rule: a hidden rule is a unit
    // reduction, and at `(yield x , …)` that reduction has to happen before
    // the parser can tell a yielded expression from a collection element.

    // Python's comprehension conditions and iterables are `or_test`: a bare
    // conditional is excluded by construction, which is also what lets
    // `... if a if b` chain instead of swallowing the second `if`.
    _no_conditional_expression: $ => choice($.lambda, $._or_test),

    // CPython's `or_test`: everything a conditional's consequence and
    // condition may be, which notably EXCLUDES a lambda. Its grammar reads
    //
    //     test: or_test ['if' or_test 'else' test] | lambdef
    //
    // so only the ELSE branch may be a bare lambda. Encoding that is what
    // settles `lambda x: A if c else B`: with the consequence allowed to be
    // a lambda, `(lambda x: A) if c else B` and `lambda x: (A if c else B)`
    // were both complete parses holding identical node multisets -- one
    // lambda, one conditional -- so neither precedence nor dynamic
    // precedence had anything to weigh, and the wrong one won.
    _or_test: $ => choice(
      alias($.boolean_expression, $.binary_expression),
      alias($.not_expression, $.unary_expression),
      $.comparison_expression,
      $.named_expression,
      $._primary_expression,
    ),

    _primary_expression: $ => choice(
      $.binary_expression,
      $.unary_expression,
      $.await_expression,
      $._invocation,
      $._access,
      $._name,
      $._literal,
      $.string,
      $.concatenated_string,
      $.list,
      $.tuple,
      $.set,
      $.dictionary,
      $.list_comprehension,
      $.set_comprehension,
      $.dictionary_comprehension,
      $.generator_expression,
      $.parenthesized_expression,
      ...v.primaryExpressions.map((name) => $[name]),
    ),

    _name: $ => choice(...nameChoices(v, $)),
    ...(v.softKeywords.length
      ? { _soft_keyword: /** @type {any} */ (_ => choice(...v.softKeywords)) }
      : {}),

    conditional_expression: $ => prec.right(PREC.conditional, seq(
      field('consequence', $._or_test),
      'if',
      field('condition', $._or_test),
      'else',
      field('alternative', $._expression),
    )),

    // `prec.right`: a lambda BODY is greedy. Python's grammar gives it a
    // full `test`, so `lambda x: A if c else B` is
    // `lambda x: (A if c else B)`. With plain `prec` the lambda (2) reduced
    // rather than shifting into the conditional (0), and the whole family
    // came out as `(lambda x: A) if c else B` -- a different program, no
    // error, invisible to the sweep.
    lambda: $ => prec.right(PREC.lambda, seq(
      'lambda',
      field('parameters', optional(alias($._lambda_params_list, $.parameters))),
      ':',
      field('body', $._expression),
    )),
    // A lambda's parameters carry NO annotations -- Python forbids them
    // there. With the annotated form reachable, `{lambda x: x: 1}` (a dict
    // whose KEY is a lambda) parsed as a SET holding one lambda, whose
    // parameter was annotated `x: x` and whose body was `1`. The dict's own
    // colon had been eaten as an annotation.
    _lambda_plain_parameter: $ => seq(field('name', $._name)),
    _lambda_parameter_with_default: $ => seq(
      field('name', $._name),
      '=',
      field('value', $._expression),
    ),
    _lambda_star_parameter: $ => seq('*', field('name', $._name)),
    _lambda_double_star_parameter: $ => seq('**', field('name', $._name)),

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
    boolean_expression: $ => choice(
      prec.left(PREC.or, seq(field('left', $._or_test), field('operator', 'or'), field('right', $._or_test))),
      prec.left(PREC.and, seq(field('left', $._or_test), field('operator', 'and'), field('right', $._or_test))),
    ),

    not_expression: $ => prec(PREC.not, seq(field('operator', 'not'), field('operand', $._or_test))),

    binary_expression: $ => {
      const table = [
        ['|', PREC.bitor], ['^', PREC.bitxor], ['&', PREC.bitand],
        ['<<', PREC.shift], ['>>', PREC.shift],
        ['+', PREC.plus], ['-', PREC.plus],
        ['*', PREC.times], ['/', PREC.times], ['//', PREC.times],
        ['%', PREC.times], ['@', PREC.times],
      ];
      return choice(
        ...table.map(([op, p]) => prec.left(p, seq(
          field('left', $._primary_expression),
          field('operator', op),
          field('right', $._primary_expression),
        ))),
        prec.right(PREC.power, seq(
          field('left', $._primary_expression),
          field('operator', '**'),
          field('right', $._primary_expression),
        )),
      );
    },

    // Operands are the primary tier — python's own shape (comparison
    // operands are bitwise-or expressions), which is what keeps
    // `a < b <= c` one flat chain instead of nesting.
    comparison_expression: $ => prec.left(PREC.compare, seq(
      $._primary_expression,
      repeat1(seq(
        field('operator', choice(
          '<', '<=', '>', '>=', '==', '!=',
          ...v.comparisonOperators,
          'in', seq('not', 'in'), 'is', seq('is', 'not'),
        )),
        $._primary_expression,
      )),
    )),

    unary_expression: $ => prec(PREC.unary, seq(
      field('operator', choice('-', '+', '~')),
      field('operand', $._primary_expression),
    )),

    await_expression: $ => prec(PREC.await, seq('await', $._primary_expression)),

    yield_expression: $ => prec.right(seq(
      'yield',
      optional(choice(
        seq('from', $._expression),
        choice($._expression, $._expression_list_tuple),
      )),
    )),

    starred_expression: $ => prec(1, seq('*', $._expression)),
    _double_starred_expression: $ => seq('**', $._expression),

    _invocation: $ => choice($.call_expression),

    call_expression: $ => prec(PREC.postfix, seq(
      field('function', $._primary_expression),
      field('arguments', $.arguments),
    )),

    arguments: $ => seq(
      '(',
      optional(seq(commaSep1($._argument), optional(','))),
      ')',
    ),

    // `f(x for x in y)`: a generator as sole argument borrows the call's
    // parentheses, so the bare form gets its own rule, aliased to the node.
    _argument: $ => choice(
      $._expression,
      $.starred_expression,
      $.keyword_argument,
      alias($._double_starred_expression, $.splat_argument),
      alias($.bare_generator, $.generator_expression),
    ),

    bare_generator: $ => seq(field('body', $._expression), $._comprehension_clauses),

    keyword_argument: $ => seq(
      field('name', $._name),
      '=',
      field('value', $._expression),
    ),

    _access: $ => choice($.member_expression, $.subscript_expression),

    member_expression: $ => prec(PREC.postfix, seq(
      field('object', $._primary_expression),
      '.',
      field('property', $._name),
    )),

    subscript_expression: $ => prec(PREC.postfix, seq(
      field('object', $._primary_expression),
      '[',
      commaSep1(field('subscript', choice($._expression, $.starred_expression, $.slice))),
      optional(','),
      ']',
    )),

    slice: $ => seq(
      optional($._expression),
      ':',
      optional($._expression),
      optional(seq(':', optional($._expression))),
    ),

    parenthesized_expression: $ => prec(PREC.postfix, seq(
      '(',
      choice($._expression, $.yield_expression),
      ')',
    )),

    // A bare comma-joined expression list is a tuple in all but name.
    _expression_list_tuple: $ => alias($._comma_expressions, $.tuple),
    // No `prec.right` here. It resolved the decision at `expr , .` in favour
    // of shifting -- of expecting another element -- which REMOVED the
    // reduce action, so `_newline` was no longer a valid lookahead in that
    // state, so the scanner was never asked for one. A trailing comma then
    // ran the tuple straight across the line break:
    //
    //     x = int,
    //     y = 1, 2
    //
    // became the single chained assignment `x = (int, y) = (1, 2)`. Two
    // statements silently became one. With the reduce available the scanner
    // is consulted, emits NEWLINE, and the continuation fork dies where it
    // should.
    //
    // `prec.dynamic` because both readings are COMPLETE parses and GLR has
    // to choose. They differ in what they contain: the correct one holds two
    // tuples, the merged one holds a `pattern_list` and a tuple. Weighting
    // the tuple is the asymmetry.
    _comma_expressions: $ => prec.dynamic(1, seq(
      choice($._expression, $.starred_expression),
      choice(
        ',',
        seq(repeat1(seq(',', choice($._expression, $.starred_expression))), optional(',')),
      ),
    )),

    // ── containers ───────────────────────────────────────────────────
    list: $ => seq('[', optional($._collection_elements), ']'),
    set: $ => seq('{', $._collection_elements, '}'),
    tuple: $ => seq('(', optional($._collection_elements), ')'),

    _collection_elements: $ => seq(
      commaSep1(choice($._expression, $.starred_expression, $.yield_expression)),
      optional(','),
    ),

    dictionary: $ => seq(
      '{',
      optional(seq(commaSep1(choice($.pair, alias($._double_starred_expression, $.splat_argument))), optional(','))),
      '}',
    ),

    pair: $ => seq(field('key', $._expression), ':', field('value', $._expression)),

    list_comprehension: $ => seq('[', $._comprehension_body, ']'),
    set_comprehension: $ => seq('{', $._comprehension_body, '}'),
    generator_expression: $ => seq('(', $._comprehension_body, ')'),
    dictionary_comprehension: $ => seq('{', field('body', $.pair), $._comprehension_clauses, '}'),

    _comprehension_body: $ => seq(
      field('body', $._expression),
      $._comprehension_clauses,
    ),

    _comprehension_clauses: $ => seq(
      $.for_in_clause,
      repeat(choice($.for_in_clause, $.if_clause)),
    ),

    for_in_clause: $ => prec.left(1, seq(
      optional('async'),
      'for',
      field('left', $._left_hand_side),
      'in',
      field('right', $._no_conditional_expression),
    )),

    if_clause: $ => prec.left(1, seq('if', $._no_conditional_expression)),

    // ── strings ──────────────────────────────────────────────────────
    // `"a" "b"` — adjacent literals concatenate, across lines too.
    concatenated_string: $ => prec.right(1, seq($.string, repeat1($.string))),

    string: $ => seq(
      $.string_start,
      repeat(choice(
        $.string_content,
        $.escape_sequence,
        $._interpolation,
      )),
      $.string_end,
    ),

    escape_sequence: _ => token.immediate(prec(1, choice(
      /\\N\{[^}\r\n]+\}/,
      // An ordinary escape, but NOT over a brace: CPython reads `\{` as a
      // literal backslash followed by an interpolation that still opens
      // (f"\{foo}" is Constant('\\') then FormattedValue(foo)), so
      // consuming the brace here loses the interpolation entirely.
      /\\[^\r\n{}]/,
      /\\\r?\n/,
      // The lone backslash of that case. Shorter than the alternative
      // above, so tree-sitter's longest-match rule still prefers `\n` etc.
      /\\/,
      /\{\{/,
      /\}\}/,
    ))),

    _interpolation: $ => choice($.interpolation),

    interpolation: $ => seq(
      token.immediate('{'),
      field('expression', choice($._expression, $._expression_list_tuple, $.yield_expression)),
      optional('='),
      optional(field('conversion', $.type_conversion)),
      optional(field('format', $.format_specifier)),
      '}',
    ),

    type_conversion: _ => /![rsa]/,

    // A format spec may span lines when the f-string is triple-quoted
    // (PEP 701 makes the whole interpolation multi-line there), so newlines
    // are content rather than a terminator. The closing brace is what ends
    // it, and braces are still excluded so a nested interpolation wins.
    format_specifier: $ => seq(
      // A LEXICAL precedence, not a parse one. After `f"{num"` both `:` and
      // `:=` are valid, and tree-sitter's longest-match rule handed the
      // walrus the colon: `f"{num:=10}"` parsed as `named_expression`, where
      // CPython gives a width-10 format spec (`'         5'`). Lexical
      // precedence outranks match length, so this `:` wins wherever a format
      // spec can start. It is a distinct token from the plain `:` used by
      // dicts, slices and annotations, so those are untouched, and a
      // PARENTHESISED walrus -- `f"{(x := 10)}"`, the only form CPython
      // accepts here -- still lexes `:=` because no format spec is valid
      // inside the parentheses.
      token(prec(1, ':')),
      repeat(choice(
        token.immediate(prec(1, /[^{}]+/)),
        $._interpolation,
      )),
    ),

    // ── literals ─────────────────────────────────────────────────────
    _literal: $ => choice(
      $.integer,
      $.float,
      $.true,
      $.false,
      $.none,
      $.ellipsis,
    ),

    integer: _ => token(choice(...v.integers)),

    float: _ => token(choice(
      /[0-9](_?[0-9])*\.([0-9](_?[0-9])*)?([eE][+-]?[0-9](_?[0-9])*)?[jJ]?/,
      /\.[0-9](_?[0-9])*([eE][+-]?[0-9](_?[0-9])*)?[jJ]?/,
      /[0-9](_?[0-9])*[eE][+-]?[0-9](_?[0-9])*[jJ]?/,
    )),

    true: _ => 'True',
    false: _ => 'False',
    none: _ => 'None',
    ellipsis: _ => '...',

    identifier: _ => /[_\p{XID_Start}][_\p{XID_Continue}]*/,

    // `[^\r\n]*`, not `.*`: tree-sitter's `.` excludes \n but MATCHES \r, so
    // on a CRLF file the comment swallowed the carriage return and every
    // comment node ended one byte late. Found by comparing against CPython's
    // `tokenize`, which is the only oracle we have below the tree.
    comment: _ => token(seq('#', /[^\r\n]*/)),
  },
});

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
