/**
 * treebank-typescript: a from-scratch grammar for the TypeScript ∪
 * JavaScript union, JSX included, carrying the treebank vocabulary
 * (DESIGN.md §3) in its parse table.
 *
 * One parser, deliberately: DESIGN.md §4.2 planned typescript/tsx dialect
 * parsers because `<T>x` casts and JSX collide — but angle-bracket casts
 * are the one construct on the typescript side of that collision, they are
 * legacy style (`as` replaced them), and the corpus measures their
 * incidence at approximately zero. So JSX is in, `<T>x` casts are a
 * ledgered known-gap, and the dialect split waits until a corpus proves it
 * necessary. 21 of 22 table-tier terms thread — everything but `_clause`.
 */

/// <reference types="tree-sitter-cli/dsl" />
// @ts-check

const tb = require('../treebank-core/vocabulary/supertypes.js');

const PREC = {
  sequence: -2,
  assign: 0,
  arrow: 1,
  yield: 1,
  ternary: 2,
  nullish: 3,
  or: 4,
  and: 5,
  bitor: 6,
  bitxor: 7,
  bitand: 8,
  equality: 9,
  relational: 10,
  shift: 11,
  add: 12,
  mul: 13,
  exp: 14,
  cast: 15,
  unary: 16,
  update: 17,
  new_no_args: 18,
  call: 19,
  member: 20,
};

module.exports = grammar({
  name: 'typescript',

  word: $ => $.identifier,

  extras: $ => [
    /\s/,
    $.comment,
  ],

  externals: $ => [
    $._automatic_semicolon,
    // A zero-width member boundary for object types. Distinct from ASI
    // because its continuation set is the opposite one: `[` CONTINUES an
    // expression (`foo\n[0]`) and BEGINS a type member (an index
    // signature), and the scanner cannot tell those apart. It does not
    // have to — this token is only ever valid where the parser wants a
    // member boundary, so validity does the disambiguation the lookahead
    // cannot.
    $._type_member_end,
  ],

  supertypes: $ => tb.assertTableTerms([
    '_statement',
    '_expression',
    '_declaration',
    '_pattern',
    '_type',
    '_name',
    '_literal',
    '_parameter',
    '_argument',
    '_member',
    '_directive',
    '_body',
    '_control_flow',
    '_branch',
    '_loop',
    '_jump',
    '_assignment',
    '_invocation',
    '_access',
    '_attribute',
    '_modifier',
    '_interpolation',
  ]).map((name) => $[name]),

  conflicts: $ => [
    [$._expression, $.named_tuple_member],
    [$.import_type, $._literal],
    [$.nested_identifier, $.import_type, $._name],
    [$.nested_identifier, $._no_conditional_type, $._name],
    [$.nested_identifier, $._type, $._name],
    [$._expression, $.nested_type_identifier],
    [$._no_conditional_type, $._name],
    [$._type, $._no_conditional_type, $._name],
    [$.rest_pattern, $.predicate_type],
    [$._expression, $.rest_pattern, $.predicate_type],
    [$._expression, $.tuple_type, $.predicate_type],
    [$._expression, $._pattern, $.predicate_type],
    [$._pattern, $.predicate_type],
    [$.nested_identifier, $._type, $.nested_type_identifier, $._name],
    [$.infer_type, $._soft_keyword],
    [$.constructor_type, $._soft_keyword],
    [$._jsx_name, $.type_parameter],
    // `<T>(x) => x` is a generic arrow in .ts and a JSX element in .tsx.
    // One parser must hold both readings until the source decides: GLR
    // forks here, jsx_opening_element's negative dynamic precedence yields
    // to the arrow whenever both complete, and a real JSX element wins
    // because the arrow fork dies at the missing `=>`.
    [$.type_parameters, $.jsx_opening_element],
    [$.type_parameters, $.jsx_self_closing_element],
    [$.arrow_function, $.jsx_element],
    [$._statement_expression, $._property_name, $.shorthand_property, $.shorthand_property_pattern],
    [$._statement_expression, $._property_name],
    [$._statement_expression, $.shorthand_property, $.shorthand_property_pattern],
    [$.function_definition, $.function_expression],
    [$.function_definition, $._reserved_property, $.function_expression],
    [$.indexed_access_type, $.index_signature],
    [$.indexed_access_type, $.computed_property_name],
    [$.array_type, $.computed_property_name],
    [$.indexed_access_type, $.index_signature, $.computed_property_name],
    [$.array_type, $.index_signature, $.computed_property_name],
    [$.array_type, $.indexed_access_type, $.index_signature],
    [$._no_conditional_type, $.predicate_type],
    [$.nested_identifier, $.nested_type_identifier, $._no_conditional_type],
    [$._type, $._no_conditional_type],
    [$._type, $._no_conditional_type, $.predicate_type],
    [$.accessor_modifier, $._soft_keyword],
    [$.readonly_modifier, $._soft_keyword],
    [$.mapped_type, $.object_type],
    [$.mapped_type, $.property_signature],
    [$.readonly_modifier, $._property_name],
    [$.function_expression, $._soft_keyword],
    [$._type, $.tuple_type, $.predicate_type, $._name],
    [$._type, $.tuple_type, $.predicate_type],
    [$.arrow_function, $._soft_keyword],
    [$.array_type, $.index_signature],
    [$._reserved_property, $.interface_definition],
    [$._reserved_property, $.import_alias, $.export_statement],
    [$._reserved_property, $.import_statement],
    [$._reserved_property, $.import_alias, $.import_statement],
    [$.for_in_statement, $._reserved_property],
    [$._reserved_property, $.enum_definition],
    [$.variable_declaration, $._reserved_property],
    [$.function_definition, $.property_signature, $.method_signature],
    // `_method_head` is the shared prefix of a method DEFINITION and a
    // method SIGNATURE; which one it is only becomes clear at the body or
    // the terminator.
    [$._method_head, $.method_signature],
    [$.property_signature, $.method_signature],
    [$.method_signature, $._soft_keyword],
    [$.import_alias, $.import_statement, $._soft_keyword],
    [$.arrow_function, $._name],
    [$.arrow_function, $.call_expression],
    [$.infer_type, $.conditional_type],
    [$.rest_type, $.tuple_type],
    [$.continue_statement, $._reserved_property],
    [$.break_statement, $._reserved_property],
    [$.debugger_statement, $._reserved_property],
    [$.case_clause],
    [$.default_clause],
    [$._reserved_property, $.construct_signature],
    [$.function_definition, $._reserved_property],
    [$._reserved_property, $.export_statement],
    [$.import_statement, $._soft_keyword],
    [$.try_statement],
    [$._reserved_property, $.false],
    [$._reserved_property, $.true],
    [$._reserved_property, $.null],
    [$._reserved_property, $.super],
    [$._reserved_property, $.this],
    [$._reserved_property, $.class_definition],
    [$.throw_statement, $._reserved_property],
    [$.return_statement, $._reserved_property],
    [$.do_statement, $._reserved_property],
    [$.while_statement, $._reserved_property],
    [$.for_statement, $.for_in_statement, $._reserved_property],
    [$.switch_statement, $._reserved_property],
    [$.if_statement, $._reserved_property],
    [$.with_statement, $._reserved_property],
    [$.nested_identifier, $.nested_type_identifier, $.import_type],
    [$.nested_identifier, $.nested_type_identifier],
    [$.nested_identifier, $.nested_type_identifier, $._name],
    [$.nested_identifier, $._type, $.nested_type_identifier, $.predicate_type],
    [$.nested_type_identifier, $._name],
    [$.nested_type_identifier, $.member_expression],
    [$.nested_type_identifier, $._name, $.member_expression],
    [$.nested_identifier, $._name],
    [$._type, $._name],
    [$._type, $._name, $._pattern],
    [$.binary_expression, $.type_arguments],
    [$.parameter, $._expression, $.predicate_type],
    [$.parameter, $.predicate_type],
    [$.parameter, $._expression],
    [$.array_type, $.subscript_expression],
    [$._type, $._expression, $._pattern],
    [$.import_expression, $.import_type],
    [$._expression, $.import_type],
    [$.index_signature],
    [$._property_name, $._expression, $.shorthand_property, $.shorthand_property_pattern],
    [$.function_definition, $.method_signature, $._soft_keyword],
    [$.empty_statement, $.object_type],
    [$.import_type],
    [$.conditional_type, $.optional_type],
    [$.import_alias],
    [$.assignment_expression, $.pair_pattern],
    [$.array, $.array_pattern, $.tuple_type],
    [$.object, $.object_pattern, $.object_type],
    [$.function_definition, $.method_signature],
    [$.array_pattern, $.tuple_type],
    [$._property_name, $.shorthand_property_pattern],
    [$.object_pattern, $.object_type],
    [$.mapped_type, $._name],
    [$.template_string, $.template_literal_type],
    [$.typeof_type, $._name],
    [$._expression, $.tuple_type],
    [$.array, $.tuple_type],
    [$._property_name, $.shorthand_property, $.shorthand_property_pattern],
    [$.object, $.object_type],
    [$.nested_identifier, $._name],
    [$.conditional_type, $.rest_type],
    [$.rest_type, $.optional_type],
    [$._modifier, $.mapped_type],
    [$.assignment_expression, $.assignment_pattern],
    [$.function_definition],
    [$.parameter, $.assignment_expression],
    [$._access, $.pair_pattern],
    [$._expression, $.literal_type],
    [$._expression, $.predicate_type],
    [$.literal_type, $._literal],
    [$._type, $.predicate_type, $._name],
    [$.binary_expression, $.call_expression],
    [$.import_specifier, $._soft_keyword],
    [$._type, $.predicate_type],
    [$.this, $.this_type],
    [$.computed_property_name, $.array],
    [$.array],
    [$.export_specifier, $._soft_keyword],
    [$.object, $.object_pattern],
    [$.for_in_statement, $._expression],
    [$.for_in_statement, $._access],
    [$._expression, $.rest_pattern],
    [$._access, $.rest_pattern],
    [$.array_pattern],
    [$.binary_expression, $.call_expression, $.new_expression],
    [$.binary_expression, $.update_expression, $.call_expression],
    [$._jsx_element_name, $.type_parameter],
    [$.jsx_opening_element, $.jsx_fragment],
    [$.decorator, $._expression],
    [$.decorator, $._access],
    [$.decorator, $._invocation],
    [$._access, $.array_pattern],
    [$.array, $.array_pattern],
    [$.binary_expression, $.await_expression, $.call_expression],
    [$._expression, $._pattern],
    [$.shorthand_property, $.shorthand_property_pattern],
    [$.shorthand_property, $._pattern, $.shorthand_property_pattern],
    [$.labeled_statement, $._property_name],
    [$._expression, $.shorthand_property, $.shorthand_property_pattern],
    [$._declaration, $.object],
    [$.binary_expression, $.unary_expression, $.call_expression],
    [$._property_name, $._expression],
    [$._declaration, $._expression],
    [$._property_name, $._literal],
    [$.namespace_definition, $._soft_keyword],
    [$.type_alias, $._soft_keyword],
    [$.function_definition, $._soft_keyword],
    [$.declare_modifier, $._soft_keyword],
    [$.override_modifier, $._soft_keyword],
    [$.abstract_modifier, $._soft_keyword],
    [$.static_modifier, $._soft_keyword],
    [$.readonly_modifier, $._soft_keyword],
    [$.accessibility_modifier, $._soft_keyword],
  ],

  rules: {
    program: $ => seq(optional($.hash_bang_line), repeat($._statement)),

    hash_bang_line: _ => /#!.*/,

    // ── statements ───────────────────────────────────────────────────
    _statement: $ => choice(
      $._declaration,
      $._directive,
      $._control_flow,
      $.expression_statement,
      $.variable_declaration,
      $.block,
      $.try_statement,
      $.labeled_statement,
      $.empty_statement,
      $.debugger_statement,
      $.with_statement,
    ),

    empty_statement: _ => ';',
    debugger_statement: $ => seq('debugger', $._semicolon),

    // JS spec: an expression statement may not BEGIN with `function`,
    // `class` or `{`. The restriction is structural and applies to the
    // first token position only — operands anywhere inside still use the
    // full expression tier.
    expression_statement: $ => seq(
      choice($._statement_expression, alias($.statement_sequence, $.sequence_expression)),
      $._semicolon,
    ),

    statement_sequence: $ => prec.right(PREC.sequence, seq(
      $._statement_expression,
      repeat1(seq(',', $._expression)),
    )),

    _statement_expression: $ => choice(
      $._invocation,
      $._access,
      $._assignment,
      $._literal,
      $.template_string,
      $._name,
      $.array,
      $.arrow_function,
      $.binary_expression,
      $.unary_expression,
      $.update_expression,
      $.conditional_expression,
      $.await_expression,
      $.yield_expression,
      $.as_expression,
      $.satisfies_expression,
      $.non_null_expression,
      $.parenthesized_expression,
      $.this,
      $.super,
      $.import_expression,
      $.import_meta,
      $.new_target,
      $.jsx_element,
      $.jsx_self_closing_element,
      $.jsx_fragment,
    ),

    _expressions: $ => choice($._expression, $.sequence_expression),

    sequence_expression: $ => prec.right(PREC.sequence, seq(
      $._expression,
      repeat1(seq(',', $._expression)),
    )),

    _semicolon: $ => choice(';', $._automatic_semicolon),

    variable_declaration: $ => seq(
      repeat($._modifier),
      field('kind', choice('var', 'let', 'const')),
      commaSep1($.variable_declarator),
      $._semicolon,
    ),

    variable_declarator: $ => seq(
      field('name', $._pattern),
      optional('!'),
      optional(seq(':', field('type', $._type))),
      optional(seq('=', field('value', $._expression))),
    ),

    labeled_statement: $ => prec.dynamic(-1, seq(
      field('label', $._name),
      ':',
      field('body', $._statement),
    )),

    with_statement: $ => seq(
      'with',
      field('object', $.parenthesized_expression),
      field('body', $._statement),
    ),

    // ── control flow ─────────────────────────────────────────────────
    _control_flow: $ => choice($._branch, $._loop, $._jump),

    _branch: $ => choice($.if_statement, $.switch_statement),

    if_statement: $ => prec.right(seq(
      'if',
      field('condition', $.parenthesized_expression),
      field('consequence', $._statement),
      optional(field('alternative', $.else_clause)),
    )),

    else_clause: $ => seq('else', $._statement),

    switch_statement: $ => seq(
      'switch',
      field('subject', $.parenthesized_expression),
      field('body', $.switch_body),
    ),

    switch_body: $ => seq(
      '{',
      repeat(choice($.case_clause, $.default_clause)),
      '}',
    ),

    case_clause: $ => seq(
      'case',
      field('value', $._expressions),
      ':',
      repeat($._statement),
    ),

    default_clause: $ => seq('default', ':', repeat($._statement)),

    _loop: $ => choice(
      $.for_statement,
      $.for_in_statement,
      $.while_statement,
      $.do_statement,
    ),

    for_statement: $ => seq(
      'for',
      '(',
      field('initializer', choice(
        alias($.for_variable_declaration, $.variable_declaration),
        $.expression_statement,
        $.empty_statement,
      )),
      field('condition', choice(
        seq($._expressions, ';'),
        ';',
      )),
      field('increment', optional($._expressions)),
      ')',
      field('body', $._statement),
    ),

    for_variable_declaration: $ => seq(
      field('kind', choice('var', 'let', 'const')),
      commaSep1($.variable_declarator),
      ';',
    ),

    for_in_statement: $ => seq(
      'for',
      optional('await'),
      '(',
      choice(
        seq(
          field('kind', choice('var', 'let', 'const')),
          field('left', $._pattern),
        ),
        field('left', choice($._name, $.member_expression, $.subscript_expression, $.object_pattern, $.array_pattern, $.parenthesized_expression)),
      ),
      field('operator', choice('in', 'of')),
      field('right', $._expressions),
      ')',
      field('body', $._statement),
    ),

    while_statement: $ => seq(
      'while',
      field('condition', $.parenthesized_expression),
      field('body', $._statement),
    ),

    do_statement: $ => prec.right(seq(
      'do',
      field('body', $._statement),
      'while',
      field('condition', $.parenthesized_expression),
      optional($._semicolon),
    )),

    _jump: $ => choice(
      $.return_statement,
      $.break_statement,
      $.continue_statement,
      $.throw_statement,
    ),

    return_statement: $ => seq('return', optional($._expressions), $._semicolon),
    break_statement: $ => seq('break', optional(field('label', $._name)), $._semicolon),
    continue_statement: $ => seq('continue', optional(field('label', $._name)), $._semicolon),
    throw_statement: $ => seq('throw', $._expressions, $._semicolon),

    try_statement: $ => seq(
      'try',
      field('body', $.block),
      optional($.catch_clause),
      optional($.finally_clause),
    ),

    // `prec.dynamic(2)`, above `function_definition`'s 1. Its method form
    // takes a name, parameters and a body with no `function` keyword, so at
    // STATEMENT level `catch (e) { ... }` also derives a function named
    // `catch` -- and the dynamic precedence made that reading win. The whole
    // clause vanished from the tree: `try {} catch (e) {}` came out as a
    // `try_statement` with no catch, followed by an unrelated
    // `function_definition`. 66 corpus files, no error anywhere.
    //
    // The deeper fix is to stop the keyword-less method form being reachable
    // at statement level at all -- it belongs to class bodies and object
    // types. That is a larger refactor of `_declaration`, which class members
    // route through on purpose; this raises the two clauses that actually
    // collide, and the shape check now covers the rest of the class.
    catch_clause: $ => prec.dynamic(2, seq(
      'catch',
      optional(seq('(', field('parameter', $._pattern), optional(seq(':', field('type', $._type))), ')')),
      field('body', $.block),
    )),

    finally_clause: $ => prec.dynamic(2, seq('finally', field('body', $.block))),

    // ── declarations ─────────────────────────────────────────────────
    _declaration: $ => choice(
      $.function_definition,
      $.class_definition,
      $.interface_definition,
      $.type_alias,
      $.enum_definition,
      $.namespace_definition,
      $.import_alias,
    ),

    _modifier: $ => choice(
      $.accessibility_modifier,
      $.readonly_modifier,
      $.static_modifier,
      $.abstract_modifier,
      $.override_modifier,
      $.declare_modifier,
      $.accessor_modifier,
    ),

    accessibility_modifier: _ => prec(1, choice('public', 'private', 'protected')),
    readonly_modifier: _ => prec(1, 'readonly'),
    static_modifier: _ => prec(1, 'static'),
    abstract_modifier: _ => prec(1, 'abstract'),
    override_modifier: _ => prec(1, 'override'),
    declare_modifier: _ => prec(1, 'declare'),
    accessor_modifier: _ => prec(1, 'accessor'),

    // Three alternatives, and the split is the point.
    //
    // The method form -- name, parameters, body, no `function` keyword -- is
    // for class and object members. It is reachable at STATEMENT level too,
    // because members route through `_declaration`, and there it collides
    // with an ordinary call. With one blanket `prec.dynamic(1)` the
    // declaration reading won, so `foo();` -- every zero-argument call
    // statement in the language -- parsed as a bodyless function declaration
    // named `foo` rather than a call. `foo(1, 2)` was fine, because number
    // literals cannot be parameters; only the argument-less case collided,
    // and it collided everywhere.
    //
    // Boundaries alone cannot catch this: `function_definition` and
    // `expression_statement` span the same bytes and only the KINDS differ,
    // so `treebank shape` is blind to it. It surfaced while reading a shape
    // fixture's tree by eye.
    //
    // So the BODYLESS method form -- the one that is a real member signature
    // in a class or interface and nothing at all at statement level -- gets
    // a negative dynamic precedence and loses to the call. The other two
    // keep theirs.
    function_definition: $ => choice(
      prec.dynamic(1, prec.right(seq(
        repeat($._attribute),
        repeat($._modifier),
        optional('async'),
        'function',
        optional('*'),
        optional(field('name', $._name)),
        field('type_parameters', optional($.type_parameters)),
        field('parameters', $.parameters),
        optional(seq(':', field('return_type', $._type))),
        choice(field('body', $._body), $._semicolon),
      ))),
      prec.dynamic(1, prec.right(seq(
        $._method_head,
        field('type_parameters', optional($.type_parameters)),
        field('parameters', $.parameters),
        optional(seq(':', field('return_type', $._type))),
        field('body', $._body),
      ))),
      prec.dynamic(-1, prec.right(seq(
        $._method_head,
        field('type_parameters', optional($.type_parameters)),
        field('parameters', $.parameters),
        optional(seq(':', field('return_type', $._type))),
        $._semicolon,
      ))),
    ),

    _method_head: $ => seq(
      repeat($._attribute),
      repeat($._modifier),
      optional('async'),
      optional(choice('get', 'set')),
      optional('*'),
      field('name', $._property_name),
      optional('?'),
    ),

    _property_name: $ => choice(
      $._name,
      alias($._reserved_property, $.identifier),
      $.string,
      $.number,
      $.computed_property_name,
      $.private_name,
    ),

    _reserved_property: _ => choice(
      'function', 'class', 'new', 'default', 'import', 'export', 'typeof',
      'void', 'delete', 'in', 'instanceof', 'if', 'else', 'for', 'while',
      'do', 'return', 'switch', 'case', 'catch', 'finally', 'throw', 'try',
      'const', 'var', 'let', 'this', 'super', 'null', 'true', 'false',
      'enum', 'interface', 'extends', 'yield', 'await', 'break', 'continue',
      'debugger', 'with',
    ),

    computed_property_name: $ => seq('[', $._expression, ']'),
    private_name: $ => seq('#', $.identifier),

    parameters: $ => seq(
      '(',
      optional(seq(commaSep1($._parameter), optional(','))),
      ')',
    ),

    _parameter: $ => choice($.parameter, $.rest_parameter),

    parameter: $ => seq(
      repeat($._attribute),
      repeat($._modifier),
      field('pattern', choice($._pattern, $.this)),
      optional('?'),
      optional(seq(':', field('type', $._type))),
      optional(seq('=', field('value', $._expression))),
    ),

    rest_parameter: $ => seq(
      '...',
      field('pattern', $._pattern),
      optional(seq(':', field('type', $._type))),
    ),

    class_definition: $ => prec.dynamic(1, prec.right(seq(
      repeat($._attribute),
      repeat($._modifier),
      'class',
      optional(field('name', $._name)),
      field('type_parameters', optional($.type_parameters)),
      optional($.class_heritage),
      field('body', $.class_body),
    ))),

    class_heritage: $ => repeat1(choice(
      seq('extends', field('value', choice(
        seq($._expression, field('type_arguments', optional($.type_arguments))),
        $.generic_type,
      ))),
      seq('implements', commaSep1($._type)),
    )),

    class_body: $ => seq(
      '{',
      repeat(choice($._member, ';')),
      '}',
    ),

    // Members route through _declaration (the python/rust precedent), so a
    // method answers (_declaration) as well as (_member).
    _member: $ => choice(
      $._declaration,
      $.field_definition,
      $.index_signature,
      $.static_block,
    ),

    field_definition: $ => seq(
      repeat($._attribute),
      repeat($._modifier),
      field('name', $._property_name),
      optional(choice('?', '!')),
      optional(seq(':', field('type', $._type))),
      optional(seq('=', field('value', $._expression))),
      $._semicolon,
    ),

    index_signature: $ => seq(
      repeat($._modifier),
      '[',
      field('name', $._name),
      ':',
      field('index_type', $._type),
      ']',
      optional(choice('?', seq(':', field('type', $._type)))),
      optional($._semicolon),
    ),

    static_block: $ => seq('static', field('body', $.block)),

    interface_definition: $ => seq(
      repeat($._modifier),
      'interface',
      field('name', $._name),
      field('type_parameters', optional($.type_parameters)),
      optional(seq('extends', commaSep1($._type))),
      field('body', $.object_type),
    ),

    type_alias: $ => seq(
      repeat($._modifier),
      'type',
      field('name', $._name),
      field('type_parameters', optional($.type_parameters)),
      '=',
      field('value', $._type),
      $._semicolon,
    ),

    enum_definition: $ => seq(
      repeat($._modifier),
      optional('const'),
      'enum',
      field('name', $._name),
      field('body', $.enum_body),
    ),

    enum_body: $ => seq(
      '{',
      optional(seq(commaSep1($.enum_member), optional(','))),
      '}',
    ),

    enum_member: $ => seq(
      field('name', $._property_name),
      optional(seq('=', field('value', $._expression))),
    ),

    namespace_definition: $ => choice(
      seq(
        repeat($._modifier),
        choice('namespace', 'module'),
        field('name', choice($._name, $.nested_identifier, $.string)),
        field('body', $.block),
      ),
      // `declare global { … }` augmentation
      seq(repeat($._modifier), field('name', alias('global', $.identifier)), field('body', $.block)),
    ),

    nested_identifier: $ => seq(
      field('object', choice($.identifier, $.nested_identifier)),
      '.',
      field('property', $.identifier),
    ),

    import_alias: $ => seq(
      optional('export'),
      'import',
      optional('type'),
      field('name', $._name),
      '=',
      field('value', choice($._name, $.nested_identifier, $.call_expression)),
      $._semicolon,
    ),

    // ── directives ───────────────────────────────────────────────────
    _directive: $ => choice($.import_statement, $.export_statement),

    import_statement: $ => seq(
      'import',
      optional(choice('type', 'typeof')),
      choice(
        seq($.import_clause, 'from', field('source', $.string)),
        field('source', $.string),
      ),
      optional($.import_attribute),
      $._semicolon,
    ),

    import_clause: $ => choice(
      $.namespace_import,
      $.named_imports,
      seq(
        field('default', $._name),
        optional(seq(',', choice($.namespace_import, $.named_imports))),
      ),
    ),

    namespace_import: $ => seq('*', 'as', field('alias', $._name)),

    named_imports: $ => seq(
      '{',
      optional(seq(commaSep1($.import_specifier), optional(','))),
      '}',
    ),

    import_specifier: $ => seq(
      optional(choice('type', 'typeof')),
      field('name', choice($._name, $.string)),
      optional(seq('as', field('alias', $._name))),
    ),

    import_attribute: $ => seq(choice('with', 'assert'), $.object),

    export_statement: $ => choice(
      seq('export', optional('type'), $.export_clause, optional(seq('from', field('source', $.string))), $._semicolon),
      seq('export', optional('type'), '*', optional(seq('as', field('alias', $._name))), 'from', field('source', $.string), $._semicolon),
      seq('export', optional('default'), $._declaration),
      seq('export', 'default', $._expression, $._semicolon),
      seq('export', $.variable_declaration),
      seq('export', '=', $._expression, $._semicolon),
      seq('export', 'as', 'namespace', $._name, $._semicolon),
    ),

    export_clause: $ => seq(
      '{',
      optional(seq(commaSep1($.export_specifier), optional(','))),
      '}',
    ),

    export_specifier: $ => seq(
      optional(choice('type', 'typeof')),
      field('name', choice($._name, $.string)),
      optional(seq('as', field('alias', choice($._name, $.string)))),
    ),

    // ── attributes (decorators) ──────────────────────────────────────
    _attribute: $ => choice($.decorator),

    decorator: $ => seq(
      '@',
      choice(
        $._name,
        $.member_expression,
        $.call_expression,
        $.parenthesized_expression,
      ),
    ),

    // ── expressions ──────────────────────────────────────────────────
    _expression: $ => choice(
      $._invocation,
      $._access,
      $._assignment,
      $._literal,
      $.template_string,
      $._name,
      $.object,
      $.array,
      $.arrow_function,
      $.function_expression,
      $.class_definition,
      $.binary_expression,
      $.unary_expression,
      $.update_expression,
      $.conditional_expression,
      $.await_expression,
      $.yield_expression,
      $.as_expression,
      $.satisfies_expression,
      $.non_null_expression,
      $.parenthesized_expression,
      $.this,
      $.super,
      $.import_expression,
      $.import_meta,
      $.new_target,
      $.jsx_element,
      $.jsx_self_closing_element,
      $.jsx_fragment,
    ),

    this: _ => 'this',
    super: _ => 'super',

    // dynamic import() and import.meta only — bare `import` stays a
    // statement keyword.
    import_expression: $ => prec(PREC.call, seq('import', field('arguments', $.arguments))),
    import_meta: $ => seq('import', '.', 'meta'),
    new_target: $ => seq('new', '.', 'target'),

    _assignment: $ => choice($.assignment_expression, $.augmented_assignment_expression),

    assignment_expression: $ => prec.right(PREC.assign, seq(
      field('left', choice($._pattern, $.member_expression, $.subscript_expression, $.parenthesized_expression, $.non_null_expression)),
      '=',
      field('right', $._expression),
    )),

    augmented_assignment_expression: $ => prec.right(PREC.assign, seq(
      field('left', choice($._name, $.member_expression, $.subscript_expression, $.parenthesized_expression, $.non_null_expression)),
      field('operator', choice(
        '+=', '-=', '*=', '/=', '%=', '**=', '<<=', '>>=', '>>>=',
        '&=', '|=', '^=', '&&=', '||=', '??=',
      )),
      field('right', $._expression),
    )),

    binary_expression: $ => {
      const table = [
        ['??', PREC.nullish], ['||', PREC.or], ['&&', PREC.and],
        ['|', PREC.bitor], ['^', PREC.bitxor], ['&', PREC.bitand],
        ['==', PREC.equality], ['!=', PREC.equality], ['===', PREC.equality], ['!==', PREC.equality],
        ['<', PREC.relational], ['<=', PREC.relational], ['>', PREC.relational], ['>=', PREC.relational],
        ['in', PREC.relational], ['instanceof', PREC.relational],
        ['<<', PREC.shift], ['>>', PREC.shift], ['>>>', PREC.shift],
        ['+', PREC.add], ['-', PREC.add],
        ['*', PREC.mul], ['/', PREC.mul], ['%', PREC.mul],
      ];
      return choice(
        prec.left(PREC.relational, seq(
          field('left', $.private_name),
          field('operator', 'in'),
          field('right', $._expression),
        )),
        ...table.map(([op, p]) => prec.left(p, seq(
          field('left', $._expression),
          field('operator', op),
          field('right', $._expression),
        ))),
        prec.right(PREC.exp, seq(
          field('left', $._expression),
          field('operator', '**'),
          field('right', $._expression),
        )),
      );
    },

    unary_expression: $ => prec.right(PREC.unary, seq(
      field('operator', choice('!', '~', '-', '+', 'typeof', 'void', 'delete')),
      field('operand', $._expression),
    )),

    update_expression: $ => choice(
      prec.left(PREC.update, seq(field('operand', $._expression), field('operator', choice('++', '--')))),
      prec.right(PREC.update, seq(field('operator', choice('++', '--')), field('operand', $._expression))),
    ),

    conditional_expression: $ => prec.right(PREC.ternary, seq(
      field('condition', $._expression),
      '?',
      field('consequence', $._expression),
      ':',
      field('alternative', $._expression),
    )),

    await_expression: $ => prec(PREC.unary, seq('await', $._expression)),

    yield_expression: $ => prec.right(PREC.yield, seq(
      'yield',
      optional('*'),
      optional($._expression),
    )),

    as_expression: $ => prec.left(PREC.cast, seq(
      $._expression,
      'as',
      choice('const', $._type),
    )),

    satisfies_expression: $ => prec.left(PREC.cast, seq(
      $._expression,
      'satisfies',
      $._type,
    )),

    non_null_expression: $ => prec.left(PREC.update, seq($._expression, '!')),

    _invocation: $ => choice($.call_expression, $.new_expression),

    call_expression: $ => choice(
      prec(PREC.call, seq(
        field('function', $._expression),
        field('type_arguments', optional($.type_arguments)),
        field('arguments', $.arguments),
      )),
      prec(PREC.member, seq(
        field('function', $._expression),
        '?.',
        field('type_arguments', optional($.type_arguments)),
        field('arguments', $.arguments),
      )),
      prec(PREC.member, seq(
        field('function', $._expression),
        field('arguments', $.template_string),
      )),
    ),

    // `new` takes a MEMBER expression, never a CALL. JavaScript binds
    // `new Date().getFullYear()` as `(new Date()).getFullYear()`, and with
    // `$._expression` as the constructor we bound it as
    // `new (Date().getFullYear())` -- a completely different program, in ~29
    // corpus files, parsing without an error. The sweep cannot see this; the
    // shape check found it, because tsc has a `NewExpression` spanning
    // `new Date()` and we had no node with that span.
    //
    // Precedence cannot express it: `member` must still bind INTO the
    // constructor (`new a.b.C()`) while `call` must not, and one number
    // cannot say both. So the constructor gets its own tier -- the member
    // chain over a primary expression, which is exactly the spec's
    // MemberExpression.
    new_expression: $ => prec.right(PREC.new_no_args, seq(
      'new',
      field('constructor', $._constructable),
      field('type_arguments', optional($.type_arguments)),
      field('arguments', optional($.arguments)),
    )),

    _constructable: $ => choice(
      $._constructable_primary,
      alias($._constructable_member, $.member_expression),
      alias($._constructable_subscript, $.subscript_expression),
    ),

    // A `new` chain is itself constructable (`new new X()()`), and so is a
    // parenthesised anything -- `new (f())()` is how you say the reading
    // this rule otherwise forbids.
    _constructable_primary: $ => choice(
      $.new_expression,
      $._name,
      $.this,
      $.super,
      $.parenthesized_expression,
      $.object,
      $.array,
      $.template_string,
      $._literal,
      $.function_expression,
      $.class_definition,
      $.import_meta,
    ),

    _constructable_member: $ => prec(PREC.member, seq(
      field('object', $._constructable),
      field('operator', choice('.', '?.')),
      field('property', choice($._name, alias($._reserved_property, $.identifier), $.private_name)),
    )),

    _constructable_subscript: $ => prec(PREC.member, seq(
      field('object', $._constructable),
      optional('?.'),
      '[',
      field('subscript', $._expressions),
      ']',
    )),

    arguments: $ => seq(
      '(',
      optional(seq(commaSep1($._argument), optional(','))),
      ')',
    ),

    _argument: $ => choice($._expression, $.spread_element),

    spread_element: $ => seq('...', $._expression),

    _access: $ => choice($.member_expression, $.subscript_expression),

    member_expression: $ => prec(PREC.member, seq(
      field('object', $._expression),
      field('operator', choice('.', '?.')),
      field('property', choice($._name, alias($._reserved_property, $.identifier), $.private_name)),
    )),

    subscript_expression: $ => prec(PREC.member, seq(
      field('object', $._expression),
      optional('?.'),
      '[',
      field('subscript', $._expressions),
      ']',
    )),

    arrow_function: $ => prec(PREC.arrow, seq(
      optional('async'),
      choice(
        field('parameter', $._name),
        seq(
          field('type_parameters', optional($.type_parameters)),
          field('parameters', $.parameters),
          optional(seq(':', field('return_type', $._type))),
        ),
      ),
      '=>',
      field('body', choice($._expression, $._body)),
    )),

    function_expression: $ => seq(
      optional('async'),
      'function',
      optional('*'),
      optional(field('name', $._name)),
      field('type_parameters', optional($.type_parameters)),
      field('parameters', $.parameters),
      optional(seq(':', field('return_type', $._type))),
      field('body', $._body),
    ),

    _body: $ => choice($.block),

    block: $ => prec(1, seq('{', repeat($._statement), '}')),

    object: $ => seq(
      '{',
      optional(seq(commaSep1(choice(
        $.pair,
        $.shorthand_property,
        $.function_definition,
        $.spread_element,
      )), optional(','))),
      '}',
    ),

    pair: $ => seq(
      field('key', $._property_name),
      ':',
      field('value', $._expression),
    ),

    shorthand_property: $ => seq(
      field('name', $._name),
      optional(seq('=', field('value', $._expression))),
    ),

    array: $ => seq(
      '[',
      optional(seq(commaSep1(optional(choice($._expression, $.spread_element))), optional(','))),
      ']',
    ),

    parenthesized_expression: $ => seq('(', $._expressions, ')'),

    // ── JSX ──────────────────────────────────────────────────────────
    jsx_element: $ => seq(
      field('open_tag', $.jsx_opening_element),
      repeat($._jsx_child),
      field('close_tag', $.jsx_closing_element),
    ),

    jsx_opening_element: $ => prec.dynamic(-1, seq(
      '<',
      optional(seq(
        field('name', $._jsx_element_name),
        field('type_arguments', optional($.type_arguments)),
        repeat($._jsx_attribute),
      )),
      '>',
    )),

    jsx_closing_element: $ => seq('<', '/', optional(field('name', $._jsx_element_name)), '>'),

    jsx_self_closing_element: $ => prec.dynamic(-1, seq(
      '<',
      field('name', $._jsx_element_name),
      field('type_arguments', optional($.type_arguments)),
      repeat($._jsx_attribute),
      '/',
      '>',
    )),

    jsx_fragment: $ => seq('<', '>', repeat($._jsx_child), '<', '/', '>'),

    // One token shape for every JSX name (dashes allowed): two same-regex
    // tokens competing at one position is the trap the rust grammar
    // documented, so identifier-vs-jsx_identifier never race.
    _jsx_element_name: $ => choice(
      $._jsx_name,
      $.nested_identifier,
      $.jsx_namespace_name,
    ),

    // JSX names must not introduce a SECOND token matching a plain
    // identifier: two tokens with the same regex are resolved by the lexer
    // before the parser sees them, which is what made `<T>(x) => x` lex as
    // JSX and never fork to the generic-arrow reading. Only dashed names
    // (`data-foo`, which an identifier cannot spell) get their own token.
    _jsx_name: $ => choice($.identifier, alias($._jsx_dashed_identifier, $.identifier)),

    jsx_namespace_name: $ => seq($._jsx_name, ':', $._jsx_name),

    _jsx_attribute: $ => choice($.jsx_attribute, $.jsx_expression),

    jsx_attribute: $ => seq(
      field('name', choice($._jsx_attribute_name, $.jsx_namespace_name)),
      optional(seq('=', field('value', choice(
        $.string,
        $.jsx_expression,
        $.jsx_element,
        $.jsx_self_closing_element,
      )))),
    ),

    _jsx_attribute_name: $ => $._jsx_name,
    _jsx_dashed_identifier: _ => /[_\p{XID_Start}][_\p{XID_Continue}]*(-[_\p{XID_Continue}]+)+/,

    _jsx_child: $ => choice(
      $.jsx_text,
      $.jsx_element,
      $.jsx_self_closing_element,
      $.jsx_fragment,
      $.jsx_expression,
    ),

    jsx_text: _ => /[^{}<>\s]([^{}<>]*[^{}<>\s])?/,

    jsx_expression: $ => seq(
      '{',
      optional(choice($._expression, $.sequence_expression, $.spread_element)),
      '}',
    ),

    // ── template strings ─────────────────────────────────────────────
    template_string: $ => seq(
      '`',
      repeat(choice(
        alias($._template_chars, $.string_content),
        alias(token.immediate('$'), $.string_content),
        $.escape_sequence,
        $._interpolation,
      )),
      '`',
    ),

    _template_chars: _ => token.immediate(prec(1, /[^`$\\]+/)),

    _interpolation: $ => choice($.template_substitution),

    template_substitution: $ => seq(
      token.immediate('${'),
      $._expressions,
      '}',
    ),

    escape_sequence: _ => token.immediate(/\\(u\{[0-9a-fA-F]+\}|u[0-9a-fA-F]{4}|x[0-9a-fA-F]{2}|\r?\n|.)/),

    // ── patterns ─────────────────────────────────────────────────────
    _pattern: $ => prec.dynamic(-1, choice(
      $._name,
      $.object_pattern,
      $.array_pattern,
    )),

    object_pattern: $ => seq(
      '{',
      optional(seq(commaSep1(choice(
        $.pair_pattern,
        $.shorthand_property_pattern,
        $.rest_pattern,
      )), optional(','))),
      '}',
    ),

    pair_pattern: $ => seq(
      field('key', $._property_name),
      ':',
      field('value', choice($._pattern, $.member_expression, $.subscript_expression)),
      optional(seq('=', field('default', $._expression))),
    ),

    shorthand_property_pattern: $ => seq(
      field('name', $._name),
      optional(seq('=', field('default', $._expression))),
    ),

    rest_pattern: $ => seq('...', choice($._name, $.member_expression, $.subscript_expression)),

    array_pattern: $ => seq(
      '[',
      optional(seq(commaSep1(optional(choice(
        $._pattern,
        $.assignment_pattern,
        $.rest_pattern,
        $.member_expression,
        $.subscript_expression,
      ))), optional(','))),
      ']',
    ),

    assignment_pattern: $ => seq(
      field('left', $._pattern),
      '=',
      field('right', $._expression),
    ),

    // ── types ────────────────────────────────────────────────────────
    _type: $ => choice(
      alias($.identifier, $.type_identifier),
      alias($._soft_keyword, $.type_identifier),
      $.predefined_type,
      $.generic_type,
      $.nested_type_identifier,
      $.union_type,
      $.intersection_type,
      $.function_type,
      $.constructor_type,
      $.object_type,
      $.array_type,
      $.tuple_type,
      $.conditional_type,
      $.mapped_type,
      $.indexed_access_type,
      $.type_operator,
      $.typeof_type,
      $.literal_type,
      $.template_literal_type,
      $.parenthesized_type,
      $.predicate_type,
      $.import_type,
      $.infer_type,
      $.this_type,
      $.rest_type,
    ),

    generic_type: $ => prec(1, seq(
      field('type', choice(alias($.identifier, $.type_identifier), $.nested_type_identifier)),
      field('type_arguments', $.type_arguments),
    )),

    // The type after `as` is greedy, and `.` was breaking it:
    // `x as React.ReactElement` came out as `(x as React).ReactElement`, a
    // member access on a cast. Both readings are well-formed, so nothing
    // errored; 73 corpus files carried it, and the shape check found them by
    // noticing tsc had a TypeReference spanning `React.ReactElement` where
    // we had none.
    //
    // DYNAMIC precedence, not static. `[member_expression,
    // nested_type_identifier]` is a declared conflict, and a declared
    // conflict is resolved by GLR at parse time -- static `prec` is not
    // consulted for it, which a first attempt at `prec(PREC.member + 1)`
    // demonstrated by changing nothing at all. Only the type reading
    // contains this node, so +1 is enough to decide it.
    nested_type_identifier: $ => prec.dynamic(1, seq(
      field('module', choice($._name, $.nested_identifier)),
      '.',
      field('name', alias($.identifier, $.type_identifier)),
    )),

    // These sit ABOVE `PREC.cast` deliberately. `x as A & B` is
    // `x as (A & B)` in TypeScript, not `(x as A) & B` -- the type after
    // `as`/`satisfies` is greedy. With the type operators below `cast`, the
    // parser reduced `as_expression` at the `&` and read the rest as a
    // bitwise-and over an object literal, which is silently well-formed
    // and silently wrong. Their order relative to each other is unchanged:
    // `&` still binds tighter than `|`.
    union_type: $ => prec.left(PREC.cast + 1, seq(optional($._type), '|', $._type)),
    intersection_type: $ => prec.left(PREC.cast + 2, seq(optional($._type), '&', $._type)),

    function_type: $ => prec.left(1, seq(
      field('type_parameters', optional($.type_parameters)),
      field('parameters', $.parameters),
      '=>',
      field('return_type', $._type),
    )),

    constructor_type: $ => prec.left(1, seq(
      optional('abstract'),
      'new',
      field('type_parameters', optional($.type_parameters)),
      field('parameters', $.parameters),
      '=>',
      field('return_type', $._type),
    )),

    // Members are SEPARATED, not merely juxtaposed. The separator used to be
    // `optional` on every member, which made `{ a: X b: Y }` parse -- tsc
    // rejects it -- and that hole was not only a widening. `readonly` is a
    // soft keyword and so a legal property name, so with juxtaposition
    // allowed, `readonly maxSize: number` had a second reading as TWO
    // members (`readonly`, then `maxSize: number`), and in real .d.ts files
    // that reading sometimes won. It parsed cleanly, so the sweep never saw
    // it; the shape check found it by noticing tsc had a PropertySignature
    // boundary where we had none.
    //
    // Only the LAST member may omit its separator, which is what lets
    // `{ a: X }` and a trailing `;` both work.
    object_type: $ => seq(
      '{',
      optional(choice(',', ';')),
      optional(seq(
        repeat(seq($._type_member, $._member_separator)),
        $._type_member,
        optional($._member_separator),
      )),
      '}',
    ),

    _member_separator: $ => choice(',', ';', $._type_member_end),

    _type_member: $ => choice(
      $.property_signature,
      $.call_signature,
      $.construct_signature,
      $.index_signature,
      alias($.method_signature, $.function_definition),
    ),

    property_signature: $ => seq(
      repeat($._modifier),
      field('name', $._property_name),
      optional('?'),
      optional(seq(':', field('type', $._type))),
    ),

    method_signature: $ => seq(
      repeat($._modifier),
      optional('async'),
      optional(choice('get', 'set')),
      optional('*'),
      field('name', $._property_name),
      optional('?'),
      field('type_parameters', optional($.type_parameters)),
      field('parameters', $.parameters),
      optional(seq(':', field('return_type', $._type))),
    ),

    call_signature: $ => seq(
      field('type_parameters', optional($.type_parameters)),
      field('parameters', $.parameters),
      optional(seq(':', field('return_type', $._type))),
    ),

    construct_signature: $ => seq(
      'new',
      field('type_parameters', optional($.type_parameters)),
      field('parameters', $.parameters),
      optional(seq(':', field('return_type', $._type))),
    ),

    array_type: $ => prec(PREC.member, seq($._type, '[', ']')),

    tuple_type: $ => seq(
      '[',
      optional(seq(commaSep1(choice(
        $._type,
        $.optional_type,
        $.named_tuple_member,
      )), optional(','))),
      ']',
    ),

    // `[opts?: X, ...rest: Y[]]` -- a LABELLED tuple element. The label and
    // the type it labels were loose children of `tuple_type` with no node
    // spanning the pair, so nothing in the tree said which type each label
    // belonged to. 92 files, 640 occurrences. tsc calls it a
    // NamedTupleMember and puts the `...` inside, so the rest form is one
    // node too.
    named_tuple_member: $ => choice(
      seq(field('label', $._name), optional('?'), ':', field('type', $._type)),
      seq('...', field('label', $._name), ':', field('type', $._type)),
    ),

    conditional_type: $ => prec.right(1, seq(
      field('left', $._type),
      'extends',
      field('right', $._type),
      '?',
      field('consequence', $._type),
      ':',
      field('alternative', $._type),
    )),

    // The constraint of `infer R extends C` may not be a bare conditional
    // — the `?` after it belongs to the enclosing conditional type. TS's
    // own grammar makes the same exclusion.
    // The inferred name and its constraint are ONE thing -- tsc models
    // `infer U extends t.Node | null` as an InferType wrapping a
    // TypeParameter spanning `U extends t.Node | null`, and we had no node
    // for that span at all. Aliased to the existing `type_parameter` rather
    // than reusing that rule directly, because it also admits `in`/`out`/
    // `const` variance and an `= default`, none of which is legal after
    // `infer`. Same node name, no widening.
    infer_type: $ => prec.right(1, seq('infer', alias($._infer_parameter, $.type_parameter))),

    _infer_parameter: $ => prec.right(seq(
      field('name', alias($.identifier, $.type_identifier)),
      optional(seq('extends', field('constraint', $._no_conditional_type))),
    )),

    _no_conditional_type: $ => choice(
      alias($.identifier, $.type_identifier),
      $.predefined_type,
      // `infer T extends new (...) => any` — a constraint may be a
      // constructor or function type. Both end in a return type, so they
      // cannot swallow the enclosing conditional's `?`.
      $.constructor_type,
      $.function_type,
      // These carry `prec.dynamic` because `[$._type,
      // $._no_conditional_type]` is a DECLARED conflict, and at
      // `infer R extends P . [` the parser genuinely cannot tell whether `P`
      // ENDS the constraint -- giving `(infer R extends P)[]` -- or
      // CONTINUES into it, giving `infer R extends P[]`. tsc says the
      // constraint is greedy. Both trees hold exactly one `array_type` and
      // one `infer_type`, so nothing asymmetric existed to decide it; this
      // is the asymmetry. Only the alternatives that can keep growing
      // rightward or leftward need it.
      prec.dynamic(1, $.union_type),
      prec.dynamic(1, $.intersection_type),
      $.generic_type,
      $.nested_type_identifier,
      $.object_type,
      prec.dynamic(1, $.array_type),
      $.tuple_type,
      prec.dynamic(1, $.indexed_access_type),
      prec.dynamic(1, $.type_operator),
      $.typeof_type,
      $.literal_type,
      $.template_literal_type,
      $.parenthesized_type,
      $.import_type,
      $.this_type,
    ),

    mapped_type: $ => seq(
      '{',
      optional(choice('+', '-')),
      optional($.readonly_modifier),
      '[',
      alias(choice($.identifier, $._soft_keyword), $.type_identifier),
      'in',
      field('keys', $._type),
      optional(seq('as', field('alias', $._type))),
      ']',
      optional(choice('+', '-')),
      optional('?'),
      optional(seq(':', field('type', $._type))),
      optional(choice(',', ';')),
      '}',
    ),

    indexed_access_type: $ => prec(PREC.member, seq(
      field('object', $._type),
      '[',
      field('index', $._type),
      ']',
    )),

    // Tighter than `intersection_type` (cast+2), which is tighter than
    // `union_type` (cast+1). `readonly string[] | undefined` is
    // `(readonly string[]) | undefined`, and `keyof A | B` is `(keyof A) | B`.
    //
    // This was prec 2 and correct until the type operators were raised above
    // `PREC.cast` to fix `x as A & B` -- that move also lifted them above
    // this one, and `readonly` silently started swallowing the union:
    // `readonly (string[] | undefined)`. 119 corpus files, no error, and no
    // sweep could have seen it. A regression introduced by a fix and caught
    // by the shape check on the very next run, which is the argument for
    // having the check at all.
    type_operator: $ => prec.right(PREC.cast + 3, seq(choice('keyof', 'readonly', 'unique'), $._type)),

    typeof_type: $ => prec.right(seq('typeof', choice($.identifier, $.nested_identifier, $.import_type), field('type_arguments', optional($.type_arguments)))),

    literal_type: $ => choice(
      $.number,
      alias($._negative_number, $.number),
      $.string,
      $.true,
      $.false,
      $.null,
      $.undefined,
    ),

    _negative_number: $ => prec(1, seq('-', $.number)),

    template_literal_type: $ => seq(
      '`',
      repeat(choice(
        alias($._template_chars, $.string_content),
        alias(token.immediate('$'), $.string_content),
        $.escape_sequence,
        alias($.template_type_substitution, $.template_substitution),
      )),
      '`',
    ),

    template_type_substitution: $ => seq(token.immediate('${'), $._type, '}'),

    parenthesized_type: $ => seq('(', $._type, ')'),

    // A predicate type needs `asserts`, or `is`, or both. The old shape --
    // `optional('asserts') name optional(is type)` -- also derived a BARE
    // name, so `let b: never` came out as `predicate_type(identifier)`
    // rather than `predefined_type`, and every plain type name had a
    // spurious second reading to carry through the table.
    predicate_type: $ => prec.right(choice(
      seq('asserts', choice($._name, $.this), optional(seq('is', $._type))),
      seq(choice($._name, $.this), 'is', $._type),
    )),

    import_type: $ => seq('import', '(', $.string, ')', optional(seq('.', choice(alias($.identifier, $.type_identifier), $.nested_type_identifier))), field('type_arguments', optional($.type_arguments))),

    predefined_type: _ => choice('void', 'never', 'unknown', 'symbol'),

    this_type: _ => 'this',
    rest_type: $ => prec(1, seq('...', $._type)),
    optional_type: $ => prec(1, seq($._type, '?')),

    type_parameters: $ => seq(
      '<',
      commaSep1($.type_parameter),
      optional(','),
      '>',
    ),

    type_parameter: $ => seq(
      repeat(choice('in', 'out', 'const')),
      field('name', alias($.identifier, $.type_identifier)),
      optional(seq('extends', field('constraint', $._type))),
      optional(seq('=', field('value', $._type))),
    ),

    // `prec.dynamic(1)`, because `a.b<T>(x)` is genuinely ambiguous with the
    // comparison chain `a.b < T > (x)` and both are complete parses. Nothing
    // asymmetric decided it, so the comparison won and generic calls on a
    // MEMBER function -- `vi.importActual<any>('v')`, `z.custom<string>(f)`
    // -- came out as `binary_expression`. A plain `f<T>(x)` already worked,
    // which is why this hid in a handful of files rather than all of them.
    //
    // +1 rather than more, so it stays BELOW the JSX arbitration: a `<` in
    // a .tsx file still has `jsx_opening_element` at -1 yielding to an
    // arrow, and this does not disturb that balance.
    type_arguments: $ => prec.dynamic(1, seq(
      '<',
      commaSep1($._type),
      optional(','),
      '>',
    )),

    // ── names & literals ─────────────────────────────────────────────
    _name: $ => choice(
      $.identifier,
      alias($._soft_keyword, $.identifier),
      alias($._value_word, $.identifier),
    ),

    _soft_keyword: _ => prec(1, choice(
      'type', 'namespace', 'module', 'declare', 'readonly', 'abstract',
      'override', 'keyof', 'infer', 'satisfies', 'unique',
      'get', 'set', 'of', 'from', 'as', 'async', 'static', 'public',
      'private', 'protected', 'any', 'assert', 'global', 'out', 'accessor',
    )),



    // Words that are legal IDENTIFIERS but must not be reachable as bare
    // TYPE names. Kept disjoint from `_soft_keyword`, which is aliased into
    // both positions -- two hidden rules matching one token is an unresolved
    // conflict, not a choice.
    //
    // `asserts` / `is` are the predicate keywords. Allowing either as a bare
    // type made `a(): asserts this is X` look COMPLETE after `asserts`
    // inside an object type, which made `_type_member_end` valid there. The
    // external scanner is not forked by GLR -- when it returns a boundary
    // that is the only lexing -- so it cut the member at `asserts` and the
    // predicate reading died before the parser saw it. That shredded into
    // four bogus `property_signature`s, which is well-formed, so it swept
    // clean; only the generic form `asserts this is X<Y>` ever errored.
    //
    // The cost is `type asserts = ...` and `let x: is` as type NAMES, which
    // TypeScript permits and nothing in the corpus does.
    // `never` / `unknown` / `symbol` are deliberately NOT here. They are
    // not reserved either, but giving them an identifier reading costs far
    // more than it buys: `expectTypeOf<t1>().toEqualTypeOf<unknown>()` then
    // has a competing generic-arrow fork that wins, and the type-argument
    // reading dies. Measured at 10 -> 23 corpus gaps. A parameter named
    // `symbol` inside a bare function type stays a known gap instead.
    _value_word: _ => prec(1, choice('asserts', 'is')),

    identifier: _ => /[_$\p{XID_Start}][_$\p{XID_Continue}]*/,

    _literal: $ => choice(
      $.string,
      $.number,
      $.true,
      $.false,
      $.null,
      $.undefined,
      $.regex,
    ),

    number: _ => token(choice(
      /0[xX][0-9a-fA-F_]+n?/,
      /0[oO][0-7_]+n?/,
      /0[bB][01_]+n?/,
      /[0-9][0-9_]*n/,
      /([0-9][0-9_]*(\.[0-9_]*)?|\.[0-9_]+)([eE][+-]?[0-9_]+)?/,
    )),

    string: $ => choice(
      token(seq('"', repeat(choice(/[^"\\\r\n]+/, /\\(.|\r?\n)/)), '"')),
      token(seq("'", repeat(choice(/[^'\\\r\n]+/, /\\(.|\r?\n)/)), "'")),
    ),

    regex: $ => seq(
      '/',
      token.immediate(prec(2, /([^/\\\[\r\n]|\\.|\[([^\]\\\r\n]|\\.)*\])+/)),
      token.immediate('/'),
      optional(field('flags', token.immediate(/[a-z]+/))),
    ),

    true: _ => 'true',
    false: _ => 'false',
    null: _ => 'null',
    undefined: _ => 'undefined',


    comment: _ => token(choice(
      seq('//', /[^\r\n]*/),
      seq('/*', /[^*]*\*+([^/*][^*]*\*+)*/, '/'),
    )),
  },
});

function commaSep1(rule) {
  return sep1(rule, ',');
}

function sep1(rule, separator) {
  return seq(rule, repeat(seq(separator, rule)));
}
