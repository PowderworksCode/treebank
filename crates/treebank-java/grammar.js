/**
 * treebank-java: a from-scratch grammar for Java 8 through 21, carrying the
 * treebank vocabulary (DESIGN.md §3) in its parse table.
 *
 * Threaded table-tier roles: see `supertypes` below. Omissions and the
 * reasons for them are in ledger.toml's roles_note.
 */

/// <reference types="tree-sitter-cli/dsl" />
// @ts-check

const tb = require('../treebank-core/vocabulary/supertypes.js');

// Java's binary operator ladder, loosest to tightest. The numbers are the
// language's own table read upward; nothing here is tuned.
const PREC = {
  assign: 1,
  ternary: 2,
  lambda: 3,
  or: 4,
  and: 5,
  bitor: 6,
  bitxor: 7,
  bitand: 8,
  equality: 9,
  relational: 10,
  shift: 11,
  additive: 12,
  multiplicative: 13,
  cast: 14,
  unary: 15,
  postfix: 16,
  new: 17,
  access: 18,
};

module.exports = grammar({
  name: 'java',

  word: $ => $.identifier,

  extras: $ => [$.line_comment, $.block_comment, /\s/],

  supertypes: $ => tb.assertTableTerms([
    '_statement',
    '_expression',
    // `_declaration` is demoted to the facet tier here. Java partitions it:
    // a field declaration and a local variable declaration are the same
    // syntax in different places, so one alternation reachable from both a
    // class body and a block makes every field ambiguous with every local.
    // See roles.json and DESIGN.md 3.1.1.
    ...tb.assertDemotable([]),
    '_pattern',
    '_type',
    '_name',
    '_literal',
    '_parameter',
    '_argument',
    '_member',
    '_modifier',
    '_attribute',
    '_directive',
    '_body',
    '_control_flow',
    '_branch',
    '_loop',
    '_jump',
    '_assignment',
    '_invocation',
    '_access',
  ]).map((name) => $[name]),

  conflicts: $ => [
    [$._name, $._type],
    [$._name, $.generic_type],
    [$._unannotated_type, $._name],
    // `case A -> …`: at `A` the parser cannot tell a case constant from a
    // lambda's parameter, and only the `->` settles it — by which time the
    // reduce toward the label is gone unless both readings stay alive.
    [$.modifiers, $.annotation],
    [$.yield_statement, $._soft_keyword],
    [$.annotation_type_element, $.field_declaration, $._type],
    [$.field_declaration, $._type],
    [$.modifiers, $.annotated_type],
    [$.lambda, $._name],
    [$.module_declaration, $.package_declaration, $.modifiers],
    [$.module_declaration, $.modifiers],
    [$.annotation_type_element, $.parameters],
    [$.switch_label_group],
    [$.switch_statement, $.switch_expression],
    [$._unannotated_type, $.generic_type],
    [$._type_id, $.inferred_parameters, $._name],
    [$.annotation_type_body, $._member],
    [$.modifiers, $.local_variable_declaration],
    [$._modifier, $.local_variable_declaration],
    [$.array_type],
    [$._primary, $.scoped_identifier],
    [$._unannotated_type, $.scoped_type_identifier],
    [$.element_value_pair, $._name],
    [$._type_id, $._name],
    [$.parameter, $.spread_parameter, $.receiver_parameter],
    [$.modifiers],
    [$.package_declaration, $.modifiers],
  ],

  rules: {
    // ── compilation unit ─────────────────────────────────────────────
    program: $ => seq(
      repeat($._directive),
      repeat($._top_level),
    ),

    _top_level: $ => choice($._type_declaration, $.module_declaration, ';'),

    // module-info.java (JLS 7.7). Every word here is CONTEXTUAL -- `module`,
    // `requires`, `exports`, `opens`, `uses`, `provides`, `to`, `with`,
    // `open` and `transitive` are all ordinary identifiers everywhere else,
    // so none of them may join the reserved set. They are spelled inline
    // rather than added to `_soft_keyword` because each is legal in exactly
    // one slot of one rule, and the file itself is the context: nothing
    // below is reachable outside a module declaration.
    module_declaration: $ => seq(
      repeat($._attribute),
      optional('open'),
      'module',
      field('name', $._name),
      field('body', $.module_body),
    ),

    module_body: $ => seq('{', repeat($.module_directive), '}'),

    module_directive: $ => choice(
      seq('requires', repeat(choice('transitive', 'static')), field('module', $._name), ';'),
      seq('exports', field('package', $._name), optional($._module_targets), ';'),
      seq('opens', field('package', $._name), optional($._module_targets), ';'),
      seq('uses', field('type', $._name), ';'),
      seq('provides', field('type', $._name), 'with', $._name, repeat(seq(',', $._name)), ';'),
    ),

    _module_targets: $ => seq('to', $._name, repeat(seq(',', $._name))),

    // What may be declared at a file's top level or inside a class body.
    _type_declaration: $ => choice(
      $.class_declaration,
      $.interface_declaration,
      $.enum_declaration,
      $.record_declaration,
      $.annotation_type_declaration,
    ),

    _directive: $ => choice($.package_declaration, $.import_declaration),

    package_declaration: $ => seq(
      repeat($._attribute),
      'package',
      field('name', $._name),
      ';',
    ),

    import_declaration: $ => seq(
      'import',
      optional('static'),
      field('name', $._name),
      optional(seq('.', '*')),
      ';',
    ),

    // ── declarations ─────────────────────────────────────────────────
    class_declaration: $ => seq(
      optional($.modifiers),
      'class',
      field('name', $.identifier),
      field('type_parameters', optional($.type_parameters)),
      optional($.superclass),
      optional($.super_interfaces),
      optional($.permits_clause),
      field('body', $.class_body),
    ),

    superclass: $ => seq('extends', $._type),
    super_interfaces: $ => seq('implements', $._type_list),
    permits_clause: $ => seq('permits', $._type_list),
    _type_list: $ => seq($._type, repeat(seq(',', $._type))),

    interface_declaration: $ => seq(
      optional($.modifiers),
      'interface',
      field('name', $.identifier),
      field('type_parameters', optional($.type_parameters)),
      optional($.extends_interfaces),
      optional($.permits_clause),
      field('body', $.interface_body),
    ),

    extends_interfaces: $ => seq('extends', $._type_list),

    enum_declaration: $ => seq(
      optional($.modifiers),
      'enum',
      field('name', $.identifier),
      optional($.super_interfaces),
      field('body', $.enum_body),
    ),

    enum_body: $ => seq(
      '{',
      optional(seq($.enum_constant, repeat(seq(',', $.enum_constant)), optional(','))),
      optional(seq(';', repeat($._member))),
      '}',
    ),

    enum_constant: $ => seq(
      repeat($._attribute),
      field('name', $.identifier),
      field('arguments', optional($.arguments)),
      field('body', optional($.class_body)),
    ),

    record_declaration: $ => seq(
      optional($.modifiers),
      'record',
      field('name', $.identifier),
      field('type_parameters', optional($.type_parameters)),
      field('parameters', $.parameters),
      optional($.super_interfaces),
      field('body', $.class_body),
    ),

    annotation_type_declaration: $ => seq(
      optional($.modifiers),
      '@',
      'interface',
      field('name', $.identifier),
      field('body', $.annotation_type_body),
    ),

    annotation_type_body: $ => seq('{', repeat(choice($._member, $.annotation_type_element, ';')), '}'),

    annotation_type_element: $ => seq(
      optional($.modifiers),
      field('type', $._unannotated_type),
      field('name', $.identifier),
      '(', ')',
      optional($._dims),
      optional(seq('default', field('value', $._element_value))),
      ';',
    ),

    // ── members ──────────────────────────────────────────────────────
    class_body: $ => seq('{', repeat($._member), '}'),
    interface_body: $ => seq('{', repeat($._member), '}'),

    _member: $ => choice(
      $._type_declaration,
      $.method_declaration,
      $.constructor_declaration,
      $.compact_constructor_declaration,
      $.field_declaration,
      $.initializer_block,
      ';',
    ),

    _body: $ => choice($.block, $.class_body, $.interface_body, $.enum_body, $.annotation_type_body),

    initializer_block: $ => seq(optional('static'), field('body', $.block)),

    field_declaration: $ => seq(
      optional($.modifiers),
      field('type', $._unannotated_type),
      $._declarator_list,
      ';',
    ),

    method_declaration: $ => seq(
      optional($.modifiers),
      field('type_parameters', optional($.type_parameters)),
      field('type', choice($._type, $.void_type)),
      field('name', $.identifier),
      field('parameters', $.parameters),
      optional($._dims),
      optional($.throws_clause),
      choice(field('body', $.block), ';'),
    ),

    constructor_declaration: $ => seq(
      optional($.modifiers),
      field('type_parameters', optional($.type_parameters)),
      field('name', $.identifier),
      field('parameters', $.parameters),
      optional($.throws_clause),
      field('body', $.block),
    ),

    // A record's compact canonical constructor: no parameter list at all,
    // the components being implicit.
    compact_constructor_declaration: $ => seq(
      optional($.modifiers),
      field('name', $.identifier),
      field('body', $.block),
    ),

    throws_clause: $ => seq('throws', $._type_list),

    // ── modifiers and annotations ────────────────────────────────────
    modifiers: $ => repeat1(choice($._attribute, $._modifier)),

    _modifier: $ => choice(
      $.access_modifier,
      $.static_modifier,
      $.final_modifier,
      $.abstract_modifier,
      $.sealed_modifier,
      $.other_modifier,
    ),

    access_modifier: _ => choice('public', 'protected', 'private'),
    static_modifier: _ => 'static',
    final_modifier: _ => 'final',
    abstract_modifier: _ => 'abstract',
    sealed_modifier: _ => choice('sealed', 'non-sealed'),
    other_modifier: _ => choice(
      'native', 'synchronized', 'transient', 'volatile', 'strictfp', 'default',
    ),

    _attribute: $ => $.annotation,

    annotation: $ => seq(
      '@',
      field('name', $._name),
      optional(field('arguments', $.annotation_arguments)),
    ),

    annotation_arguments: $ => seq(
      '(',
      optional(choice(
        $._element_value,
        seq($.element_value_pair, repeat(seq(',', $.element_value_pair))),
      )),
      ')',
    ),

    element_value_pair: $ => seq(
      field('key', $.identifier),
      '=',
      field('value', $._element_value),
    ),

    _element_value: $ => choice($._expression, $.element_value_array, $.annotation),
    element_value_array: $ => seq(
      '{',
      optional(seq($._element_value, repeat(seq(',', $._element_value)), optional(','))),
      '}',
    ),

    // ── types ────────────────────────────────────────────────────────
    _type: $ => choice($._unannotated_type, $.annotated_type),

    annotated_type: $ => seq(repeat1($._attribute), $._unannotated_type),

    _type_id: $ => alias($.identifier, $.type_identifier),

    _unannotated_type: $ => choice(
      $.primitive_type,
      $._type_id,
      $.scoped_type_identifier,
      $.generic_type,
      $.array_type,
    ),

    primitive_type: _ => choice(
      'byte', 'short', 'int', 'long', 'char', 'float', 'double', 'boolean',
    ),

    void_type: _ => 'void',


    scoped_type_identifier: $ => seq(
      field('scope', choice($._type_id, $.scoped_type_identifier, $.generic_type)),
      '.',
      repeat($._attribute),
      field('name', $._type_id),
    ),

    generic_type: $ => prec.dynamic(1, seq(
      choice($._type_id, $.scoped_type_identifier),
      field('type_arguments', $.type_arguments),
    )),

    array_type: $ => seq(
      field('element', $._unannotated_type),
      $._dims,
    ),

    _dims: $ => repeat1(seq(repeat($._attribute), $._empty_dim)),

    // One token, so the LEXER settles what precedence could not. At
    // `new int[2] • [` the parser may either continue this expression's
    // dimensions or start an array access on what was just created; with
    // `[]` arriving whole, an array access — which needs `[` and then an
    // index — cannot match it at all, and the question never reaches the
    // parse table. Java allows whitespace inside the brackets, so the
    // token does too.
    _empty_dim: _ => token(seq('[', /\s*/, ']')),

    type_arguments: $ => seq(
      '<',
      optional(seq(choice($._type, $.wildcard), repeat(seq(',', choice($._type, $.wildcard))))),
      '>',
    ),

    wildcard: $ => seq(
      repeat($._attribute),
      '?',
      optional(seq(choice('extends', 'super'), $._type)),
    ),

    type_parameters: $ => seq(
      '<',
      $.type_parameter,
      repeat(seq(',', $.type_parameter)),
      '>',
    ),

    type_parameter: $ => seq(
      repeat($._attribute),
      field('name', $.identifier),
      optional(seq('extends', $._type, repeat(seq('&', $._type)))),
    ),

    // ── parameters ───────────────────────────────────────────────────
    parameters: $ => seq(
      '(',
      optional(seq($._parameter, repeat(seq(',', $._parameter)))),
      ')',
    ),

    _parameter: $ => choice($.parameter, $.spread_parameter, $.receiver_parameter),

    parameter: $ => seq(
      repeat(choice($._attribute, $.final_modifier)),
      field('type', $._unannotated_type),
      field('name', $.identifier),
      optional($._dims),
    ),

    spread_parameter: $ => seq(
      repeat(choice($._attribute, $.final_modifier)),
      field('type', $._unannotated_type),
      repeat($._attribute),
      '...',
      field('name', $.identifier),
    ),

    // `void m(Outer.this)` — the explicit receiver, java 8 and rare.
    receiver_parameter: $ => seq(
      repeat($._attribute),
      field('type', $._unannotated_type),
      optional(seq($.identifier, '.')),
      'this',
    ),

    // ── statements ───────────────────────────────────────────────────
    _statement: $ => choice(
      // A local class is legal; a field declaration is not.
      $._type_declaration,
      $.local_variable_declaration,
      $._control_flow,
      $.expression_statement,
      $.block,
      $.labeled_statement,
      $.synchronized_statement,
      $.assert_statement,
      $.empty_statement,
    ),

    block: $ => seq('{', repeat($._statement), '}'),
    empty_statement: _ => ';',
    expression_statement: $ => seq($._expression, ';'),

    labeled_statement: $ => seq(field('label', $.identifier), ':', $._statement),

    synchronized_statement: $ => seq(
      'synchronized',
      field('lock', $.parenthesized_expression),
      field('body', $.block),
    ),

    assert_statement: $ => seq(
      'assert',
      $._expression,
      optional(seq(':', $._expression)),
      ';',
    ),

    local_variable_declaration: $ => seq(
      repeat(choice($._attribute, $.final_modifier)),
      field('type', $._unannotated_type),
      $._declarator_list,
      ';',
    ),

    _declarator_list: $ => seq($.variable_declarator, repeat(seq(',', $.variable_declarator))),

    variable_declarator: $ => seq(
      field('name', $.identifier),
      optional($._dims),
      optional(seq('=', field('value', choice($._expression, $.array_initializer)))),
    ),

    array_initializer: $ => seq(
      '{',
      optional(seq(
        choice($._expression, $.array_initializer),
        repeat(seq(',', choice($._expression, $.array_initializer))),
        optional(','),
      )),
      '}',
    ),

    _control_flow: $ => choice($._branch, $._loop, $._jump, $.try_statement),

    _branch: $ => choice($.if_statement, $.switch_statement),

    if_statement: $ => prec.right(seq(
      'if',
      field('condition', $.parenthesized_expression),
      field('consequence', $._statement),
      optional(seq('else', field('alternative', $._statement))),
    )),

    switch_statement: $ => seq(
      'switch',
      field('condition', $.parenthesized_expression),
      field('body', $.switch_block),
    ),

    // A switch is all-arrow or all-colon; java forbids mixing them, and
    // writing it as one choice keeps the two forms out of each other's
    // parser states.
    switch_block: $ => seq('{', choice(repeat($.switch_rule), repeat($.switch_label_group)), '}'),

    // Java 14 arrow form: `case A -> expr;`
    switch_rule: $ => seq(
      $._switch_label,
      '->',
      choice(field('body', $.block), $.throw_statement, seq($._expression, ';')),
    ),

    // The colon form, whose statements belong to the label until the next.
    switch_label_group: $ => seq(repeat1(seq($._switch_label, ':')), repeat($._statement)),

    _switch_label: $ => choice($.case_label, $.default_label),

    // The label admits a FULL expression, lambda included, which reads
    // backwards: `case A -> 1` is exactly the lambda we do not want. It is
    // the fix rather than the bug. Excluding lambda -- five attempts, via
    // a `_no_lambda` tier, static precedence, dynamic precedence and
    // conflicts declared at four different levels -- never worked, because
    // with no ambiguity DECLARED the parser commits to the lambda shift
    // and the constant reading is never explored. Admitting it and
    // declaring `[$.lambda, $._primary]` makes GLR carry both; the lambda
    // branch then needs a second `->` that is not there and dies, and the
    // label reading is what survives. This is upstream's resolution too,
    // whose grammar.js marks the conflict "only conflicts in switch
    // expressions".
    case_label: $ => seq(
      'case',
      choice(seq($._expression, repeat(seq(',', $._expression))), $._pattern),
      optional($.guard),
    ),

    guard: $ => seq('when', $._expression),
    default_label: _ => 'default',

    _loop: $ => choice($.for_statement, $.enhanced_for_statement, $.while_statement, $.do_statement),

    for_statement: $ => seq(
      'for',
      '(',
      choice(field('initializer', $.local_variable_declaration), seq(optional($._expressions), ';')),
      field('condition', optional($._expression)),
      ';',
      field('update', optional($._expressions)),
      ')',
      field('body', $._statement),
    ),

    _expressions: $ => seq($._expression, repeat(seq(',', $._expression))),

    enhanced_for_statement: $ => seq(
      'for',
      '(',
      repeat(choice($._attribute, $.final_modifier)),
      field('type', $._unannotated_type),
      field('name', $.identifier),
      optional($._dims),
      ':',
      field('value', $._expression),
      ')',
      field('body', $._statement),
    ),

    while_statement: $ => seq(
      'while',
      field('condition', $.parenthesized_expression),
      field('body', $._statement),
    ),

    do_statement: $ => seq(
      'do',
      field('body', $._statement),
      'while',
      field('condition', $.parenthesized_expression),
      ';',
    ),

    _jump: $ => choice($.break_statement, $.continue_statement, $.return_statement, $.throw_statement, $.yield_statement),

    break_statement: $ => seq('break', optional(field('label', $.identifier)), ';'),
    continue_statement: $ => seq('continue', optional(field('label', $.identifier)), ';'),
    return_statement: $ => seq('return', optional($._expression), ';'),
    throw_statement: $ => seq('throw', $._expression, ';'),
    yield_statement: $ => seq('yield', $._expression, ';'),

    try_statement: $ => seq(
      'try',
      optional($.resource_specification),
      field('body', $.block),
      repeat($.catch_clause),
      optional($.finally_clause),
    ),

    resource_specification: $ => seq(
      '(',
      $._resource,
      repeat(seq(';', $._resource)),
      optional(';'),
      ')',
    ),

    _resource: $ => choice($.resource, $._name, $.field_access),

    resource: $ => seq(
      repeat(choice($._attribute, $.final_modifier)),
      field('type', $._unannotated_type),
      field('name', $.identifier),
      '=',
      field('value', $._expression),
    ),

    catch_clause: $ => seq(
      'catch',
      '(',
      field('parameter', $.catch_parameter),
      ')',
      field('body', $.block),
    ),

    catch_parameter: $ => seq(
      repeat(choice($._attribute, $.final_modifier)),
      field('type', $._type),
      repeat(seq('|', $._type)),
      field('name', $.identifier),
    ),

    finally_clause: $ => seq('finally', field('body', $.block)),

    // ── expressions ──────────────────────────────────────────────────
    _expression: $ => choice($.lambda, $._no_lambda),

    // Java puts LambdaExpression at the TOP of its expression grammar: a
    // lambda is never an operand. Ours had it reachable from every operand
    // position, so at `case A -> 1` the parser started a ternary whose
    // condition was the lambda `A -> 1` and then had nothing to finish it
    // with — 539 corpus files, every one an arrow switch over an enum.
    // Excluding lambda from the top of `_case_constant` was not enough,
    // because the sub-tiers put it back as a prefix.
    _no_lambda: $ => choice(
      $._assignment,
      $.ternary_expression,
      $.binary_expression,
      $.unary_expression,
      $.update_expression,
      $.cast_expression,
      $.instanceof_expression,
      $.switch_expression,
      $._primary,
    ),

    _primary: $ => choice(
      $._literal,
      $._name,
      $._access,
      $._invocation,
      $.parenthesized_expression,
      $.object_creation_expression,
      $.array_creation_expression,
      $.method_reference,
      $.class_literal,
      $.this,
      $.super,
    ),

    parenthesized_expression: $ => seq('(', $._expression, ')'),

    this: _ => 'this',
    super: _ => 'super',

    _assignment: $ => choice($.assignment),

    assignment: $ => prec.right(PREC.assign, seq(
      field('left', choice($._name, $._access)),
      field('operator', choice(
        '=', '+=', '-=', '*=', '/=', '%=', '&=', '|=', '^=', '<<=', '>>=', '>>>=',
      )),
      field('right', choice($._expression, $.array_initializer)),
    )),

    // The condition may not be a lambda; the branches may — `c ? x -> 1 : y -> 2`
    // is legal java.
    ternary_expression: $ => prec.right(PREC.ternary, seq(
      field('condition', $._no_lambda),
      '?',
      field('consequence', $._expression),
      ':',
      field('alternative', $._expression),
    )),

    binary_expression: $ => choice(
      ...[
        ['||', PREC.or], ['&&', PREC.and], ['|', PREC.bitor], ['^', PREC.bitxor],
        ['&', PREC.bitand], ['==', PREC.equality], ['!=', PREC.equality],
        ['<', PREC.relational], ['>', PREC.relational],
        ['<=', PREC.relational], ['>=', PREC.relational],
        ['<<', PREC.shift], ['>>', PREC.shift], ['>>>', PREC.shift],
        ['+', PREC.additive], ['-', PREC.additive],
        ['*', PREC.multiplicative], ['/', PREC.multiplicative], ['%', PREC.multiplicative],
      ].map(([op, p]) => prec.left(p, seq(
        field('left', $._no_lambda),
        field('operator', op),
        field('right', $._no_lambda),
      ))),
    ),

    unary_expression: $ => prec.right(PREC.unary, seq(
      field('operator', choice('+', '-', '!', '~')),
      field('operand', $._no_lambda),
    )),

    update_expression: $ => choice(
      prec.right(PREC.unary, seq(choice('++', '--'), $._expression)),
      prec.left(PREC.postfix, seq($._expression, choice('++', '--'))),
    ),

    cast_expression: $ => prec(PREC.cast, seq(
      '(',
      field('type', $._type),
      repeat(seq('&', $._type)),
      ')',
      field('value', $._expression),
    )),

    instanceof_expression: $ => prec(PREC.relational, seq(
      field('left', $._no_lambda),
      'instanceof',
      optional('final'),
      field('right', choice($._type, $._pattern)),
    )),

    switch_expression: $ => seq(
      'switch',
      field('condition', $.parenthesized_expression),
      field('body', $.switch_block),
    ),

    lambda: $ => seq(
      field('parameters', choice($.identifier, $.parameters, $.inferred_parameters)),
      '->',
      field('body', choice($._expression, $.block)),
    ),

    inferred_parameters: $ => seq(
      '(',
      $.identifier,
      repeat(seq(',', $.identifier)),
      ')',
    ),

    // ── access and invocation ────────────────────────────────────────
    _access: $ => choice($.field_access, $.array_access),

    field_access: $ => prec(PREC.access, seq(
      field('object', choice($._primary, $.super)),
      '.',
      optional(seq($.super, '.')),
      field('field', choice($.identifier, $.this)),
    )),

    array_access: $ => prec(PREC.access, seq(
      field('array', $._primary),
      '[',
      field('index', $._expression),
      ']',
    )),

    _invocation: $ => choice($.method_invocation, $.explicit_constructor_invocation),

    method_invocation: $ => prec(PREC.access, seq(
      optional(seq(
        field('object', choice($._primary, $.super)),
        '.',
        optional(seq($.super, '.')),
        field('type_arguments', optional($.type_arguments)),
      )),
      field('name', $.identifier),
      field('arguments', $.arguments),
    )),

    // No `;` of its own: it is reached through `_invocation` like a method
    // call, and `expression_statement` supplies the terminator. Java only
    // allows this form as a constructor's first statement, so accepting it
    // wherever an expression may go is a widening -- recorded in
    // ledger.toml rather than paid for by demoting `_invocation`.
    explicit_constructor_invocation: $ => prec(PREC.access, seq(
      optional(seq($._primary, '.')),
      field('type_arguments', optional($.type_arguments)),
      field('constructor', choice($.this, $.super)),
      field('arguments', $.arguments),
    )),

    arguments: $ => seq(
      '(',
      optional(seq($._argument, repeat(seq(',', $._argument)))),
      ')',
    ),

    _argument: $ => $._expression,

    object_creation_expression: $ => prec.right(PREC.new, seq(
      optional(seq(field('object', $._primary), '.')),
      'new',
      field('type_arguments', optional($.type_arguments)),
      repeat($._attribute),
      field('type', choice($._type_id, $.scoped_type_identifier, $.generic_type, $.primitive_type)),
      field('arguments', $.arguments),
      field('body', optional($.class_body)),
    )),

    // Above `_access`: at `new int[2] • [` the parser can either continue
    // this expression's dimensions or start an array access on what was
    // just created, and java says the dimensions win.
    array_creation_expression: $ => prec.right(PREC.access + 1, seq(
      'new',
      repeat($._attribute),
      field('type', choice($._type_id, $.scoped_type_identifier, $.generic_type, $.primitive_type)),
      choice(
        // Sized dimensions, then any number of empty ones:
        // `new int[n][][]`. The trailing pair is written out rather than
        // reusing `_dims`, which the parser also owes to `array_access` and
        // could not tell apart past the first `[]`.
        seq(
          repeat1(seq(repeat($._attribute), '[', $._expression, ']')),
          repeat(seq(repeat($._attribute), $._empty_dim)),
        ),
        seq($._dims, field('value', $.array_initializer)),
      ),
    )),

    method_reference: $ => prec(PREC.access, seq(
      choice($._primary, $._unannotated_type, $.super),
      '::',
      field('type_arguments', optional($.type_arguments)),
      choice('new', $.identifier),
    )),

    class_literal: $ => seq(
      choice($._unannotated_type, $.void_type),
      '.',
      'class',
    ),

    // ── patterns (java 16 instanceof, java 21 switch) ────────────────
    _pattern: $ => choice($.type_pattern, $.record_pattern),

    type_pattern: $ => seq(
      repeat(choice($._attribute, $.final_modifier)),
      field('type', $._unannotated_type),
      field('name', $.identifier),
    ),

    record_pattern: $ => seq(
      field('type', choice($._type_id, $.scoped_type_identifier, $.generic_type)),
      '(',
      optional(seq($._pattern_component, repeat(seq(',', $._pattern_component)))),
      ')',
    ),

    _pattern_component: $ => choice($._pattern, $.unnamed_pattern),
    unnamed_pattern: _ => '_',

    // ── names ────────────────────────────────────────────────────────
    _name: $ => choice(
      $.identifier,
      alias($._soft_keyword, $.identifier),
      $.scoped_identifier,
    ),

    // Contextual keywords: each one starts a construct in exactly one
    // position and is an ordinary identifier everywhere else. `record` is
    // the one that matters -- `record.setCategory(x)` appears in 113 corpus
    // files, all of them predating the keyword by a decade.
    _soft_keyword: _ => choice('record', 'sealed', 'permits', 'yield', 'when'),

    scoped_identifier: $ => seq(
      field('scope', $._name),
      '.',
      field('name', $.identifier),
    ),

    identifier: _ => /[\p{L}_$][\p{L}\p{Nd}_$]*/,

    // ── literals ─────────────────────────────────────────────────────
    _literal: $ => choice(
      $.integer_literal,
      $.floating_point_literal,
      $.character_literal,
      $.string_literal,
      $.text_block,
      $.true,
      $.false,
      $.null_literal,
    ),

    integer_literal: _ => token(seq(
      choice(
        /0[xX][0-9a-fA-F_]+/,
        /0[bB][01_]+/,
        /0[0-7_]+/,
        /[0-9][0-9_]*/,
      ),
      optional(/[lL]/),
    )),

    floating_point_literal: _ => token(choice(
      seq(/[0-9][0-9_]*/, '.', optional(/[0-9][0-9_]*/), optional(/[eE][+-]?[0-9_]+/), optional(/[fFdD]/)),
      seq('.', /[0-9][0-9_]*/, optional(/[eE][+-]?[0-9_]+/), optional(/[fFdD]/)),
      seq(/[0-9][0-9_]*/, /[eE][+-]?[0-9_]+/, optional(/[fFdD]/)),
      seq(/[0-9][0-9_]*/, /[fFdD]/),
      seq(/0[xX][0-9a-fA-F_]*\.?[0-9a-fA-F_]*/, /[pP][+-]?[0-9_]+/, optional(/[fFdD]/)),
    )),

    character_literal: _ => token(seq(
      "'",
      choice(
        /[^'\\\n]/,
        /\\[btnfrs'"\\]/,
        /\\[0-7]{1,3}/,
        /\\u+[0-9a-fA-F]{4}/,
      ),
      "'",
    )),

    string_literal: _ => token(seq('"', repeat(choice(/[^"\\\n]/, /\\(.|\n)/)), '"')),

    // Java 15 text blocks. One token, like every other string here: the
    // interior may contain `"` and `\`, and only the closing `"""` ends it.
    text_block: _ => token(seq(
      '"""',
      repeat(choice(/[^"\\]/, /\\(.|\n)/, /"[^"]/, /""[^"]/)),
      '"""',
    )),

    true: _ => 'true',
    false: _ => 'false',
    null_literal: _ => 'null',

    // ── comments ─────────────────────────────────────────────────────
    line_comment: _ => token(seq('//', /[^\r\n]*/)),
    block_comment: _ => token(seq('/*', /[^*]*\*+([^/*][^*]*\*+)*/, '/')),
  },
});
