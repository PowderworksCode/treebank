/**
 * treebank-cpp: a from-scratch grammar for C++, carrying the treebank
 * vocabulary (DESIGN.md §3) in its parse table.
 *
 * **It extends treebank-c rather than copying it**, through tree-sitter's
 * own grammar inheritance. That is not a convenience: C++ genuinely is C's
 * declarator grammar with more on top, and the alternative — a second copy
 * of the declaration specifiers, the four declarator shapes, the whole
 * preprocessor and the GNU extensions — is a copy that drifts. A fix to the
 * way `int (*f[3])(void)` parses has to land in both languages or in
 * neither, and inheritance is what makes that true by construction.
 *
 * What C++ adds here, and the two things that make it hard:
 *
 * **`<` is ambiguous and stays ambiguous.** `a < b > c` is a comparison of
 * comparisons or an instantiation of the template `a`, and no amount of
 * lexing decides it — the answer is whether `a` names a template, which
 * needs the symbol table a parser does not have. Both readings are carried
 * in the table and the declared conflicts are where that is written down.
 * The scanner is deliberately not asked; see src/scanner.c.
 *
 * **The raw string is the one token a DFA cannot match**, because the
 * program picks its own terminator. That is the whole of the external
 * scanner and the only thing in it.
 *
 * The vocabulary lands as it does in C, with three additions: `_pattern`
 * gets its first C-family member (structured bindings), `_modifier` grows
 * the access and virtual specifiers, and `_member` finally has a body worth
 * threading through — a C struct holds fields, a C++ class holds
 * declarations.
 */

/// <reference types="tree-sitter-cli/dsl" />
// @ts-check

const C = require('../treebank-c/grammar');
const tb = require('../treebank/vocabulary/supertypes.js');

const PREC = {
  LAMBDA: -3,
  ASSIGNMENT: -2,
  CONDITIONAL: -1,
  DEFAULT: 0,
  LOGICAL_OR: 1,
  LOGICAL_AND: 2,
  INCLUSIVE_OR: 3,
  EXCLUSIVE_OR: 4,
  BITWISE_AND: 5,
  EQUAL: 6,
  RELATIONAL: 7,
  THREE_WAY: 8,
  SHIFT: 9,
  ADD: 10,
  MULTIPLY: 11,
  CAST: 12,
  SIZEOF: 13,
  UNARY: 14,
  NEW: 15,
  CALL: 16,
  FIELD: 17,
  SUBSCRIPT: 18,
  SCOPE: 19,
};

module.exports = grammar(C, {
  name: 'cpp',

  externals: ($, previous) => previous.concat([
    $.raw_string_literal,
  ]),

  supertypes: ($, previous) => previous.concat(
    tb.assertTableTerms(['_pattern']).map((name) => $[name]),
  ),

  conflicts: ($, previous) => previous.concat([
    // C dropped this one when `macro_modifier` subsumed it there. C++
    // still needs it: `unexpanded_macro` is reachable from a namespace
    // body here, so `namespace { X signed …` has the two readings C no
    // longer has.
    [$.unexpanded_macro, $._sizeable_type],
    [$.unexpanded_macro, $._expression],
    [$.unexpanded_macro, $._type, $._expression],
    [$.unexpanded_macro, $.namespace_definition],
    [$.abstract_function_declarator, $._function_suffix],
    [$._declaration_specifiers, $.type_descriptor],
    [$._declaration_modifiers, $._type_qualifier_or_attribute],
    [$._modifier, $._type_qualifier_or_attribute],
    [$._type, $._assignment_target],
    [$.comma_expression, $.initializer_pair],
    [$.parameter_list],
    [$._expression, $.lambda_capture],
    [$._type, $._sizeable_type],
    [$._type, $.class_specifier],
    [$._type, $.union_specifier],
    [$._type, $.struct_specifier],
    [$._declarator, $._expression, $._assignment_target],
    [$._declarator, $._expression],
    [$.comma_expression, $._initializer_list_item],
    [$.compound_statement, $.initializer_list],
    [$._declarator_with_init, $._field_declarator],
    [$.declaration, $.field_declaration],
    [$.reference_declarator, $.abstract_reference_declarator],
    [$._declaration_specifiers, $.friend_declaration],
    [$.type_descriptor],
    [$._declarator, $._assignment_target],
    [$._type, $._declarator, $._expression],
    [$.preproc_else, $._declaration],
    [$.preproc_ifdef, $._declaration],
    [$.translation_unit, $._declaration],
    // The `<` question, in every shape it takes. None of these is
    // resolvable without knowing whether the name on the left is a
    // template, so both readings are carried and the tree records which
    // one won.
    [$.class_specifier],
    [$.struct_specifier],
    [$.union_specifier],
    [$.enum_specifier],
    [$.base_class_clause],
    [$.operator_name],
  ]),

  rules: {
    // ── the declaration specifiers ────────────────────────────────────
    // Written out to restore C's SYMMETRIC form: the same repetition
    // before the type and after it. C's own rule admits a `macro_modifier`
    // in the leading repetition only — `ZEND_API void f(void);` — which
    // makes the two repetitions different symbols, and a parser that has
    // just reduced a modifier can no longer tell which of them it is in.
    //
    // C absorbs that with six declared conflicts and thirty kilobytes of
    // table. C++ does not: the same change asks for a conflict on
    // `_out_of_line_definition`, then on the two member forms, then on
    // `macro_modifier` against `_expression` in four widening combinations
    // — the runaway shape DESIGN.md §8.1 records, where each conflict
    // spawns the next. The rule is worth less here for the reason
    // `_top_level_item` gives above: a C++ header's declarations open with
    // `template`, `namespace` and access labels far more often than with a
    // visibility macro.
    //
    // Everything else C added for unexpanded macros is inherited and does
    // generate: the macro after a declarator, beside a pointer's
    // qualifiers, on an enumerator and inside a typedef.
    _declaration_specifiers: $ => seq(
      repeat($._declaration_modifiers),
      field('type', $._type),
      repeat($._declaration_modifiers),
    ),

    // Without C's leading `macro_modifier` alternative, for the same
    // reason: `( identifier • *` is `(a * b)` here as readily as it is a
    // declarator, and the macro reading is a third one on top of that.
    parenthesized_declarator: $ => prec.dynamic(-10, seq(
      '(',
      $._declarator,
      ')',
    )),

    // And the same for the pointer's qualifier run: `* identifier •  (` is
    // a multiplication here too.
    pointer_declarator: $ => prec.dynamic(1, prec.right(seq(
      '*',
      repeat($._type_qualifier_or_attribute),
      field('declarator', $._declarator),
    ))),

    // Without C's trailing `macro_modifier` run. A C++ member list also
    // holds function DEFINITIONS, so after a declarator an identifier is
    // already ambiguous between the start of a definition and something
    // else; C's alignment-macro run makes it a three-way one. C has no
    // member definitions and pays nothing for the rule.
    field_declaration: $ => seq(
      $._declaration_specifiers,
      commaSep($._field_declarator),
      ';',
    ),

    // The suffix form only. C's rule has a second alternative that opens
    // with the macro — `ZEND_API void ZEND_FASTCALL f(…)` — which puts
    // `macro_modifier` at the START of a declarator, and a declarator
    // begins in far more states here than in C: a lambda's init-capture is
    // one, and `_Static_assert(([x](…)…` is where the table said so. The
    // macro after the declarator costs nothing and is kept.
    macro_attributed_declarator: $ => prec.right(seq(
      field('declarator', choice(
        $.function_declarator,
        $.array_declarator,
        $.parenthesized_declarator,
        $.attributed_declarator,
      )),
      repeat1(field('attribute', $.macro_attribute)),
    )),

    // ── what a translation unit may hold ──────────────────────────────
    // Written out rather than extended, in order to LEAVE OUT
    // `unexpanded_macro`. C admits a bare `__BEGIN_DECLS` at file scope
    // because a great many C headers open with one; C++ headers open with
    // `namespace` instead, so the rule earns much less here — and it costs
    // far more, because every C++ construct that can begin with an
    // identifier now has a second reading. Measured: it was the source of
    // the last four conflicts the table needed before this, each one a
    // wider combination of the same three rules than the last.
    _top_level_item: $ => choice(
      $._declaration,
      $._directive,
      $.empty_declaration,
      $.asm_statement,
      $.namespace_definition,
      $.linkage_specification,
      $.using_declaration,
      $.alias_declaration,
      $.template_declaration,
      $.explicit_instantiation,
      $.namespace_alias_definition,
      $._out_of_line_definition,
    ),

    _declaration: ($, previous) => choice(
      previous,
      $.namespace_definition,
      $.linkage_specification,
      $.using_declaration,
      $.alias_declaration,
      $.template_declaration,
    ),

    // ── namespaces ────────────────────────────────────────────────────
    // `namespace a::b::c { … }` is C++17's nested form and is one
    // definition, not three; `inline namespace` is C++11's versioning
    // device and appears throughout libstdc++.
    namespace_definition: $ => seq(
      optional('inline'),
      'namespace',
      optional(field('name', choice($.identifier, $.nested_namespace_specifier))),
      repeat(choice($._attribute, $.unexpanded_macro)),
      field('body', $.declaration_list),
    ),

    // `namespace std _GLIBCXX_VISIBILITY(default) { … }`, and
    // `_GLIBCXX_BEGIN_NAMESPACE_VERSION` on a line of its own inside it.
    // A macro that expands to an attribute, to `inline namespace __8 {`, or
    // to nothing at all — and unexpanded, it is none of those.
    //
    // C has the same rule and admits it at FILE SCOPE, which a C++ grammar
    // cannot afford: a bare name where a declaration goes hands every
    // `f(x)` in C++ a second reading, and that was measured — it was the
    // last four conflicts before this table stopped converging. Here it is
    // reachable from exactly two places, a namespace head and a namespace
    // body, and both are already committed by the `namespace` keyword
    // before it appears. On libstdc++ the two positions are 200 and 73
    // files of 791.
    unexpanded_macro: $ => seq(
      field('name', $.identifier),
      optional(field('arguments', $.argument_list)),
    ),

    nested_namespace_specifier: $ => seq(
      optional('inline'),
      $.identifier,
      repeat1(seq('::', optional('inline'), $.identifier)),
    ),

    namespace_alias_definition: $ => seq(
      'namespace',
      field('name', $.identifier),
      '=',
      field('value', choice($.identifier, $.qualified_identifier)),
      ';',
    ),

    // `extern "C" { … }` — and the brace-less `extern "C" int f(void);`,
    // which is how a single declaration is marked.
    //
    // This is the construct that forces treebank-preprocessing to exist on
    // the C side: compiled as C, the `#ifdef __cplusplus` around it deletes
    // both braces, and they sit in different conditionals so no single tree
    // holds both configurations. Compiled as C++ it is an ordinary node,
    // which is the asymmetry worth noticing.
    linkage_specification: $ => seq(
      'extern',
      field('value', $.string_literal),
      // `$.declaration` and not `$._declaration`: `extern "C" int f(void);`
      // is the brace-less form, and a namespace, a template or a function
      // definition is not. Offering all of them made every top-level item
      // ambiguous with the one before it.
      field('body', choice($.declaration_list, $.declaration)),
    ),

    declaration_list: $ => seq(
      '{',
      repeat(choice($._block_item, $.unexpanded_macro, $._out_of_line_definition)),
      '}',
    ),

    // `using std::swap;`, `using namespace std;`, `using enum Colour;`
    using_declaration: $ => seq(
      'using',
      optional(choice('namespace', 'enum')),
      field('name', choice($.identifier, $.qualified_identifier)),
      ';',
    ),

    // C++11's `using X = Y;`, which replaced `typedef Y X;` and reads the
    // right way round.
    alias_declaration: $ => seq(
      'using',
      field('name', $.identifier),
      repeat($._attribute),
      '=',
      field('type', $.type_descriptor),
      ';',
    ),

    // ── templates ─────────────────────────────────────────────────────
    template_declaration: $ => seq(
      'template',
      field('parameters', $.template_parameter_list),
      optional(field('constraint', $.requires_clause)),
      // Named rather than `$._declaration`, because most of what that
      // alternation holds cannot be templated: there is no template
      // namespace, no template `using namespace`, no template `extern "C"`.
      choice(
        $.declaration,
        $.function_definition,
        $.type_definition,
        $.alias_declaration,
        $.concept_definition,
        $.template_declaration,
        $.field_declaration,
        $.friend_declaration,
      ),
    ),

    template_parameter_list: $ => seq(
      '<',
      commaSep(choice(
        $.type_parameter_declaration,
        $.variadic_type_parameter_declaration,
        $.template_template_parameter_declaration,
        $.parameter_declaration,
        $.optional_parameter_declaration,
        $.variadic_parameter_declaration,
      )),
      alias(token(prec(1, '>')), '>'),
    ),

    // Above `class_specifier` by precedence rather than by a declared
    // conflict: inside a template parameter list a `class T` is a type
    // parameter and never a class definition, so there is nothing to carry
    // both readings for.
    type_parameter_declaration: $ => prec(1, seq(
      choice('typename', 'class'),
      optional(field('name', $.identifier)),
      optional(seq('=', field('default_type', $.type_descriptor))),
    )),

    variadic_type_parameter_declaration: $ => seq(
      choice('typename', 'class'),
      '...',
      optional(field('name', $.identifier)),
    ),

    template_template_parameter_declaration: $ => seq(
      'template',
      field('parameters', $.template_parameter_list),
      choice($.type_parameter_declaration, $.variadic_type_parameter_declaration),
    ),

    optional_parameter_declaration: $ => seq(
      $._declaration_specifiers,
      optional(field('declarator', choice($._declarator, $._abstract_declarator))),
      '=',
      field('default_value', $._expression),
    ),

    variadic_parameter_declaration: $ => seq(
      $._declaration_specifiers,
      '...',
      optional(field('declarator', $._declarator)),
    ),

    // An instantiation: `vector<int>` in type position, `f<int>(x)` in
    // expression position. ONE rule, reachable from both — which is what
    // `identifier` already does, and what this had as two identical rules
    // before. The two differed only in which alternation reached them, so
    // every instantiation in the corpus carried both readings and the
    // parser had to be told, by a declared conflict, that they were the
    // same thing. A node type per position is not worth that: the position
    // is already in the tree.
    template_type: $ => prec(1, seq(
      field('name', choice($.identifier, $.qualified_identifier)),
      field('arguments', $.template_argument_list),
    )),

    // `>>` closing two nested argument lists is C++11's rule and predates
    // it in every real compiler, so the closing `>` is its own token and
    // never the shift operator here.
    template_argument_list: $ => seq(
      '<',
      commaSep(choice($.type_descriptor, $._expression, $.parameter_pack_expansion)),
      alias(token(prec(1, '>')), '>'),
    ),

    explicit_instantiation: $ => prec(1, seq(
      optional('extern'),
      'template',
      choice($.declaration, $.function_definition),
    )),

    // C++20 concepts, in the two forms that carry the weight: the clause on
    // a template and the `requires` expression that defines one.
    // The constraint is an ordinary `_expression`: `requires C<T>`,
    // `requires (A<T> && B<T>)`, `requires requires (T x) { … }`. A
    // parallel alternation for it — which is what this rule had first — is
    // a second expression grammar reachable from the same positions as the
    // first, and every name in it becomes ambiguous with itself.
    requires_clause: $ => prec.right(seq(
      'requires',
      // `template_type` is named beside `_expression` because it is NOT a
      // member of it — see `_expression` — and `requires Integral<T>` is
      // the commonest constraint there is.
      field('constraint', choice($._expression, $.template_type)),
    )),

    requires_expression: $ => seq(
      'requires',
      optional(field('parameters', $.parameter_list)),
      field('body', $.requirement_seq),
    ),

    requirement_seq: $ => seq('{', repeat($._requirement), '}'),

    _requirement: $ => choice(
      seq($._expression, ';'),
      seq('{', $._expression, '}', optional('noexcept'), optional(seq('->', $._type)), ';'),
      seq('typename', $._type, ';'),
      seq($.requires_clause, ';'),
    ),

    concept_definition: $ => seq(
      'concept',
      field('name', $.identifier),
      '=',
      field('value', $._expression),
      ';',
    ),

    // ── classes ───────────────────────────────────────────────────────
    // `class` is a specifier like `struct`, and C's `struct_specifier` and
    // `union_specifier` are extended in place rather than duplicated: a C++
    // struct takes base classes and access specifiers too, and the only
    // difference between the two keywords is the default access.
    //
    // `_type` itself is written out rather than extended, to leave
    // `macro_type_specifier` — C's `SOME_MACRO(int) x;` — behind. In C it is
    // the only reading available for a type-shaped macro; in C++ the same
    // shape is a constructor call, a functional cast, a template
    // instantiation or a declarator, and offering a fifth reading of it
    // made every one of those ambiguous with the others.
    _type: $ => choice(
      $.primitive_type,
      $.sized_type_specifier,
      $.struct_specifier,
      $.union_specifier,
      $.enum_specifier,
      $.typeof_specifier,
      $.atomic_type_specifier,
      alias($.identifier, $.type_identifier),
      $.class_specifier,
      $.template_type,
      $.qualified_identifier,
      $.decltype,
      $.placeholder_type_specifier,
      $.dependent_type,
    ),

    // The name is a plain identifier. `template<> struct X<int> { … }` —
    // an explicit specialisation, whose name is an instantiation — is a
    // ledgered known gap: admitting `template_type` and
    // `qualified_identifier` there gave every `struct`, `union` and `class`
    // three readings of its own name, and `_type` reaches all three of
    // those forms by other routes anyway.
    class_specifier: $ => seq(
      'class',
      repeat($._attribute),
      choice(
        seq(
          field('name', choice($.identifier, $.template_type)),
          optional($.virtual_specifier),
          optional(field('bases', $.base_class_clause)),
          optional(field('body', $.field_declaration_list)),
        ),
        seq(
          optional(field('bases', $.base_class_clause)),
          field('body', $.field_declaration_list),
        ),
      ),
    ),

    // The extension adds what C's rule LACKS and requires it. C already
    // has `struct X { … }`, so an alternative that made the base clause
    // optional would be a second, identical reading of every plain struct
    // in the corpus — which is exactly what it was, and what the declared
    // conflict on `struct_specifier` was paying for.
    struct_specifier: ($, previous) => choice(
      previous,
      seq(
        'struct',
        repeat($._attribute),
        field('name', $.template_type),
        optional($.virtual_specifier),
        optional(field('bases', $.base_class_clause)),
        optional(field('body', $.field_declaration_list)),
      ),
      seq(
        'struct',
        repeat($._attribute),
        field('name', $.identifier),
        optional($.virtual_specifier),
        field('bases', $.base_class_clause),
        optional(field('body', $.field_declaration_list)),
      ),
    ),

    // The extension adds what C's rule LACKS and requires it. C already
    // has `union X { … }`, so an alternative that made the base clause
    // optional would be a second, identical reading of every plain union
    // in the corpus — which is exactly what it was, and what the declared
    // conflict on `union_specifier` was paying for.
    union_specifier: ($, previous) => choice(
      previous,
      seq(
        'union',
        repeat($._attribute),
        field('name', $.template_type),
        optional(field('bases', $.base_class_clause)),
        optional(field('body', $.field_declaration_list)),
      ),
      seq(
        'union',
        repeat($._attribute),
        field('name', $.identifier),
        field('bases', $.base_class_clause),
        optional(field('body', $.field_declaration_list)),
      ),
    ),

    // `enum class Colour : uint8_t { … }` — the scoped enum, which is the
    // one C++ addition to `enum` that changes its syntax rather than only
    // its lookup.
    enum_specifier: ($, previous) => choice(
      previous,
      seq(
        'enum',
        choice('class', 'struct'),
        repeat($._attribute),
        choice(
          seq(
            field('name', $.identifier),
            optional(seq(':', field('underlying_type', $._type))),
            optional(field('body', $.enumerator_list)),
          ),
          seq(
            optional(seq(':', field('underlying_type', $._type))),
            field('body', $.enumerator_list),
          ),
        ),
      ),
    ),

    base_class_clause: $ => seq(
      ':',
      commaSep1(seq(
        optional($.access_specifier),
        optional('virtual'),
        optional($.access_specifier),
        choice($.identifier, $.qualified_identifier, $.template_type),
        optional('...'),
      )),
    ),

    // A class body holds DECLARATIONS where a C struct body holds fields,
    // which is why `_member` grows here rather than staying what C left it.
    // Written out rather than extended, to LEAVE OUT `unexpanded_macro`.
    // C admits a bare macro as a whole member because `struct
    // k_atm_aal_stats { __AAL_STAT_ITEMS };` is what a kernel header looks
    // like; here a member may be a function DECLARATION, so `f() const` and
    // a macro call followed by a qualifier are the same tokens, and the
    // table said so at `struct { identifier ( ) •  const`. It is the same
    // exclusion `_top_level_item` makes above and for the same reason.
    _member: $ => choice(
      $.field_declaration,
      $.static_assert_declaration,
      $._directive_in_field_declaration_list,
      $.access_label,
      $.function_definition,
      $._out_of_line_definition,
      $._member_definition,
      $._member_declaration,
      $.template_declaration,
      $.friend_declaration,
      $.using_declaration,
      $.alias_declaration,
      $.type_definition,
      $.namespace_alias_definition,
      $.explicit_instantiation,
      $.concept_definition,
    ),

    // Two nodes, because the keyword does two jobs. `public` in a base
    // clause modifies how a base is inherited; `public:` in a class body is
    // a member that changes the default for everything after it. Baking the
    // colon into one node meant `class D : public B` did not parse at all.
    //
    // Both are named nodes rather than anonymous tokens, because an
    // anonymous token can never carry a role (DESIGN.md §3.2).
    access_specifier: _ => choice('public', 'private', 'protected'),

    access_label: $ => seq($.access_specifier, ':'),

    // `friend class X;`, `friend void f(T);`. Named rather than
    // `$._declaration` for the same reason as the template above: a friend
    // namespace is not a thing.
    friend_declaration: $ => seq(
      'friend',
      choice(
        $.declaration,
        $.function_definition,
        $.field_declaration,
        seq(optional(choice('class', 'struct', 'union')), $._type, ';'),
      ),
    ),

    // ── constructors, destructors, conversion operators ──────────────
    // None of these is a rule of its own, and that is the point. A
    // constructor is a member function with NO RETURN TYPE — that is the
    // whole of what distinguishes it — and `~Point()`, `A::~A()` and
    // `operator bool()` are the same shape with a different name in the
    // declarator. Giving each its own rule meant three near-copies of
    // `function_definition` whose only difference was the node name, and a
    // parser that had to guess between them at every member.
    //
    // So `function_definition` grows one alternative with the type omitted,
    // and the declarator says which kind it is: a `destructor_name` inside
    // it makes it a destructor, an `operator_name` a conversion operator, a
    // plain name a constructor. A query asks the tree rather than the node
    // type, and the tree is the language's own answer.
    // An out-of-line definition, reachable from a namespace or a class body
    // and NOT from a statement — C++ has no nested function definitions, and
    // the restriction is load-bearing rather than pedantic. While these sat
    // in `function_definition`, which `_statement` reaches through
    // `_declaration`, the `prec(2)` on their declarator won inside every
    // block and `std::__terminate();` read as a declaration of
    // `std::__terminate` that then errored on its own semicolon.
    _out_of_line_definition: $ => alias($._out_of_line_definition_inner, $.function_definition),

    _out_of_line_definition_inner: $ => choice(
      // WITH a return type: `int Widget::count() const { … }`.
      seq(
        $._declaration_specifiers,
        field('declarator', alias($._qualified_member_declarator, $.function_declarator)),
        choice(field('body', $.compound_statement), seq($.default_method_clause, ';')),
      ),
      // WITHOUT one: `Widget::Widget(int n) : count_(n) { }`,
      // `Widget::~Widget() { }`, `Widget::operator bool() const { … }`.
      seq(
        repeat($._declaration_modifiers),
        field('declarator', alias($._qualified_member_declarator, $.function_declarator)),
        optional(field('initializers', $.field_initializer_list)),
        choice(field('body', $.compound_statement), seq($.default_method_clause, ';')),
      ),
    ),

    // Two declarator shapes, and WHERE each is reachable from is the whole
    // of what makes them affordable.
    //
    // A no-return-type form begins with a bare name, so offering one
    // wherever a declaration may go hands every `f(x)` in the language a
    // second reading — and the parse table stops converging. It was
    // measured: with one shared rule reachable from `_declaration`,
    // generation ran past fifteen minutes and produced a 65 MB table.
    //
    // Out of line, the declarator is QUALIFIED — `Widget::Widget(int)` —
    // and a leading `A::` cannot begin anything else, so that form is safe
    // wherever a declaration goes. In class it is unqualified, and that
    // form is reachable only from `_member`, where there is nothing else
    // `Widget(` can be: a member function declaration needs a return type
    // before its name.
    //
    // Neither takes `$._declarator`. A constructor is never a pointer, an
    // array or a parenthesised declarator, and offering those shapes at
    // the start of every member costs states for nothing.
    _qualified_member_declarator: $ => prec(2, seq(
      field('declarator', $.qualified_identifier),
      field('parameters', $.parameter_list),
      repeat($._function_suffix),
    )),

    _unqualified_member_declarator: $ => prec(2, seq(
      field('declarator', choice(
        $.identifier,
        $.destructor_name,
        $.operator_name,
        $.template_type,
      )),
      field('parameters', $.parameter_list),
      repeat($._function_suffix),
    )),

    _member_definition: $ => alias($._member_definition_inner, $.function_definition),

    _member_definition_inner: $ => seq(
      repeat($._declaration_modifiers),
      field('declarator', alias($._unqualified_member_declarator, $.function_declarator)),
      optional(field('initializers', $.field_initializer_list)),
      choice(field('body', $.compound_statement), seq($.default_method_clause, ';')),
    ),

    // The same shape with a `;` where the body goes: `Widget(int);`,
    // `virtual ~Widget() override;`, `operator bool() const;`. Aliased to
    // `declaration` because that is what it is, and because C's
    // `int f(void);` already comes out as a `declaration` holding a
    // `function_declarator` — this is the same tree with the type absent.
    _member_declaration: $ => alias($._member_declaration_inner, $.declaration),

    _member_declaration_inner: $ => seq(
      repeat($._declaration_modifiers),
      field('declarator', alias($._unqualified_member_declarator, $.function_declarator)),
      ';',
    ),

    // `explicit` alone. C++20's `explicit(cond)` is a ledgered known gap:
    // the parenthesised form is indistinguishable from a call until it
    // closes, and it appears in a distribution's C++ approximately never.
    explicit_specifier: _ => 'explicit',

    // `Point(int x) : x_(x), y_(0) {}` — the member initializer list, which
    // is a `_clause` and not an assignment: it initialises rather than
    // stores, and that is the distinction C++ cares most about.
    field_initializer_list: $ => seq(
      ':',
      commaSep1($.field_initializer),
    ),

    field_initializer: $ => seq(
      field('name', $.identifier),
      field('value', choice($.initializer_list, $.argument_list)),
      optional('...'),
    ),

    // `= default` and `= delete` are not initializers and not assignments —
    // they say what the compiler should write instead of a body.
    default_method_clause: $ => seq('=', choice('default', 'delete')),

    // Everything that may follow a member function's parameter list.
    _function_suffix: $ => choice(
      $.type_qualifier,
      $.virtual_specifier,
      $.noexcept_specifier,
      $.throw_specifier,
      $.trailing_return_type,
      $.ref_qualifier,
      $._attribute,
    ),

    virtual_specifier: _ => choice('virtual', 'override', 'final'),

    ref_qualifier: _ => choice('&', '&&'),

    noexcept_specifier: $ => prec.right(seq(
      'noexcept',
      optional(seq('(', optional($._expression), ')')),
    )),

    // The pre-C++11 exception specification, still all over older headers.
    throw_specifier: $ => seq(
      'throw',
      '(',
      commaSep($.type_descriptor),
      ')',
    ),

    // `auto f(int x) -> int` — C++11's trailing return, and the only way to
    // write a return type that mentions the parameters.
    trailing_return_type: $ => seq('->', field('type', $.type_descriptor)),

    // ── types and names ───────────────────────────────────────────────
    // `A::B::C` — one node whatever its depth, because a query asking "what
    // is this called" should not have to know how many `::` were involved.
    qualified_identifier: $ => prec.right(PREC.SCOPE, seq(
      field('scope', optional(choice(
        $.identifier,
        $.template_type,
        $.decltype,
        $.qualified_identifier,
      ))),
      '::',
      field('name', choice(
        $.identifier,
        $.qualified_identifier,
        $.template_type,
          $.operator_name,
        $.destructor_name,
      )),
    )),

    destructor_name: $ => prec(1, seq('~', $.identifier)),

    // `operator+`, `operator[]`, `operator bool`, `operator""_km`, and the
    // three-word ones (`operator new[]`). Spelled out rather than taken as
    // `operator` plus a token soup, because the name is what a query wants.
    operator_name: $ => prec(1, seq(
      'operator',
      choice(
        seq(choice('new', 'delete'), optional(seq('[', ']'))),
        '+', '-', '*', '/', '%', '^', '&', '|', '~', '!', '=', '<', '>',
        '+=', '-=', '*=', '/=', '%=', '^=', '&=', '|=', '<<', '>>',
        '>>=', '<<=', '==', '!=', '<=', '>=', '<=>', '&&', '||',
        '++', '--', ',', '->*', '->', '()', '[]',
        seq('""', $.identifier),
        $._type,
      ),
    )),

    // C++11's `decltype(x)`. Its operand is an expression, always — unlike
    // `typeof`, which C spells both ways.
    decltype: $ => seq(
      'decltype',
      '(',
      choice($._expression, 'auto'),
      ')',
    ),

    // `auto`, and C++20's `Concept auto`. It is a type SLOT rather than a
    // type: what fills it is decided by the initializer.
    placeholder_type_specifier: $ => prec(1, seq(
      optional(field('constraint', choice($.identifier, $.qualified_identifier, $.template_type))),
      choice('auto', $.decltype),
    )),

    // `typename T::type` inside a template — the disambiguator that tells
    // the compiler a dependent name is a type. It exists precisely because
    // the parse is ambiguous without it, which is a fact worth a node.
    dependent_type: $ => prec.right(1, seq('typename', $._type)),

    // `access_specifier` is deliberately NOT here. `public` modifies how a
    // base is inherited and, with a colon, labels a run of members; it does
    // not modify a declaration. Admitting it as a `_modifier` made
    // `class W { public int x; };` — a `public:` whose colon was left out —
    // parse as a field with a `public` modifier, which
    // `test/negative/access-specifier-without-a-colon.cc` now catches.
    _modifier: ($, previous) => choice(
      previous,
      $.virtual_specifier,
      $.explicit_specifier,
    ),

    storage_class_specifier: (_, previous) => choice(
      previous,
      'mutable',
      'consteval',
      'constinit',
    ),

    // ── declarators ───────────────────────────────────────────────────
    // A reference is a declarator shape C does not have. `&&` is the
    // rvalue reference and is one token, never two `&`s, which is why it
    // sits beside `&` here rather than being built from it.
    // `qualified_identifier` is what an out-of-line definition needs —
    // `void Widget::reset() {}` — and `reference_declarator` is the shape C
    // does not have. `structured_binding_declarator` is NOT here: a `[` at
    // the start of a declarator is already an array declarator, an
    // attribute and a lambda, and a fourth reading of it is not
    // affordable. It stays reachable from `for_range_loop`, which is where
    // C++17 code puts it. `operator_name`, `destructor_name` and
    // `template_type` are reachable through `qualified_identifier` and
    // through the member declarators, where a `::` or a class body has
    // already settled what they are, rather than being offered bare in
    // every declarator position.
    // `reference_declarator` is the shape C does not have, and it is the
    // only thing added here.
    //
    // `qualified_identifier` is NOT: an out-of-line definition —
    // `void Widget::reset() { }` — reaches its declarator through
    // `_qualified_member_declarator` instead, where a `prec` decides it
    // outright. Offering `A::b` as a declarator as well as a type and an
    // expression made the parser carry the same three symbols in three
    // roles through every position, which is where this table stopped
    // converging. `int A::x = 5;` — an out-of-line static member
    // definition — is the form that costs, and it is a ledgered gap.
    _declarator: ($, previous) => choice(
      previous,
      $.reference_declarator,
      // `bool operator==(const W&) const;` — an operator with a return
      // type, which is most of them. This one is cheap where
      // `qualified_identifier` was not: `operator` is a KEYWORD, so the
      // token settles the reading before anything else is offered.
      $.operator_name,
    ),

    _abstract_declarator: ($, previous) => choice(
      previous,
      $.abstract_reference_declarator,
    ),

    // `const auto& [key, value]` — the reference wraps the binding, so the
    // binding has to be reachable through it as well as directly.
    reference_declarator: $ => prec.dynamic(1, prec.right(seq(
      choice('&', '&&'),
      repeat($._type_qualifier_or_attribute),
      field('declarator', optional(choice($._declarator, $._pattern))),
    ))),

    abstract_reference_declarator: $ => prec.right(seq(
      choice('&', '&&'),
      repeat($._type_qualifier_or_attribute),
      field('declarator', optional($._abstract_declarator)),
    )),

    // C++17's structured binding: `auto [key, value] = *it;`. This is the
    // first `_pattern` in the C family — a destructuring position, which is
    // exactly what the term is for, and which C has nowhere at all.
    structured_binding_declarator: $ => prec(1, seq(
      '[',
      commaSep1($.identifier),
      ']',
    )),

    _pattern: $ => choice($.structured_binding_declarator),

    // A function declarator in C++ carries everything that may follow the
    // parameter list, which in C is only an attribute — and it takes a
    // parameter list and nothing else. C's `identifier_list`, the K&R
    // `f(a, b)` whose parameters are typed on the following lines, is
    // written out here: C++ never had it, and its presence made every
    // single-argument call ambiguous with a declarator.
    function_declarator: $ => prec.right(1, seq(
      field('declarator', $._declarator),
      field('parameters', $.parameter_list),
      repeat($._function_suffix),
    )),

    // `[](auto x) -> int { … }` — a lambda's declarator is an abstract
    // function declarator, and `-> int` is a function suffix, which C's
    // rule has no slot for because C has no trailing return type.
    abstract_function_declarator: ($, previous) => choice(
      previous,
      prec.right(1, seq(
        field('declarator', optional($._abstract_declarator)),
        field('parameters', $.parameter_list),
        repeat1($._function_suffix),
      )),
    ),

    // A parameter may be defaulted or a pack, neither of which C has.
    parameter_list: ($, previous) => choice(
      previous,
      seq(
        '(',
        commaSep(choice(
          $.parameter_declaration,
          $.optional_parameter_declaration,
          $.variadic_parameter_declaration,
          $.variadic_parameter,
        )),
        ')',
      ),
    ),

    // `T x{1};` — brace initialisation. The parenthesised form `T x(1);`
    // is LEFT OUT, and it is the most vexing parse that leaves it out:
    // `T x(1)` and `T x(int)` are the same shape, a declarator whose
    // parameter list might be an argument list, and carrying both readings
    // through every declaration in the language is the most expensive thing
    // this grammar could do. `T x{1}` and `T x = 1` both parse; `T x(1)` is
    // a ledgered known gap.
    init_declarator: ($, previous) => choice(
      previous,
      seq(
        field('declarator', $._declarator),
        field('value', $.initializer_list),
      ),
    ),

    _field_declarator: ($, previous) => choice(
      previous,
      $.init_declarator,
      seq(field('declarator', $._declarator), $.default_method_clause),
    ),

    // ── statements ────────────────────────────────────────────────────
    _statement: ($, previous) => choice(
      previous,
      $.co_return_statement,
    ),

    // `try`/`catch` is `_control_flow` by the vocabulary's own definition,
    // which names `try_statement` there alongside the branches and loops.
    _control_flow: ($, previous) => choice(
      previous,
      $.try_statement,
    ),

    try_statement: $ => seq(
      'try',
      field('body', $.compound_statement),
      repeat1($.catch_clause),
    ),

    catch_clause: $ => seq(
      'catch',
      field('parameters', $.parameter_list),
      field('body', $.compound_statement),
    ),

    _loop: ($, previous) => choice(
      previous,
      $.for_range_loop,
    ),

    // `for (auto& [k, v] : map)` — and the C++20 init-statement before it.
    for_range_loop: $ => seq(
      'for',
      '(',
      optional(seq(field('initializer', $.declaration))),
      $._declaration_specifiers,
      field('declarator', choice($._declarator, $._pattern)),
      ':',
      field('right', choice($._expression, $.initializer_list)),
      ')',
      field('body', $._statement),
    ),

    co_return_statement: $ => seq(
      'co_return',
      optional(field('value', $._expression)),
      ';',
    ),

    // `if constexpr`, which is the one part of C++'s condition syntax that
    // costs nothing: the keyword decides it before the paren is reached.
    //
    // A DECLARATION in a condition — `if (auto* p = find(x))`, `while (auto
    // x = next())` — and C++17's init-statement before it are both left
    // out, and the reason is the open paren: admitting a declaration there
    // means every `if (` and `while (` in the language carries a
    // declaration reading and an expression reading together, all the way
    // to whatever closes them. Ledgered as a known gap.
    if_statement: ($, previous) => choice(
      previous,
      prec.right(seq(
        'if',
        'constexpr',
        field('condition', $.parenthesized_expression),
        field('consequence', $._statement),
        optional(field('alternative', $.else_clause)),
      )),
    ),

    // A plain braced list. C's version admits `#if` and `#define` between
    // its elements — which real C tables of syscalls and ioctls need — and
    // C++ inherits that at a price it cannot pay: a `{` in C++ is already a
    // compound statement, a lambda body, a braced initialiser and a class
    // body, and a fifth reading of its contents is what three of the
    // remaining declared conflicts were for. Conditionals inside a C++
    // initialiser list are a ledgered known gap.
    initializer_list: $ => seq(
      '{',
      commaSep($._initializer_list_item),
      optional(','),
      '}',
    ),

    // ── expressions ───────────────────────────────────────────────────
    // `template_type` is NOT here, and so `f<int>(x)` — an explicit
    // template argument list in EXPRESSION position — is a ledgered known
    // gap. `std::vector<int> v;` parses, because that is type position;
    // `A<B>::c` parses, through `qualified_identifier`. What is left out is
    // the one form that would put `a < b` and `a<b>` in competition at
    // every comparison in the language.
    _expression: ($, previous) => choice(
      previous,
      $.lambda_expression,
      $.new_expression,
      $.delete_expression,
      $.throw_expression,
      $.co_await_expression,
      $.co_yield_expression,
      $.qualified_identifier,
      $.this,
      $.requires_expression,
    ),

    // `args...`, reachable only from an argument position — which is what
    // C++ says: a pack expansion is legal in a pack-expansion CONTEXT and
    // nowhere else. Admitting it as an ordinary `_expression`, which this
    // rule did first, gave every expression in the language a second
    // possible continuation and the parse table grew accordingly.
    // Written out rather than extended, to leave out C's type argument.
    // C admits a bare `type_descriptor` where an argument goes because
    // `va_arg(ap, char *)` and `offsetof(struct s, m)` need it; in C++ the
    // same position is already a constructor call, a functional cast and a
    // template argument, and a fourth reading of every argument in the
    // language is not affordable. `va_arg` in C++ is a ledgered known gap.
    _argument: $ => choice(
      $._expression,
      $.compound_statement,
      $.parameter_pack_expansion,
    ),

    _literal: ($, previous) => choice(
      previous,
      $.raw_string_literal,
    ),

    this: _ => 'this',

    // `throw` is an expression in C++ and not a `_jump`, for the same
    // structural reason `?:` is not a `_branch` in C: `_jump` nests inside
    // `_control_flow` inside `_statement`, and a `throw` admitted there
    // would make `throw x` parse with no semicolon. The language agrees —
    // C++'s own grammar makes throw-expression an assignment-expression,
    // which is why `f(cond ? a : throw e)` is legal.
    throw_expression: $ => prec.right(PREC.ASSIGNMENT, seq(
      'throw',
      optional(field('value', $._expression)),
    )),

    co_await_expression: $ => prec.right(PREC.UNARY, seq('co_await', $._expression)),
    co_yield_expression: $ => prec.right(PREC.ASSIGNMENT, seq('co_yield', $._expression)),

    new_expression: $ => prec.right(PREC.NEW, seq(
      optional('::'),
      'new',
      field('placement', optional($.argument_list)),
      field('type', $.type_descriptor),
      field('arguments', optional(choice($.argument_list, $.initializer_list))),
    )),

    delete_expression: $ => prec.right(PREC.NEW, seq(
      optional('::'),
      'delete',
      optional(seq('[', ']')),
      field('value', $._expression),
    )),

    // `[&, this](auto x) -> int { … }`. The capture list is what makes a
    // lambda recognisable at its very first character, which is the only
    // reason `[` in expression position is not hopeless.
    lambda_expression: $ => prec(PREC.LAMBDA, seq(
      field('captures', $.lambda_capture_specifier),
      optional(field('template_parameters', $.template_parameter_list)),
      optional(field('constraint', $.requires_clause)),
      optional(field('declarator', $.abstract_function_declarator)),
      field('body', $.compound_statement),
    )),

    lambda_capture_specifier: $ => prec(1, seq(
      '[',
      optional(choice(
        $.lambda_default_capture,
        commaSep1($._lambda_capture),
        seq($.lambda_default_capture, ',', commaSep1($._lambda_capture)),
      )),
      ']',
    )),

    lambda_default_capture: _ => choice('=', '&'),

    // Captures are a closed list — `x`, `&x`, `this`, `*this`, `x = e`,
    // `...args` — and spelling them out rather than admitting any
    // `_expression` is what keeps `[` in expression position tractable.
    _lambda_capture: $ => choice(
      $.lambda_capture,
      $.init_declarator,
      $.parameter_pack_expansion,
    ),

    lambda_capture: $ => seq(
      optional(choice('&', '*')),
      choice($.identifier, $.this),
    ),

    parameter_pack_expansion: $ => prec(-1, seq(
      field('pattern', choice($._expression, $.type_descriptor)),
      '...',
    )),

    // C++17's fold expression -- `(args + ...)` -- and C++11's
    // user-defined literal -- `1_km` -- are both LEFT OUT, and the ledger
    // records them as known gaps rather than as oversights. Each is a third
    // reading of something the table already carries two of (an open paren,
    // a literal), and neither earns the parse-table states it costs in a
    // first-pass grammar for a distribution's C++.

    // C++ adds the three-way comparison and the pointer-to-member
    // operators to C's ladder.
    binary_expression: ($, previous) => choice(
      previous,
      prec.left(PREC.THREE_WAY, seq(
        field('left', $._expression),
        field('operator', '<=>'),
        field('right', $._expression),
      )),
      prec.left(PREC.FIELD, seq(
        field('left', $._expression),
        field('operator', choice('.*', '->*')),
        field('right', $._expression),
      )),
    ),

    // The named casts. They are not `cast_expression`: the whole point of
    // `static_cast<T>(x)` is that it says WHICH conversion, and collapsing
    // the four into one node throws that away.
    cast_expression: ($, previous) => choice(
      previous,
      prec(PREC.CALL, seq(
        field('kind', alias(choice(
          'static_cast', 'dynamic_cast', 'const_cast', 'reinterpret_cast',
        ), $.cast_kind)),
        '<',
        field('type', $.type_descriptor),
        alias(token(prec(1, '>')), '>'),
        '(',
        field('value', $._expression),
        ')',
      )),
    ),

    // `sizeof...(Args)` — the pack size, which is a different operator.
    sizeof_expression: ($, previous) => choice(
      previous,
      prec(PREC.SIZEOF, seq(
        'sizeof', '...', '(', field('value', $.identifier), ')',
      )),
    ),

    // `T{1, 2}` — a temporary built with braces. The parenthesised form
    // `T(1, 2)` is deliberately NOT here: it is indistinguishable from a
    // call of `T` by anything the syntax can see, and `call_expression` is
    // the honest reading of it. The brace form has no such twin.
    compound_literal_expression: ($, previous) => choice(
      previous,
      prec(PREC.CALL, seq(
        field('type', choice($.template_type, $.qualified_identifier)),
        field('value', $.initializer_list),
      )),
    ),

  },
});

/** @param {RuleOrLiteral} rule */
function commaSep(rule) {
  return optional(commaSep1(rule));
}

/** @param {RuleOrLiteral} rule */
function commaSep1(rule) {
  return seq(rule, repeat(seq(',', rule)));
}
