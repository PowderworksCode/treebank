/**
 * treebank-ruby: a from-scratch grammar for Ruby 3.x (accepting the older
 * spellings that cost nothing), carrying the treebank vocabulary
 * (DESIGN.md §3) in its parse table.
 *
 * Ruby is expression-oriented the way rust is — `x = if c then 1 else 2
 * end` is ordinary code — so `_control_flow` and its children thread
 * through the expression tier, and `_statement` reaches most of its
 * members through `_expression`. What ruby adds on top is LEXICAL
 * ambiguity: `a / b` against `a /b/`, `foo *args` against `a * b`,
 * `puts <<~EOS` against `a << b` — decisions the language's own lexer
 * makes from spacing and parser state. Those all live in the external
 * scanner, next to the string machinery (every delimited literal, and
 * heredocs, whose bodies begin after the line their operator is on).
 *
 * Omissions and the reasons for them are in ledger.toml's roles_note.
 */

/// <reference types="tree-sitter-cli/dsl" />
// @ts-check

const tb = require('../treebank-core/vocabulary/supertypes.js');

// Ruby's own operator ladder (parse.y), tightest first.
const PREC = {
  member: 40,       // a.b a[i] calls
  unary_not: 30,    // ! ~ unary +
  power: 28,        // ** (right)
  unary_minus: 26,  // -a  (below **: -2**2 is -(2**2))
  times: 24,        // * / %
  plus: 22,         // + -
  shift: 20,        // << >>
  bitand: 18,       // &
  bitor: 16,        // | ^
  compare: 14,      // < <= > >=
  equality: 12,     // == === != =~ !~ <=>
  and: 10,          // &&
  or: 8,            // ||
  range: 6,         // .. ...
  ternary: 4,       // ? :
  rescue_mod: 3,    // arg rescue arg — tighter than `=`, looser than ?:
  assign: 2,        // = += ...
  defined: 1,       // defined?
  not: 0,           // not
  and_or: -1,       // and or
  modifier: -2,     // stmt if c / stmt while c
  command: -3,      // paren-less call arguments bind loosest of all
  do_block: -4,     // `do` yields to a loop header's own `do`
};

module.exports = grammar({
  name: 'ruby',

  word: $ => $._identifier_token,

  // Ruby's hard keywords cannot be local variable reads: without this,
  // `end` at statement level lexed as an identifier (the keyword token is
  // only preferred where a keyword is VALID), and a stray `end` parsed —
  // the exact widening the negative corpus exists to catch. Positions that
  // genuinely admit keywords as names (`x.class`, `def if`) list them
  // explicitly and are unaffected.
  reserved: {
    global: $ => [
      'alias', 'and', 'begin', 'break', 'case', 'class', 'def', 'do',
      'else', 'elsif', 'end', 'ensure', 'false', 'for', 'if', 'in',
      'module', 'next', 'nil', 'not', 'or', 'redo', 'rescue', 'retry',
      'return', 'self', 'super', 'then', 'true', 'undef', 'unless',
      'until', 'when', 'while', 'yield',
    ],
  },

  extras: $ => [
    $.comment,
    // `=begin`/`=end` block comments, column-0-anchored — the anchor is
    // why they live in the scanner rather than in a token regex.
    $.block_comment,
    // The heredoc EXCEPTION to §4.1's extras rule, declared in ledger.toml:
    // a heredoc's body physically sits between the newline that ends its
    // operator's line and the next line, i.e. between tokens of unrelated
    // constructs. An extra is the only placement the parse table offers for
    // a node that can appear at any token boundary; the scanner only ever
    // produces one when a heredoc operator is actually pending.
    $.heredoc_body,
    /\s|\\\r?\n/,
  ],

  externals: $ => [
    $._line_break,
    $.string_start,
    $._symbol_start,
    $._subshell_start,
    $._regex_start,
    $._words_start,
    $._symbols_start,
    $.string_content,
    $.string_end,
    $.escape_sequence,
    $.heredoc_beginning,
    $._heredoc_body_start,
    $.heredoc_content,
    $.heredoc_end,
    $._hash_key,
    $._identifier_suffix,
    $._binary_star,
    $._splat_star,
    $._binary_star_star,
    $._splat_star_star,
    $._binary_amp,
    $._block_amp,
    $._binary_slash,
    $._binary_minus,
    $._unary_minus,
    $._binary_plus,
    $._unary_plus,
    $.block_comment,
    $.simple_symbol,
    $._error_sentinel,
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
    '_interpolation',
  ]).map((name) => $[name]),

  conflicts: $ => [
    [$.scope_resolution],
    [$.singleton_class, $.member_block],
    [$.range_expression],
    [$._statements, $.function_definition],
    [$.call_expression],
    [$.module_definition, $.member_block],
    [$.class_definition, $.member_block],
    [$._statements, $.do_block],
    [$._pattern, $._name, $._pattern_value],
    [$._pattern, $._access],
    [$._pattern, $._variable],
    [$._pattern, $._name],
    [$._name, $.scope_resolution],
    [$._primary, $.scope_resolution],
    [$._name, $.scope_resolution, $._callee],
    [$._primary, $.concatenated_string],
    [$._name, $.module_definition],
    [$._callee, $.module_definition],
    [$._name, $.class_definition],
    [$._callee, $.class_definition],
    [$._keyword_method_name, $.module_definition],
    [$.function_definition, $._keyword_method_name],
    [$.super, $._keyword_method_name],
    [$._name, $.pattern_splat],
    [$.array_pattern, $.array],
    [$.hash_pattern, $.hash],
    [$.pattern_unary, $._literal],
    [$._pattern, $.pattern_splat],
    [$._name, $.pattern_double_splat],
    [$.pattern_double_splat, $._literal],
    [$._jump_value, $._bare_list],
    [$.range_expression, $.forward_argument, $.pattern_range],
    [$.splat_argument, $.pattern_double_splat],
    [$.splat_argument, $.pattern_splat],
    [$._callee, $.array_pattern, $.hash_pattern],
    [$._name, $._callee, $._pattern_value],
    [$._name, $._pattern_value],
    [$._callee, $.array_pattern],
    [$._pattern, $._pattern_value],
    [$.range_expression, $.pattern_range],
    [$.star_pattern, $.pattern_splat],
    [$._primary, $._pattern_value],
    [$.pattern_double_splat, $.hash_splat],
    [$.pattern_pair, $.pair],
    [$._right_hand_side, $._bare_list],
    [$._callee, $.hash_pattern],
    [$._parameter, $.tuple_parameter],
    [$._argument, $.pair],
    [$._primary, $.function_definition],
    [$._name, $.function_definition],
    [$._keyword_method_name, $.case_statement],
    [$._keyword_method_name, $.for_statement],
    [$._keyword_method_name, $.begin_statement],
    [$._keyword_method_name, $.class_definition],
    [$.yield_expression, $._keyword_method_name],
    [$._keyword_method_name, $.until_statement],
    [$._keyword_method_name, $.while_statement],
    [$._keyword_method_name, $.unless_statement],
    [$._keyword_method_name, $.if_statement],
    [$._access, $._callee],
    [$._primary, $._callee],
    [$._name, $._callee],],

  rules: {
    program: $ => seq(
      optional($._directive),
      optional($._statements),
      optional($.uninterpreted),
    ),

    // `#!/usr/bin/env ruby`. Ruby's only piece of syntax that addresses the
    // environment rather than computing in it — require/require_relative
    // are ordinary method calls, which is why `_directive` has one member.
    _directive: $ => choice($.shebang),
    shebang: _ => token(prec(2, seq('#!', /[^\r\n]*/))),

    // `__END__` and everything after it: the DATA section.
    uninterpreted: _ => token(seq('__END__', /\r?\n/, /(.|\s)*/)),

    // Statements separated by newlines the scanner decides are terminators
    // (see _line_break in scanner.c) or by `;`. A terminator is an item of
    // its own, not a required suffix of a statement: a region of nothing
    // but blank lines and comments (a comment-only file, `class C\n# note
    // \nend`) still matches, where requiring a statement after a leading
    // terminator committed the parser to one that never came — that single
    // shape error broke 141 of the first thousand stdlib files.
    _statements: $ => choice(
      seq(
        repeat1(choice(seq($._statement, $._terminator), $._terminator)),
        optional($._statement),
      ),
      $._statement,
    ),

    _terminator: $ => choice(';', $._line_break),

    // ── the statement tier ───────────────────────────────────────────
    // Almost everything is an expression, so `_statement → _expression`
    // is a derivation chain, not a wrapper: one occurrence answers both
    // queries, which is DESIGN.md §2 fact 2 working as intended.
    _statement: $ => choice(
      $._expression,
      $.if_modifier,
      $.unless_modifier,
      $.while_modifier,
      $.until_modifier,
      $.rescue_modifier,
      $.alias_statement,
      $.undef_statement,
      $.begin_block,
      $.end_block,
    ),

    // Statement modifiers. These are NOT threaded through `_branch`/`_loop`,
    // and the reason is the same one python's roles.json records for its
    // match shapes: a supertype's members enter every position that
    // references it, `_branch` is reachable from argument position (where
    // `x = if c then a end` is legal), and a modifier there — `foo(1 if c)`
    // — is a SyntaxError from CRuby. Roles are per-grammar-position facts.
    if_modifier: $ => prec.left(PREC.modifier, seq(
      field('body', $._statement),
      'if',
      field('condition', $._expression),
    )),
    unless_modifier: $ => prec.left(PREC.modifier, seq(
      field('body', $._statement),
      'unless',
      field('condition', $._expression),
    )),
    while_modifier: $ => prec.left(PREC.modifier, seq(
      field('body', $._statement),
      'while',
      field('condition', $._expression),
    )),
    until_modifier: $ => prec.left(PREC.modifier, seq(
      field('body', $._statement),
      'until',
      field('condition', $._expression),
    )),
    rescue_modifier: $ => prec.left(PREC.rescue_mod, seq(
      field('body', choice($._statement)),
      'rescue',
      field('handler', $._expression),
    )),

    alias_statement: $ => seq(
      'alias',
      field('name', $._method_name),
      field('alias', $._method_name),
    ),

    undef_statement: $ => seq('undef', commaSep1($._method_name)),

    _method_name: $ => choice(
      $.identifier,
      $.constant,
      $.simple_symbol,
      $.global_variable,
      alias($._operator_token, $.operator),
      alias($._keyword_method_name, $.identifier), // alias nil? empty?
      $.setter_method_name,
    ),

    begin_block: $ => seq('BEGIN', '{', optional($._statements), '}'),
    end_block: $ => seq('END', '{', optional($._statements), '}'),

    // ── the expression tier ──────────────────────────────────────────
    // parse.y's `expr`: the operator ladder (`_arg`), plus the forms that
    // only exist at this looser level — paren-less command calls, the
    // keyword logical operators, and the jumps.
    _expression: $ => choice(
      $._arg,
      alias($.command_call, $.call_expression),
      alias($._and_or, $.binary_expression),
      alias($.not_expression, $.unary_expression),
      $._jump,
      alias($.multiple_assignment, $.assignment),
      $.match_pattern,
    ),

    // One-line pattern matching: `opcode => :je | :jne` deconstructs (and
    // raises on mismatch); `x in pattern` tests. Expression-level only,
    // like ruby's own grammar. Dynamically penalised so that where a `=>`
    // also reads as a hash-rocket pair — `scanner :ruby, :tokens => @x` —
    // the pair wins; the pattern match is the reading of last resort.
    match_pattern: $ => prec(1, prec.dynamic(-1, seq(
      field('value', $._arg),
      field('operator', choice('=>', 'in')),
      field('pattern', choice(
        $._case_pattern,
        alias($.pattern_top_list, $.array_pattern),
      )),
    ))),

    _and_or: $ => prec.left(PREC.and_or, seq(
      field('left', $._expression),
      field('operator', choice('and', 'or')),
      field('right', $._expression),
    )),

    not_expression: $ => prec(PREC.not, seq(
      field('operator', 'not'),
      field('operand', $._expression),
    )),

    // ── jumps ────────────────────────────────────────────────────────
    // raise and throw are NOT here: both are Kernel methods, not syntax,
    // and they parse as the calls they are.
    _jump: $ => choice(
      $.return_statement,
      $.break_statement,
      $.next_statement,
      $.redo_statement,
      $.retry_statement,
    ),

    // prec.LEFT, deliberately: at `break • if` the parser must reduce the
    // bare jump so the `if` becomes a MODIFIER — the same choice CRuby's
    // lexer makes by entering EXPR_MID after these keywords. The conflict
    // only exists on the modifier keywords (nothing else in a jump's
    // follow set can also start a value), so `break 1` still shifts.
    return_statement: $ => prec.left(seq('return', optional($._jump_value))),
    break_statement: $ => prec.left(seq('break', optional($._jump_value))),
    next_statement: $ => prec.left(seq('next', optional($._jump_value))),
    redo_statement: _ => prec.left('redo'),
    retry_statement: _ => prec.left('retry'),

    _jump_value: $ => choice(
      $._arg,
      alias($.command_call, $.call_expression),
      alias($._bare_list, $.array),
      $.splat_argument,
    ),

    // ── assignment ───────────────────────────────────────────────────
    _assignment: $ => choice($.assignment, $.augmented_assignment),

    assignment: $ => prec.right(PREC.assign, seq(
      field('left', $._pattern),
      '=',
      field('right', $._right_hand_side),
    )),

    // `a, b = 1, 2` — a bare comma-joined target list is only legal at the
    // expression level, never as an operand, which is why it is a separate
    // rule threaded from `_expression` and aliased to the same node.
    // A lone splat (`*values = …`) and a parenthesised list
    // (`(y, m, d) = …`) are multiple assignments too.
    multiple_assignment: $ => prec.right(PREC.assign, seq(
      field('left', choice($.pattern_list, $.star_pattern, $.tuple_pattern)),
      '=',
      field('right', $._right_hand_side),
    )),

    _right_hand_side: $ => choice(
      $._arg,
      alias($.command_call, $.call_expression),
      alias($._bare_list, $.array),
      $.splat_argument,
      $._jump,
    ),

    // The right side of `x = 1, 2` collects into an Array in all but
    // spelling, exactly as python's bare `1, 2` is a tuple in all but
    // parentheses — the same alias trick, for the same reason.
    _bare_list: $ => prec.right(seq(
      choice($._arg, $.splat_argument),
      repeat1(seq(',', choice($._arg, $.splat_argument))),
    )),

    augmented_assignment: $ => prec.right(PREC.assign, seq(
      field('left', $._augmented_target),
      field('operator', choice(
        '+=', '-=', '*=', '/=', '%=', '**=', '<<=', '>>=',
        '&&=', '||=', '&=', '|=', '^=',
      )),
      field('right', $._right_hand_side),
    )),

    _augmented_target: $ => choice(
      $.identifier,
      $.constant,
      $.instance_variable,
      $.class_variable,
      $.global_variable,
      $.member_expression,
      $.subscript_expression,
      $.scope_resolution,
    ),

    // ── patterns (assignment targets) ────────────────────────────────
    _pattern: $ => choice(
      $.identifier,
      $.constant,
      $.instance_variable,
      $.class_variable,
      $.global_variable,
      $.member_expression,
      $.subscript_expression,
      $.scope_resolution,
    ),

    pattern_list: $ => prec.right(seq(
      choice($._pattern, $.star_pattern, $.tuple_pattern),
      choice(
        ',',
        seq(
          repeat1(seq(',', choice($._pattern, $.star_pattern, $.tuple_pattern))),
          optional(','),
        ),
      ),
    )),

    star_pattern: $ => seq($._splat_star, optional($._pattern)),

    // `a, (b, c) = …`, and the single-element nesting `_, ((x, y)) = s`.
    tuple_pattern: $ => seq(
      '(',
      choice($.pattern_list, $._pattern, $.star_pattern, $.tuple_pattern),
      ')',
    ),

    // ── the operator ladder (parse.y's `arg`) ────────────────────────
    _arg: $ => choice(
      $._assignment,
      $.conditional_expression,
      $.range_expression,
      $.binary_expression,
      $.unary_expression,
      $.defined_expression,
      alias($.arg_rescue_modifier, $.rescue_modifier),
      $._primary,
    ),

    // `x = y rescue z` — the modifier exists INSIDE the ladder too, binding
    // tighter than `=` (CRuby: the whole right side is rescued).
    arg_rescue_modifier: $ => prec.left(PREC.rescue_mod, seq(
      field('body', $._arg),
      'rescue',
      field('handler', $._arg),
    )),

    conditional_expression: $ => prec.right(PREC.ternary, seq(
      field('condition', $._arg),
      '?',
      field('consequence', $._arg),
      ':',
      field('alternative', $._arg),
    )),

    range_expression: $ => prec.left(PREC.range, seq(
      field('left', optional($._arg)),
      field('operator', choice('..', '...')),
      field('right', optional($._arg)),
    )),

    binary_expression: $ => {
      const table = [
        ['||', PREC.or], ['&&', PREC.and],
        ['==', PREC.equality], ['!=', PREC.equality], ['===', PREC.equality],
        ['=~', PREC.equality], ['!~', PREC.equality], ['<=>', PREC.equality],
        ['<', PREC.compare], ['<=', PREC.compare],
        ['>', PREC.compare], ['>=', PREC.compare],
        ['|', PREC.bitor], ['^', PREC.bitor],
        ['<<', PREC.shift], ['>>', PREC.shift],
      ];
      return choice(
        ...table.map(([op, p]) => prec.left(p, seq(
          field('left', $._arg),
          field('operator', op),
          field('right', $._arg),
        ))),
        // The spacing-ambiguous operators come from the scanner, which is
        // what keeps `a * b` a product while `foo *args` splats: the
        // grammar sees two different tokens for the same character.
        prec.left(PREC.times, seq(field('left', $._arg), field('operator', alias($._binary_star, '*')), field('right', $._arg))),
        prec.left(PREC.times, seq(field('left', $._arg), field('operator', alias($._binary_slash, '/')), field('right', $._arg))),
        prec.left(PREC.times, seq(field('left', $._arg), field('operator', '%'), field('right', $._arg))),
        prec.left(PREC.plus, seq(field('left', $._arg), field('operator', alias($._binary_plus, '+')), field('right', $._arg))),
        prec.left(PREC.plus, seq(field('left', $._arg), field('operator', alias($._binary_minus, '-')), field('right', $._arg))),
        prec.left(PREC.bitand, seq(field('left', $._arg), field('operator', alias($._binary_amp, '&')), field('right', $._arg))),
        prec.right(PREC.power, seq(field('left', $._arg), field('operator', alias($._binary_star_star, '**')), field('right', $._arg))),
      );
    },

    unary_expression: $ => choice(
      prec(PREC.unary_not, seq(field('operator', choice('!', '~')), field('operand', $._arg))),
      prec(PREC.unary_minus, seq(field('operator', alias($._unary_minus, '-')), field('operand', $._arg))),
      prec(PREC.unary_not, seq(field('operator', alias($._unary_plus, '+')), field('operand', $._arg))),
    ),

    defined_expression: $ => prec(PREC.defined, seq('defined?', $._arg)),

    // ── primaries ────────────────────────────────────────────────────
    _primary: $ => choice(
      $._name,
      $._variable,
      $.self,
      $.super,
      $._literal,
      $.string,
      $.concatenated_string,
      $.quoted_symbol,
      $.subshell,
      $.regex,
      $.string_array,
      $.symbol_array,
      $.heredoc_beginning,
      $.array,
      $.hash,
      $.parenthesized_expression,
      $._invocation,
      $._access,
      $._control_expression,
      $._declaration,
      $.lambda,
      $.yield_expression,
    ),

    // `_name` is the table-tier role for a name IN a naming position; the
    // occurrences here are uses of names as values, which is fact 4's
    // occurrence semantics doing the work.
    _name: $ => choice(
      $.identifier,
      $.constant,
      $.scope_resolution,
    ),

    _variable: $ => choice(
      $.instance_variable,
      $.class_variable,
      $.global_variable,
    ),

    self: _ => 'self',
    super: _ => 'super',

    // The scope is a constant chain (or self), NOT a general primary: with
    // `_primary` here, every `_name` position — a def's name, a module
    // header — also expected every literal-start token, and `def /(x)`
    // lexed its operator as a regex. `expr::CONST` on a computed receiver
    // is rare enough to ledger.
    //
    // prec.dynamic, and no static precedence: statically preferring the
    // shift here starved the member_expression reading of `Time::now`
    // entirely — with a constant on the left the parser could never reduce
    // toward a lowercase property. GLR keeps both; the weight makes `A::B`
    // a scope_resolution where both complete.
    scope_resolution: $ => prec.left(prec.dynamic(1, seq(
      optional(field('scope', choice($.constant, $.scope_resolution, $.self))),
      '::',
      field('name', $.constant),
    ))),

    parenthesized_expression: $ => prec(PREC.member, seq(
      '(',
      optional($._statements),
      ')',
    )),

    // ── access ───────────────────────────────────────────────────────
    _access: $ => choice($.member_expression, $.subscript_expression),

    // `obj.attr` with no arguments and no parentheses: syntactically a
    // read, whatever it dispatches to at run time. `::` reaches methods
    // too (`Util::make_components_hash(...)`), but only lowercase ones
    // here — a capitalised `A::B` is the scope_resolution.
    member_expression: $ => prec(PREC.member, choice(
      seq(
        field('object', $._primary),
        field('operator', choice('.', '&.')),
        field('property', choice(
          $.identifier,
          $.constant,
          alias($._keyword_method_name, $.identifier),
          alias($._operator_token, $.operator),
        )),
      ),
      seq(
        field('object', $._primary),
        field('operator', '::'),
        // A constant property is allowed — `typeclass::TypeValue` reads a
        // constant off a computed receiver — and where the whole chain is
        // constants, scope_resolution's dynamic weight wins instead.
        field('property', choice(
          $.identifier,
          $.constant,
          alias($._keyword_method_name, $.identifier),
        )),
      ),
    )),

    subscript_expression: $ => prec(PREC.member, seq(
      field('object', $._primary),
      token.immediate('['),
      optional(seq(commaSep1(field('subscript', $._argument)), optional(','))),
      ']',
    )),

    // ── invocation ───────────────────────────────────────────────────
    _invocation: $ => choice($.call_expression),

    // The do-block alternatives take LOW precedence so that in `for i in
    // 0...n do`, the `do` belongs to the loop: at `n • do` the parser can
    // either reduce the loop's iterable or shift into a block on `n`, both
    // ways make a complete program, and the wrong one silently eats an
    // `end`. `do` binds to the nearest loop/while, `{` to the nearest
    // call — ruby's own rule, spelled as precedence. Calls not in a loop
    // header never see the conflict (`do` follows nothing else there).
    call_expression: $ => choice(
      // foo(args) / obj.foo(args) — parentheses, then optionally a block.
      prec(PREC.member, seq(
        field('function', $._callee),
        field('arguments', $.argument_list),
        optional(field('block', $.brace_block)),
      )),
      prec(PREC.do_block, prec.dynamic(-1, seq(
        field('function', $._callee),
        field('arguments', $.argument_list),
        field('block', $.do_block),
      ))),
      // foo { } / obj.foo do end — no arguments; the block is what makes
      // this a call rather than a bare name or member read.
      prec(PREC.member, seq(
        field('function', $._callee),
        field('block', $.brace_block),
      )),
      prec(PREC.do_block, prec.dynamic(-1, seq(
        field('function', $._callee),
        field('block', $.do_block),
      ))),
      // `callable.(args)` — call with the method name omitted, sugar for
      // .call. The dot is what distinguishes it from a plain paren call.
      prec(PREC.member, seq(
        field('function', $._primary),
        choice('.', '&.'),
        field('arguments', $.argument_list),
        optional(field('block', choice($.brace_block, $.do_block))),
      )),
      // obj.foo= v is an assignment, handled there; obj.foo v is a command,
      // aliased into this node from _expression.
    ),

    _callee: $ => choice(
      $.identifier,
      $.constant,
      $.member_expression,
      $.scope_resolution,
      $.super,
    ),

    // A paren-less call with arguments: `puts x, y`, `attr_reader :a, :b`,
    // `obj.write data`. Only legal at the expression level — an argument
    // cannot itself be a bare command — which is the entire reason the
    // expression/arg split exists in ruby's own grammar.
    command_call: $ => choice(
      prec.right(PREC.command, seq(
        field('function', $._callee),
        field('arguments', alias($.command_argument_list, $.argument_list)),
      )),
      prec.right(PREC.do_block, prec.dynamic(-1, seq(
        field('function', $._callee),
        field('arguments', alias($.command_argument_list, $.argument_list)),
        field('block', $.do_block),
      ))),
    ),

    // A parenthesised list may hold ONE command — `new(sanitize_exception
    // e)` — which is ruby's own rule: command_args reach args only through
    // parens or as the whole list.
    argument_list: $ => prec(PREC.member, seq(
      token.immediate('('),
      optional(choice(
        seq(commaSep1($._argument), optional(',')),
        alias($.command_call, $.call_expression),
      )),
      ')',
    )),

    // The LAST argument of a command may itself be a command — `foo bar
    // baz` is foo(bar(baz)), `assert_equal x, compute y` passes a nested
    // call — which is also what lets `return render json: x` parse.
    command_argument_list: $ => prec.right(choice(
      seq(
        commaSep1($._argument),
        optional(seq(',', alias($.command_call, $.call_expression))),
      ),
      alias($.command_call, $.call_expression),
    )),

    // prec.left for the jump-keyword reason above: at `log • if` the bare
    // identifier reduces and the `if` is a modifier, never a command whose
    // first argument is an if-expression.
    _argument: $ => prec.left(choice(
      $._arg,
      $.splat_argument,
      $.block_argument,
      $.forward_argument,
      $.pair,
    )),

    splat_argument: $ => prec.right(seq(
      choice(alias($._splat_star, '*'), alias($._splat_star_star, '**')),
      optional($._arg),
    )),

    block_argument: $ => prec.right(seq(alias($._block_amp, '&'), optional($._arg))),

    forward_argument: _ => '...',

    yield_expression: $ => prec.right(seq(
      'yield',
      optional(choice(
        field('arguments', $.argument_list),
        field('arguments', alias($.command_argument_list, $.argument_list)),
      )),
    )),

    // ── blocks ───────────────────────────────────────────────────────
    // The parameters may sit on their own line below the opener.
    do_block: $ => seq(
      'do',
      optional($._terminator),
      optional(field('parameters', $.block_parameters)),
      optional(field('body', $._body)),
      repeat($.rescue_clause),
      optional($.else_clause),
      optional($.ensure_clause),
      'end',
    ),

    brace_block: $ => prec(PREC.member, seq(
      '{',
      optional($._terminator),
      optional(field('parameters', $.block_parameters)),
      optional(field('body', $._body)),
      '}',
    )),

    block_parameters: $ => seq(
      '|',
      optional(seq(commaSep1($._parameter), optional(','))),
      optional(seq(';', commaSep1(alias($.identifier, $.parameter)))),
      '|',
    ),

    lambda: $ => seq(
      '->',
      // `->(x)`, `-> (x)` and `-> x` all occur; after `->` a plain paren
      // is unambiguous, so the spaced form gets its own alternative here
      // where def-parameters cannot have one.
      field('parameters', optional(choice(
        alias($.lambda_parameters, $.parameters),
        alias($.spaced_lambda_parameters, $.parameters),
        alias($.bare_parameters, $.parameters),
      ))),
      field('body', choice($.brace_block, $.do_block)),
    ),

    spaced_lambda_parameters: $ => seq('(', optional(seq(commaSep1($._parameter), optional(','))), ')'),

    // token.immediate: the same decision ruby's lexer makes. `def f(a)` and
    // `->(x)` open parameters; `def f (a)` (warned by CRuby) falls back to
    // the bare-parameter list, where `(a)` reads as a destructuring — a
    // ledgered mis-shape of a form the language itself warns about.
    lambda_parameters: $ => seq(token.immediate('('), optional(seq(commaSep1($._parameter), optional(','))), ')'),
    bare_parameters: $ => commaSep1($._parameter),

    // ── parameters ───────────────────────────────────────────────────
    // One alternation: ruby orders its list (required, optional, rest,
    // post, keywords, block) but the ordering is not yet spelled out here.
    // ledger.toml records that as a known widening, with python's
    // parameterRules chain as the shape of the fix.
    _parameter: $ => choice(
      $.parameter,
      alias($.optional_parameter, $.parameter),
      $.star_parameter,
      $.double_star_parameter,
      $.block_parameter,
      $.keyword_parameter,
      $.forward_parameter,
      $.tuple_parameter,
    ),

    parameter: $ => field('name', $.identifier),

    optional_parameter: $ => prec(1, seq(
      field('name', $.identifier),
      '=',
      field('value', $._arg),
    )),

    star_parameter: $ => seq(
      alias($._splat_star, '*'),
      optional(field('name', $.identifier)),
    ),

    double_star_parameter: $ => seq(
      alias($._splat_star_star, '**'),
      optional(field('name', choice($.identifier, $.nil))),
    ),

    block_parameter: $ => seq(
      alias($._block_amp, '&'),
      optional(field('name', $.identifier)),
    ),

    keyword_parameter: $ => prec.right(seq(
      field('name', alias($._hash_key, $.hash_key_symbol)),
      optional(field('value', $._arg)),
    )),

    forward_parameter: _ => '...',

    tuple_parameter: $ => seq(
      '(',
      commaSep1(choice($.parameter, $.star_parameter, $.tuple_parameter)),
      ')',
    ),

    // ── declarations ─────────────────────────────────────────────────
    _declaration: $ => choice(
      $.function_definition,
      $.class_definition,
      $.module_definition,
      $.singleton_class,
    ),

    // `def name`, `def self.name`, `def obj.name`, endless `def name = e`.
    // One node for all of them: the receiver is a field, not a different
    // construct, and `(function_definition name: (_name))` stays one query.
    function_definition: $ => seq(
      'def',
      optional(seq(
        field('object', choice($.self, $.identifier, $.constant, $._variable)),
        choice('.', '::'),
      )),
      field('name', choice(
        $._name,
        alias($._operator_token, $.operator),
        $.setter_method_name,
        alias($._keyword_method_name, $.identifier),
      )),
      choice(
        // Parenthesised parameters need no separator before the body —
        // `def f(a) end` is valid to CRuby; `def f a end` and `def f end`
        // are not, so the bare and empty forms keep the requirement.
        seq(
          choice(
            seq(
              field('parameters', alias($.lambda_parameters, $.parameters)),
              optional($._terminator),
            ),
            seq(
              field('parameters', optional(alias($.def_bare_parameters, $.parameters))),
              $._terminator,
            ),
          ),
          optional(field('body', $._body)),
          repeat($.rescue_clause),
          optional($.else_clause),
          optional($.ensure_clause),
          'end',
        ),
        // Endless (3.0): the body is one expression, no `end`.
        seq(
          field('parameters', optional(alias($.lambda_parameters, $.parameters))),
          '=',
          field('body', $._arg),
        ),
      ),
    ),

    def_bare_parameters: $ => seq(commaSep1($._parameter)),

    // `def x=(value)`: the `=` is part of the name, and only when it abuts
    // it — `def x = v` with space is an endless method named x.
    setter_method_name: $ => seq($.identifier, token.immediate(prec(1, '='))),

    _operator_token: _ => choice(
      '+', '-', '*', '/', '%', '**', '==', '===', '!=', '<', '<=', '>', '>=',
      '<=>', '=~', '!~', '<<', '>>', '&', '|', '^', '!', '~', '+@', '-@',
      '[]', '[]=', '`',
    ),

    // Ruby lets every keyword be a method name after `def` or `.`, and the
    // suffix reaches them too: `x.nil?` is the single most common method
    // in the language whose name is a keyword plus `?`.
    _keyword_method_name: $ => seq(
      choice(
        'class', 'module', 'def', 'begin', 'end', 'if', 'unless', 'while',
        'until', 'for', 'case', 'when', 'in', 'do', 'then', 'else', 'elsif',
        'ensure', 'rescue', 'yield', 'super', 'self', 'nil', 'true', 'false',
        'and', 'or', 'not', 'return', 'break', 'next', 'redo', 'retry',
        'alias', 'undef', 'defined?', 'new',
      ),
      optional($._identifier_suffix),
    ),

    // The terminator after the name is optional because the body carries
    // its own: `module English end if false` and even `class C x = 1 end`
    // are valid to CRuby.
    class_definition: $ => seq(
      'class',
      field('name', choice($.constant, $.scope_resolution)),
      optional(seq('<', field('superclass', $._arg))),
      optional($._terminator),
      optional(field('body', alias($.member_block, $.block))),
      'end',
    ),

    module_definition: $ => seq(
      'module',
      field('name', choice($.constant, $.scope_resolution)),
      optional($._terminator),
      optional(field('body', alias($.member_block, $.block))),
      'end',
    ),

    // `class << obj` — reopening a singleton class.
    singleton_class: $ => seq(
      'class',
      '<<',
      field('value', $._arg),
      optional($._terminator),
      optional(field('body', alias($.member_block, $.block))),
      'end',
    ),

    // A class or module body is the same statement sequence threaded
    // through `_member`, so `(_member)` matches exactly the statements
    // that are members. Aliased to `block` so trees stay uniform.
    member_block: $ => choice(
      seq(
        repeat1(choice(seq($._member, $._terminator), $._terminator)),
        optional($._member),
      ),
      $._member,
    ),

    _member: $ => choice($._statement),

    // ── bodies ───────────────────────────────────────────────────────
    _body: $ => choice($.block),

    block: $ => $._statements,

    // ── control flow ─────────────────────────────────────────────────
    // `_control_flow` itself is NOT declared, exactly as python omits it:
    // ruby's branches and loops are values (`x = if c then 1 end`) but its
    // jumps are not (`x = break` is a SyntaxError from CRuby), so the
    // umbrella term cannot contain `_jump` without accepting those — and
    // the vocabulary's containment rule rightly refuses an umbrella that
    // does not contain its parts.
    _control_expression: $ => choice(
      $._branch,
      $._loop,
      $.begin_statement,
    ),

    _branch: $ => choice($.if_statement, $.unless_statement, $.case_statement),

    if_statement: $ => seq(
      'if',
      field('condition', $._expression),
      $._then,
      optional(field('body', $._body)),
      repeat(field('alternative', $.elsif_clause)),
      optional(field('alternative', $.else_clause)),
      'end',
    ),

    unless_statement: $ => seq(
      'unless',
      field('condition', $._expression),
      $._then,
      optional(field('body', $._body)),
      optional(field('alternative', $.else_clause)),
      'end',
    ),

    elsif_clause: $ => seq(
      'elsif',
      field('condition', $._expression),
      $._then,
      optional(field('body', $._body)),
    ),

    else_clause: $ => seq('else', optional(field('body', $._body))),

    _then: $ => choice(
      'then',
      seq($._terminator, optional('then')),
    ),

    case_statement: $ => seq(
      'case',
      optional(field('value', $._expression)),
      repeat($._terminator),
      choice(
        seq(repeat1($.when_clause), optional($.else_clause)),
        seq(repeat1($.in_clause), optional($.else_clause)),
      ),
      'end',
    ),

    when_clause: $ => seq(
      'when',
      commaSep1(field('value', choice($._arg, $.splat_argument))),
      $._then,
      optional(field('body', $._body)),
    ),

    in_clause: $ => seq(
      'in',
      // `in Type::Nil, _` — a top-level comma list is an array pattern
      // without its brackets, so it is aliased to the same node.
      field('pattern', choice(
        $._case_pattern,
        alias($.pattern_top_list, $.array_pattern),
      )),
      optional(choice(
        seq('if', field('guard', $._expression)),
        seq('unless', field('guard', $._expression)),
      )),
      $._then,
      optional(field('body', $._body)),
    ),

    pattern_top_list: $ => seq(
      choice($._case_pattern, alias($.pattern_splat, $.star_pattern)),
      repeat1(seq(',', choice($._case_pattern, alias($.pattern_splat, $.star_pattern)))),
      optional(','),
    ),

    // ── case/in patterns (ruby 3 pattern matching) ───────────────────
    // The shapes share node names with python's match patterns wherever
    // the construct matches (or_pattern, as_pattern, array/hash patterns),
    // for the same reason python shares them with its destructuring side.
    _case_pattern: $ => choice(
      $._pattern_expr,
      $.as_pattern,
      $.or_pattern,
    ),

    as_pattern: $ => seq(
      field('pattern', $._pattern_expr),
      '=>',
      field('alias', $.identifier),
    ),

    or_pattern: $ => prec.left(seq(
      $._pattern_expr,
      repeat1(seq('|', $._pattern_expr)),
    )),

    _pattern_expr: $ => choice(
      $._pattern_value,
      $.array_pattern,
      $.hash_pattern,
      $.pin_pattern,
    ),

    _pattern_value: $ => choice(
      $._literal,
      $.string,
      $.quoted_symbol,
      $.regex,
      $.string_array,
      $.symbol_array,
      $.heredoc_beginning,
      $.identifier,
      $.constant,
      $.scope_resolution,
      $.self,
      alias($.pattern_range, $.range_expression),
      alias($.pattern_unary, $.unary_expression),
    ),

    pattern_range: $ => prec.left(PREC.range, seq(
      field('left', optional($._pattern_value)),
      field('operator', choice('..', '...')),
      field('right', optional($._pattern_value)),
    )),

    pattern_unary: $ => seq(
      field('operator', alias($._unary_minus, '-')),
      field('operand', choice($.integer, $.float)),
    ),

    array_pattern: $ => seq(
      optional(field('class', choice($.constant, $.scope_resolution))),
      choice(
        seq('[', optional(seq(commaSep1($._array_pattern_element), optional(','))), ']'),
        seq('(', optional(seq(commaSep1($._array_pattern_element), optional(','))), ')'),
      ),
    ),

    _array_pattern_element: $ => choice(
      $._case_pattern,
      alias($.pattern_splat, $.star_pattern),
    ),

    pattern_splat: $ => seq(
      choice('*', alias($._splat_star, '*')),
      optional($.identifier),
    ),

    hash_pattern: $ => seq(
      optional(field('class', choice($.constant, $.scope_resolution))),
      choice(
        seq('{', optional(seq(commaSep1($._hash_pattern_element), optional(','))), '}'),
        seq('[', seq(commaSep1($._hash_pattern_element), optional(',')), ']'),
      ),
    ),

    _hash_pattern_element: $ => choice(
      alias($.pattern_pair, $.pair),
      alias($.pattern_double_splat, $.splat_argument),
    ),

    pattern_pair: $ => prec.right(seq(
      field('key', alias($._hash_key, $.hash_key_symbol)),
      optional(field('value', $._case_pattern)),
    )),

    pattern_double_splat: $ => seq(
      choice('**', alias($._splat_star_star, '**')),
      optional(choice($.identifier, $.nil)),
    ),

    pin_pattern: $ => seq(
      '^',
      choice(
        $.identifier,
        $.instance_variable,
        $.class_variable,
        $.global_variable,
        $.parenthesized_expression,
      ),
    ),

    // ── loops ────────────────────────────────────────────────────────
    _loop: $ => choice($.while_statement, $.until_statement, $.for_statement),

    while_statement: $ => seq(
      'while',
      field('condition', $._expression),
      $._do_or_terminator,
      optional(field('body', $._body)),
      'end',
    ),

    until_statement: $ => seq(
      'until',
      field('condition', $._expression),
      $._do_or_terminator,
      optional(field('body', $._body)),
      'end',
    ),

    for_statement: $ => seq(
      'for',
      field('left', choice($._pattern, $.pattern_list)),
      'in',
      field('right', $._arg),
      $._do_or_terminator,
      optional(field('body', $._body)),
      'end',
    ),

    _do_or_terminator: $ => choice(
      'do',
      seq($._terminator, optional('do')),
    ),

    // ── begin / rescue / ensure ──────────────────────────────────────
    begin_statement: $ => seq(
      'begin',
      optional(field('body', $._body)),
      repeat($.rescue_clause),
      optional($.else_clause),
      optional($.ensure_clause),
      'end',
    ),

    rescue_clause: $ => seq(
      'rescue',
      optional(field('exceptions', alias($.rescue_exceptions, $.argument_list))),
      optional(seq('=>', field('alias', $._pattern))),
      $._then,
      optional(field('body', $._body)),
    ),

    rescue_exceptions: $ => commaSep1(choice($._arg, $.splat_argument)),

    ensure_clause: $ => seq('ensure', optional(field('body', $._body))),

    // ── literals ─────────────────────────────────────────────────────
    _literal: $ => choice(
      $.integer,
      $.float,
      $.simple_symbol,
      $.character,
      $.true,
      $.false,
      $.nil,
      $.encoding_literal,
      $.file_literal,
      $.line_literal,
    ),

    integer: _ => token(choice(
      /0[bB][01](_?[01])*/,
      /0[oO]?[0-7](_?[0-7])*/,
      /0[dD][0-9](_?[0-9])*/,
      /0[xX][0-9a-fA-F](_?[0-9a-fA-F])*/,
      /[0-9](_?[0-9])*r?i?/,
    )),

    float: _ => token(
      /[0-9](_?[0-9])*(\.[0-9](_?[0-9])*)?([eE][+-]?[0-9](_?[0-9])*|\.[0-9](_?[0-9])*[eE][+-]?[0-9](_?[0-9])*)?(\.[0-9](_?[0-9])*)?r?i?/,
    ),

    true: _ => 'true',
    false: _ => 'false',
    nil: _ => 'nil',
    encoding_literal: _ => '__ENCODING__',
    file_literal: _ => '__FILE__',
    line_literal: _ => '__LINE__',

    // `:foo`, `:foo?`, `:foo=`, `:+`, `:[]=` — one token, fully determined
    // by its text, which is what earns it a place in `_literal`. Scanner-
    // owned because the `=` suffix needs one character of lookahead:
    // `:decl=>1` is the symbol :decl and a hash rocket, `:decl=` alone is
    // a setter symbol, and no regex can hold both.

    character: _ => token(seq(
      '?',
      choice(
        /[^\s\\]/,
        /\\u\{[0-9a-fA-F ]+\}/,
        /\\u[0-9a-fA-F]{4}/,
        /\\x[0-9a-fA-F]{1,2}/,
        /\\[0-7]{1,3}/,
        /\\(C-|c)./,
        /\\M-./,
        /\\./,
      ),
    )),

    // ── strings and friends (scanner-delimited) ──────────────────────
    string: $ => seq(
      $.string_start,
      repeat($._string_part),
      $.string_end,
    ),

    // `"a" "b"` — adjacent literals concatenate, across (escaped) line
    // breaks too. Same construct and same node name as python's.
    concatenated_string: $ => prec.right(seq($.string, repeat1($.string))),

    quoted_symbol: $ => seq(
      alias($._symbol_start, $.string_start),
      repeat($._string_part),
      $.string_end,
    ),

    subshell: $ => seq(
      alias($._subshell_start, $.string_start),
      repeat($._string_part),
      $.string_end,
    ),

    regex: $ => seq(
      alias($._regex_start, $.string_start),
      repeat($._string_part),
      $.string_end,
    ),

    string_array: $ => seq(
      alias($._words_start, $.string_start),
      repeat($._string_part),
      $.string_end,
    ),

    symbol_array: $ => seq(
      alias($._symbols_start, $.string_start),
      repeat($._string_part),
      $.string_end,
    ),

    _string_part: $ => choice(
      $.string_content,
      $.escape_sequence,
      $._interpolation,
    ),

    _interpolation: $ => choice($.interpolation),

    interpolation: $ => seq(
      token.immediate(prec(1, '#{')),
      optional(field('expression', $._statements)),
      '}',
    ),

    heredoc_body: $ => seq(
      $._heredoc_body_start,
      repeat(choice(
        $.heredoc_content,
        $.escape_sequence,
        $._interpolation,
      )),
      $.heredoc_end,
    ),

    // ── collections ──────────────────────────────────────────────────
    array: $ => seq(
      '[',
      optional(seq(commaSep1($._argument), optional(','))),
      ']',
    ),

    hash: $ => seq(
      '{',
      optional(seq(
        commaSep1(choice($.pair, alias($.hash_splat, $.splat_argument))),
        optional(','),
      )),
      '}',
    ),

    hash_splat: $ => seq(alias($._splat_star_star, '**'), optional($._arg)),

    pair: $ => choice(
      seq(field('key', $._arg), '=>', field('value', $._arg)),
      prec.right(seq(
        field('key', alias($._hash_key, $.hash_key_symbol)),
        optional(field('value', $._arg)),
      )),
      seq(
        field('key', $.string),
        token.immediate(':'),
        field('value', $._arg),
      ),
    ),

    // ── names ────────────────────────────────────────────────────────
    identifier: $ => seq(
      $._identifier_token,
      optional($._identifier_suffix),
    ),

    _identifier_token: _ => token(/[_a-z\u{0080}-\u{10FFFF}][_a-zA-Z0-9\u{0080}-\u{10FFFF}]*/u),

    constant: _ => token(/[A-Z][_a-zA-Z0-9\u{0080}-\u{10FFFF}]*/u),

    instance_variable: _ => token(/@[_a-zA-Z\u{0080}-\u{10FFFF}][_a-zA-Z0-9\u{0080}-\u{10FFFF}]*/u),

    class_variable: _ => token(/@@[_a-zA-Z\u{0080}-\u{10FFFF}][_a-zA-Z0-9\u{0080}-\u{10FFFF}]*/u),

    global_variable: _ => token(/\$([_a-zA-Z][_a-zA-Z0-9]*|[0-9]+|[~&`'+*$?!@\/\\;,.=:<>"]|-[a-zA-Z0-9_])/),

    comment: _ => token(seq('#', /[^\r\n]*/)),
  },
});

function commaSep1(rule) {
  return sep1(rule, ',');
}

function sep1(rule, separator) {
  return seq(rule, repeat(seq(separator, rule)));
}
