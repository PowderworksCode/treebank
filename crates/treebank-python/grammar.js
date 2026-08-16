/**
 * treebank-python: a from-scratch grammar for the union of Python 2.7 and
 * every Python 3, carrying the treebank vocabulary (DESIGN.md §3) in its
 * parse table.
 *
 * Threaded table-tier roles (18): _statement _expression _declaration
 * _pattern _name _literal _parameter _argument _member _clause? no — see
 * ledger. Actual list: see `supertypes` below; omissions and the reasons
 * for them are in ledger.json's roles_note.
 */

/// <reference types="tree-sitter-cli/dsl" />
// @ts-check

const tb = require('../treebank-core/vocabulary/supertypes.js');

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

module.exports = grammar({
  name: 'python',

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
    '_parameter',
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
  ]).map((name) => $[name]),

  conflicts: $ => [
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
    // The py2 statement keywords double as py3 names: `print(x)` is a call,
    // `print x` a statement; GLR keeps both, and the statements' negative
    // dynamic precedence yields to the expression reading when both parse.
    [$.print_statement, $._soft_keyword],
    [$.exec_statement, $._soft_keyword],
    [$.match_statement, $._soft_keyword],
    [$.type_alias_statement, $._soft_keyword],
    [$.exec_statement, $.comparison_expression],
    [$.exec_statement, $.conditional_expression, $.comparison_expression],
    [$.exec_statement, $.conditional_expression],
    // A conditional's consequence and a completed construct are
    // indistinguishable until the `if`/`else` arrives; GLR forks and the
    // reading without an `else` dies. Generate marks some of these
    // unnecessary, but with dynamic precedences in play its analysis is
    // unreliable — removing them broke `exec a in b, c` at runtime.
    [$.conditional_expression, $.expression_statement],
    [$.delete_statement, $.conditional_expression],
    [$.assert_statement, $.conditional_expression],
    [$.print_statement, $.conditional_expression],
    [$._right_hand_side, $.conditional_expression],
    [$.assignment, $.conditional_expression],
    [$.conditional_expression, $._comma_expressions],
    [$.type_alias_statement, $.conditional_expression],
    [$._case_patterns, $.conditional_expression],
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
      $.print_statement,
      $.exec_statement,
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

    augmented_assignment: $ => seq(
      field('left', $._left_hand_side),
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

    _right_hand_side: $ => choice(
      $._expression,
      $._expression_list_tuple,
      $.yield_expression,
      $.assignment,           // a = b = c
    ),

    // ── patterns (destructuring positions) ───────────────────────────
    _pattern: $ => choice(
      $.identifier,
      alias($._soft_keyword, $.identifier),
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

    return_statement: $ => prec.right(seq('return', optional(choice($._expression, $._expression_list_tuple, $.yield_expression)))),
    break_statement: _ => 'break',
    continue_statement: _ => 'continue',

    raise_statement: $ => prec.right(seq(
      'raise',
      optional(seq(
        $._expression,
        optional(choice(
          seq('from', field('cause', $._expression)),        // py3
          seq(',', $._expression, optional(seq(',', $._expression))), // py2
        )),
      )),
    )),

    delete_statement: $ => seq('del', choice($._expression, $._expression_list_tuple)),

    assert_statement: $ => seq('assert', $._expression, optional(seq(',', $._expression))),

    global_statement: $ => seq('global', commaSep1($._name)),
    nonlocal_statement: $ => seq('nonlocal', commaSep1($._name)),

    // Python 2. `print(x)` parses as a call (dynamic precedence below), so
    // this fires only for the forms py3 cannot read.
    print_statement: $ => prec.dynamic(-1, seq(
      'print',
      choice(
        seq('>>', $._expression, optional(seq(',', choice($._expression, $._expression_list_tuple)))),
        choice($._expression, $._expression_list_tuple),
      ),
    )),

    // PEP 695 `type X = int`; `type` stays a plain name everywhere else.
    type_alias_statement: $ => prec.dynamic(-1, seq(
      'type',
      field('name', $._name),
      field('type_parameters', optional($.type_parameters)),
      '=',
      field('value', $._expression),
    )),

    // The code operand is primary-tier so `exec code in g` needs no
    // reduce before the `in` — which would otherwise lose to the
    // comparison reading statically.
    exec_statement: $ => prec.dynamic(-1, seq(
      'exec',
      $._primary_expression,
      optional(seq('in', $._expression, optional(seq(',', $._expression)))),
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
        commaSep1(choice($.dotted_name, $.aliased_import)),
        seq('(', commaSep1(choice($.dotted_name, $.aliased_import)), optional(','), ')'),
      ),
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

    parameters: $ => seq('(', optional(seq(commaSep1($._parameter), optional(','))), ')'),

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

    _parameter: $ => choice(
      $.parameter,
      $.star_parameter,
      $.double_star_parameter,
      $.keyword_separator,
      $.positional_separator,
      $.tuple_parameter,        // py2: def f((a, b)):
    ),

    parameter: $ => seq(
      field('name', $._name),
      optional(seq(':', field('type', $._expression))),
      optional(seq('=', field('value', $._expression))),
    ),

    star_parameter: $ => seq('*', field('name', $._name), optional(seq(':', field('type', $._expression)))),
    double_star_parameter: $ => seq('**', field('name', $._name), optional(seq(':', field('type', $._expression)))),
    keyword_separator: _ => '*',
    positional_separator: _ => '/',
    tuple_parameter: $ => seq('(', commaSep1(choice($._name, $.tuple_parameter)), optional(','), ')'),

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
      optional(seq('if', field('guard', $._no_conditional_expression))),
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
        optional(choice(
          seq('as', field('alias', $._name)),   // both
          seq(',', field('alias', $._name)),    // py2
        )),
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
      seq($._statement, repeat(seq(';', $._statement)), optional(';'), $._newline),
      seq($._newline, $._indent, repeat1($._line), $._dedent),
    ),

    // ── expressions ──────────────────────────────────────────────────
    _expression: $ => choice(
      $.conditional_expression,
      $.starred_expression,
      $._no_conditional_expression,
    ),

    // Python's comprehension conditions and iterables are `or_test`: a bare
    // conditional is excluded by construction, which is also what lets
    // `... if a if b` chain instead of swallowing the second `if`.
    _no_conditional_expression: $ => choice(
      $.lambda,
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
      $.repr_expression,
    ),

    _name: $ => choice(
      $.identifier,
      alias($._soft_keyword, $.identifier),
    ),
    _soft_keyword: _ => choice('print', 'exec', 'match', 'case', 'type'),

    conditional_expression: $ => prec.right(PREC.conditional, seq(
      field('consequence', $._expression),
      'if',
      field('condition', $._expression),
      'else',
      field('alternative', $._expression),
    )),

    lambda: $ => prec(PREC.lambda, seq(
      'lambda',
      field('parameters', optional(alias($._lambda_parameters, $.parameters))),
      ':',
      field('body', $._expression),
    )),
    _lambda_parameters: $ => seq(commaSep1($._parameter), optional(',')),

    named_expression: $ => prec.right(PREC.walrus, seq(
      field('name', $._name),
      ':=',
      field('value', $._expression),
    )),

    boolean_expression: $ => choice(
      prec.left(PREC.or, seq(field('left', $._expression), field('operator', 'or'), field('right', $._expression))),
      prec.left(PREC.and, seq(field('left', $._expression), field('operator', 'and'), field('right', $._expression))),
    ),

    not_expression: $ => prec(PREC.not, seq(field('operator', 'not'), field('operand', $._expression))),

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
          '<', '<=', '>', '>=', '==', '!=', '<>',   // <> is py2
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
      commaSep1(field('subscript', choice($._expression, $.slice))),
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

    repr_expression: $ => seq('`', choice($._expression, $._expression_list_tuple), '`'),  // py2

    // A bare comma-joined expression list is a tuple in all but name.
    _expression_list_tuple: $ => alias($._comma_expressions, $.tuple),
    _comma_expressions: $ => prec.right(seq(
      $._expression,
      choice(
        ',',
        seq(repeat1(seq(',', $._expression)), optional(',')),
      ),
    )),

    // ── containers ───────────────────────────────────────────────────
    list: $ => seq('[', optional($._collection_elements), ']'),
    set: $ => seq('{', $._collection_elements, '}'),
    tuple: $ => seq('(', optional($._collection_elements), ')'),

    _collection_elements: $ => seq(
      commaSep1(choice($._expression, $.yield_expression)),
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
        $.interpolation,
      )),
      $.string_end,
    ),

    escape_sequence: _ => token.immediate(prec(1, choice(
      /\\N\{[^}\r\n]+\}/,
      /\\[^\r\n]/,
      /\\\r?\n/,
      /\{\{/,
      /\}\}/,
    ))),

    interpolation: $ => seq(
      token.immediate('{'),
      field('expression', choice($._expression, $._expression_list_tuple, $.yield_expression)),
      optional('='),
      optional(field('conversion', $.type_conversion)),
      optional(field('format', $.format_specifier)),
      '}',
    ),

    type_conversion: _ => /![rsa]/,

    format_specifier: $ => seq(
      ':',
      repeat(choice(
        token.immediate(prec(1, /[^{}\n]+/)),
        $.interpolation,
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

    integer: _ => token(choice(
      /0[xX][0-9a-fA-F](_?[0-9a-fA-F])*[lL]?/,
      /0[oO][0-7](_?[0-7])*[lL]?/,
      /0[bB][01](_?[01])*[lL]?/,
      /[0-9](_?[0-9])*[lLjJ]?/,
    )),

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

    comment: _ => token(seq('#', /.*/)),
  },
});

function commaSep1(rule) {
  return sep1(rule, ',');
}

function sep1(rule, separator) {
  return seq(rule, repeat(seq(separator, rule)));
}
