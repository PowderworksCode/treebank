/**
 * treebank-c: a from-scratch grammar for C, carrying the treebank
 * vocabulary (DESIGN.md §3) in its parse table.
 *
 * Three facts about C decide the shape of this file.
 *
 * **The preprocessor is part of the syntax.** Not a pass that runs first —
 * real C puts `#ifdef` between two enumerators, inside an initializer list
 * and around half a function body, and a grammar that cannot hold a
 * directive in those positions fails the file rather than the directive.
 * So conditionals are generated per context by `preprocIf` below, once for
 * every list they may interrupt.
 *
 * **A declarator is a tree, not a name.** `int *(*f[3])(void)` reads
 * outward-in, and the same recursion appears with a name (declarations),
 * without one (casts, `sizeof`, parameters) and with a bit-field width
 * (struct members). Two hierarchies carry it here — `_declarator` ending in
 * an identifier, `_abstract_declarator` ending in nothing — where a grammar
 * that also wanted `field_identifier` and `type_identifier` node types
 * would need four. That choice is a naming one and it is recorded as a
 * deviation in ledger.toml, not left to be discovered.
 *
 * **The dialect is GNU C, because the corpus is.** `__attribute__`,
 * statement expressions, `typeof`, case ranges, `__asm__` and K&R
 * definitions are not extensions to be tolerated: they are what a
 * distribution's C is written in, and every one of them is in the table.
 *
 * `_pattern` and `_interpolation` are not threaded — C has neither.
 * `_parameter` is demoted to the facet tier; see roles.json for why.
 */

/// <reference types="tree-sitter-cli/dsl" />
// @ts-check

const tb = require('../treebank-core/vocabulary/supertypes.js');

// C's own precedence ladder, from the standard's grammar. Nothing here is
// invented: the numbers are the order of the productions in C11 6.5.
const PREC = {
  PAREN_DECLARATOR: -10,
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
  OFFSETOF: 8,
  SHIFT: 9,
  ADD: 10,
  MULTIPLY: 11,
  CAST: 12,
  SIZEOF: 13,
  UNARY: 14,
  CALL: 15,
  FIELD: 16,
  SUBSCRIPT: 17,
};

module.exports = grammar({
  name: 'c',

  // Keyword extraction: without it every keyword is a separate token in the
  // lexer's start state and `internal` (not a keyword) lexes as `int` plus
  // `ernal` in the states where `int` is valid.
  word: $ => $.identifier,

  // A backslash-newline is whitespace anywhere, not only in a directive:
  // GNU code line-continues inside ordinary declarations and string
  // concatenations too.
  extras: $ => [
    /[\s]|\\\r?\n/,
    $.comment,
  ],

  supertypes: $ => tb.assertTableTerms([
    '_statement',
    '_expression',
    '_declaration',
    '_type',
    '_name',
    '_literal',
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
    // `_parameter` is demoted to the facet tier here. A C parameter list is
    // ordered by the grammar itself — `...` is only ever last, and `void`
    // alone is only ever the whole list — so one alternation repeated by
    // commas necessarily accepts `f(..., int x)`. Both members are concrete
    // node types occurring nowhere else, so the facet selects exactly the
    // nodes the supertype would have (DESIGN.md §3.1.1). See roles.json.
    ...tb.assertDemotable([]),
  ]).map((name) => $[name]),

  conflicts: $ => [
    [$._declaration_specifiers, $.parenthesized_declarator, $.macro_attributed_declarator],
    [$.macro_modifier, $._type, $.macro_type_specifier, $._declarator],
    [$.macro_attributed_declarator],
    [$.pointer_declarator, $.macro_attributed_declarator],
    [$.parenthesized_declarator, $.macro_attributed_declarator],
    [$.type_definition, $.macro_attributed_declarator],
    [$.pointer_declarator, $.abstract_pointer_declarator],
    [$.macro_modifier, $._declarator],
    [$.macro_modifier, $._type, $._declarator],
    [$.macro_modifier, $._sizeable_type],
    [$.macro_modifier, $._type],
    [$.unexpanded_macro, $.macro_modifier, $._sizeable_type],
    [$.unexpanded_macro, $.macro_modifier],
    [$.unexpanded_macro, $.macro_modifier, $._type],
    [$.macro_type_specifier, $._type_argument],
    [$.unexpanded_macro, $._type, $.macro_type_specifier],
    [$.unexpanded_macro, $._type],
    [$.case_clause],
    [$.default_clause],
    [$._literal, $.concatenated_string],
    [$.attributed_statement],
    [$._declaration_modifiers, $.attributed_statement],
    [$._type, $.identifier_list],
    [$.union_specifier],
    [$.struct_specifier],
    [$.enum_specifier],
    [$._expression, $._assignment_target],
    [$._declaration_modifiers, $.attributed_declarator],
    // `T (x)` is a parenthesised declarator of `x` or a call of `T`, and at
    // the open paren nothing has decided which. Both readings are complete
    // and the negative PAREN_DECLARATOR precedence picks the call unless a
    // declaration context is already committed.
    [$._type, $._expression],
    // `f(a)` in a parameter list: a K&R identifier list or one unnamed
    // parameter whose type is the typedef `a`. Only the body that follows
    // settles it.
    [$._declarator, $._type],
    [$.sized_type_specifier],
    [$._type, $._expression, $.macro_type_specifier],
    [$._type, $.macro_type_specifier],
    // `int f(a, b) T a; { … }` against `int f(void) __THROW;`: after the
    // parameter list an identifier is either the first K&R parameter
    // declaration or a macro sitting on the declarator, and nothing before
    // it has decided which. A declared conflict rather than a precedence
    // because both readings are real and only what follows picks one; the
    // negative dynamic precedence on `macro_attribute` gives K&R the tie.
    [$._declarator, $.macro_attributed_declarator],
  ],

  inline: $ => [
    $._top_level_item,
    $._block_item,
  ],

  rules: {
    // ── the compilation unit ─────────────────────────────────────────
    translation_unit: $ => repeat($._top_level_item),

    // Top level and block level are deliberately different alternations. A
    // bare `f();` at file scope is not C, and the negative corpus has a file
    // that says so — the cost of merging them would be a grammar that
    // accepts a whole class of not-C.
    _top_level_item: $ => choice(
      $._declaration,
      $._directive,
      $.empty_declaration,
      // `__asm__(".section .init");` at file scope is real, and common in
      // libc and in the kernel headers a distribution ships.
      $.asm_statement,
      $.unexpanded_macro,
    ),

    _block_item: $ => choice(
      $._statement,
      $._directive,
    ),

    empty_declaration: _ => ';',

    // `__BEGIN_DECLS`, `_XFUNCPROTOBEGIN`, `G_BEGIN_DECLS`,
    // `BEGIN_EXTERN_C()` — a macro standing at file scope with no semicolon
    // of its own, because what it expands to already carries one or is
    // empty. It is not valid C and clang says so; it is what unpreprocessed
    // C LOOKS like, which is the only thing a grammar gets to see.
    //
    // This is the grammar's one deliberate over-acceptance and the ledger
    // declares it: `f();` at file scope now parses too, as this rule plus
    // an `empty_declaration`, and the compiler rejects that. It is admitted
    // because the alternative is losing every header that opens with one —
    // measured at 467 of 1,367 local failures, a third of them.
    //
    // The negative dynamic precedence keeps it last: anything readable as a
    // declaration stays a declaration.
    unexpanded_macro: $ => prec.dynamic(-1, seq(
      field('name', $.identifier),
      optional(field('arguments', $.argument_list)),
    )),

    // ── the preprocessor ─────────────────────────────────────────────
    // Directives are `_directive` in the vocabulary's sense exactly: they
    // affect the compilation unit rather than compute in it.
    _directive: $ => choice(
      $.preproc_include,
      $.preproc_def,
      $.preproc_function_def,
      $.preproc_undef,
      $.preproc_if,
      $.preproc_ifdef,
      $.preproc_call,
    ),

    preproc_include: $ => seq(
      preprocessor('include'),
      field('path', choice(
        $.string_literal,
        $.system_lib_string,
        // `#include MACRO` and `#include MACRO(x)`: the path is computed.
        $.identifier,
        alias($.preproc_call_expression, $.call_expression),
      )),
      '\n',
    ),

    preproc_def: $ => seq(
      preprocessor('define'),
      field('name', $.identifier),
      field('value', optional($.preproc_arg)),
      '\n',
    ),

    preproc_function_def: $ => seq(
      preprocessor('define'),
      field('name', $.identifier),
      field('parameters', $.preproc_params),
      field('value', optional($.preproc_arg)),
      '\n',
    ),

    // `(` must be IMMEDIATE. `#define A (x)` defines A as the parenthesised
    // expression `(x)`; `#define A(x)` defines a function-like macro. One
    // space is the whole difference, and it is the preprocessor's own rule.
    preproc_params: $ => seq(
      token.immediate('('),
      commaSep(choice($.identifier, $.variadic_parameter, alias($.gnu_variadic_parameter, $.variadic_parameter))),
      ')',
    ),

    // `#define log(fmt, args...)` — the GNU spelling of a variadic macro,
    // where the trailing parameter names what ISO C leaves as __VA_ARGS__.
    gnu_variadic_parameter: $ => seq($.identifier, token.immediate('...')),

    preproc_undef: $ => seq(
      preprocessor('undef'),
      field('name', $.identifier),
      '\n',
    ),

    // `#pragma`, `#error`, `#warning`, `#line`, `#ident`, `#assert`, and
    // whatever a compiler adds next. Taking the tail as one argument token
    // is the honest reading: the content is not C and its grammar belongs
    // to whoever consumes the directive.
    preproc_call: $ => seq(
      field('directive', $.preproc_directive),
      field('argument', optional($.preproc_arg)),
      '\n',
    ),

    ...preprocIf('', $ => $._top_level_item),
    ...preprocIf('_in_block', $ => $._block_item),
    ...preprocIf('_in_field_declaration_list', $ => $._member),
    ...preprocIf('_in_enumerator_list', $ => seq($.enumerator, ',')),
    ...preprocIf('_in_enumerator_list_no_comma', $ => $.enumerator, -1),
    ...preprocIf('_in_initializer_list', $ => seq($._initializer_list_item, ',')),
    ...preprocIf('_in_initializer_list_no_comma', $ => $._initializer_list_item, -1),

    // The condition of an `#if`. It is not an `_expression` of the language
    // and is not threaded as one: `defined X` is not C, the arithmetic is
    // done on preprocessor tokens, and an identifier here is a macro name
    // that stands for 0 when undefined. The shapes it shares with C — a
    // binary operator, a call — are aliased to the ordinary node names so a
    // consumer sees `binary_expression` and not a parallel vocabulary.
    _preproc_expression: $ => choice(
      $.identifier,
      alias($.preproc_call_expression, $.call_expression),
      $.number_literal,
      $.char_literal,
      $.string_literal,
      $.preproc_defined,
      $.preproc_has_include,
      alias($.preproc_unary_expression, $.unary_expression),
      alias($.preproc_binary_expression, $.binary_expression),
      alias($.preproc_conditional_expression, $.conditional_expression),
      alias($.preproc_parenthesized_expression, $.parenthesized_expression),
    ),

    // `#if defined __cplusplus ? __GNUC_PREREQ (2, 6) : __GNUC_PREREQ (2, 4)`
    // — glibc's `sys/cdefs.h` asks a different question of the compiler
    // depending on the language, and the preprocessor's grammar has `?:`
    // exactly as C's does.
    preproc_conditional_expression: $ => prec.right(PREC.CONDITIONAL, seq(
      field('condition', $._preproc_expression),
      '?',
      field('consequence', $._preproc_expression),
      ':',
      field('alternative', $._preproc_expression),
    )),

    preproc_parenthesized_expression: $ => seq('(', $._preproc_expression, ')'),

    preproc_call_expression: $ => prec(PREC.CALL, seq(
      field('function', $.identifier),
      field('arguments', alias($.preproc_argument_list, $.argument_list)),
    )),

    preproc_argument_list: $ => seq('(', commaSep($._preproc_expression), ')'),

    preproc_unary_expression: $ => prec.left(PREC.UNARY, seq(
      field('operator', choice('!', '~', '-', '+')),
      field('argument', $._preproc_expression),
    )),

    preproc_binary_expression: $ => choice(...binaryOperators().map(([precedence, operator]) =>
      prec.left(precedence, seq(
        field('left', $._preproc_expression),
        field('operator', operator),
        field('right', $._preproc_expression),
      )),
    )),

    // `defined X` and `defined(X)`. Its operand is a macro NAME, not an
    // expression: an undefined macro is legal here and nowhere else.
    preproc_defined: $ => choice(
      prec(PREC.CALL, seq('defined', '(', $.identifier, ')')),
      seq('defined', $.identifier),
    ),

    // `#if __has_include(<stdckdint.h>)`. Like `defined` the operand is not
    // an expression — the `<...>` form is a header name, which inside an
    // `#if` would otherwise lex as a less-than operator and never close.
    preproc_has_include: $ => prec(PREC.CALL, seq(
      choice('__has_include', '__has_include_next'),
      '(',
      choice($.string_literal, $.system_lib_string, $.identifier),
      ')',
    )),

    // A directive name this grammar has no rule for. The leading `#` may be
    // followed by spaces — `#  pragma once` is legal and appears in real
    // headers — and the null directive (`#` alone on a line) is legal too.
    preproc_directive: _ => token(seq('#', /[ \t]*/, /[a-zA-Z_][a-zA-Z0-9_]*/)),

    // Everything from here to the end of the logical line. Comments have to
    // be recognised inside it or a `/* */` carrying a stray backslash-newline
    // would end the argument in the middle of itself.
    preproc_arg: _ => token(prec(-1, repeat1(choice(
      /[^/\\\r\n]/,
      /\\\r?\n/,
      /\\./,
      /\/[^*/\r\n]/,
      seq('/*', repeat(choice(/[^*]/, /\*[^/]/)), '*/'),
    )))),

    // `<sys/types.h>` — one token, because inside a `#include` the angle
    // brackets are not operators.
    system_lib_string: _ => token(seq('<', repeat(choice(/[^>\n]/, '\\>')), '>')),

    // ── declarations ─────────────────────────────────────────────────
    _declaration: $ => choice(
      $.declaration,
      $.function_definition,
      $.type_definition,
      $.static_assert_declaration,
    ),

    // The declarator list is empty-able, and that is the compiler's rule
    // rather than a relaxation: `struct node { … };` declares a tag and
    // nothing else, and `int;` — measured against clang, which warns and
    // does not error — is a declaration that declares nothing. `B32;`, a
    // macro standing alone with its semicolon, lands here for the same
    // reason and needs no rule of its own.
    declaration: $ => seq(
      $._declaration_specifiers,
      commaSep(field('declarator', $._declarator_with_init)),
      ';',
    ),

    _declarator_with_init: $ => choice($._declarator, $.init_declarator),

    init_declarator: $ => seq(
      field('declarator', $._declarator),
      '=',
      field('value', choice($.initializer_list, $._expression)),
    ),

    // The macro between the specifiers and the name is the alignment and
    // deprecation marker a typedef carries: `typedef int __ONCE_ALIGNMENT
    // pthread_once_t;`, `typedef struct { … } __ATM_API_ALIGN atm_kptr_t;`,
    // `} __ARCH_SI_ATTRIBUTES siginfo_t;`. It is admitted here and not in
    // `_declaration_specifiers` because that repetition is shared with
    // `declaration`, where a macro after the type is precisely the rule that
    // makes `int x y;` parse. `typedef` has already committed the parser,
    // so nothing else can be reached through it.
    //
    // Which of two adjacent identifiers is the macro is not decidable
    // here, and the ledger says so: glib writes the marker AFTER the name
    // (`typedef struct _GTrashStack GTrashStack GLIB_DEPRECATED_TYPE_IN_2_48;`)
    // and the file parses, with the `declarator` field naming the macro and
    // the macro naming the type. One of the two spellings has to lose, and
    // the one that keeps its tree is the one whose macro is a type
    // attribute rather than a deprecation notice.
    type_definition: $ => seq(
      optional($.extension_specifier),
      'typedef',
      $._declaration_specifiers,
      repeat($.macro_modifier),
      // Empty-able for the same reason `declaration`'s list is: a typedef
      // written as one macro call — `typedef PNG_CALLBACK(void,
      // *png_error_ptr, (png_structp, png_const_charp));` — has its name
      // inside the macro's arguments, so the specifier is the whole of it.
      commaSep(field('declarator', $._declarator)),
      ';',
    ),

    // C11 6.7.10. The message is optional from C23 and from every compiler
    // long before it.
    static_assert_declaration: $ => seq(
      choice('_Static_assert', 'static_assert'),
      '(',
      field('condition', $._expression),
      optional(seq(',', field('message', choice($.string_literal, $.concatenated_string)))),
      ')',
      ';',
    ),

    // The declaration specifier soup: qualifiers, storage classes and
    // attributes on either side of exactly one type. C really does allow
    // `const static unsigned int` in any order, so one unordered repetition
    // is the correct rule and not a shortcut — which is why `_modifier`
    // stays in the TABLE tier here where rust had to demote it.
    _declaration_specifiers: $ => seq(
      repeat(choice($._declaration_modifiers, $.macro_modifier)),
      field('type', $._type),
      repeat($._declaration_modifiers),
    ),

    // `ZEND_API ZEND_ATTRIBUTE_MALLOC char *ZEND_FASTCALL zend_strndup (…)`,
    // `static zend_always_inline zend_result f (…)`,
    // `OSSL_DEPRECATEDIN_3_0 int ERR_load_ASN1_strings (void);` — a macro
    // standing in the specifier soup beside the real type, which will expand
    // to a storage class, a visibility attribute, a calling convention, or to
    // nothing at all.
    //
    // BEFORE THE TYPE ONLY, and that restriction is the whole safety
    // argument. `int x y;` cannot reach this rule: `int` is the type, and a
    // macro after the type is not admitted. What it does admit is
    // `foo bar baz;` — three identifiers, where the first is read as a macro,
    // the second as the type and the third as the name. The ledger declares
    // that.
    //
    // No argument list, unlike `macro_attribute` after the declarator. That
    // is not a claim that such macros never take arguments; it is that at
    // `identifier •  (` three rules already compete — `macro_type_specifier`,
    // `unexpanded_macro` and a call — and a fourth reading of the hottest
    // state in the grammar costs more than the handful of files it buys.
    // Dynamic precedence BELOW `unexpanded_macro`, not merely below a real
    // declaration. At file scope `__BEGIN_DECLS int f(void);` can be read
    // either way — a macro standing alone, or a modifier on the declaration
    // that follows — and both readings parse the whole file, so the tie is
    // decided here rather than by chance. It goes to the standalone reading,
    // which is the one the corpus test asserts and the one `__BEGIN_DECLS`
    // actually is. Where a declaration has already started, `static
    // zend_always_inline zend_result f(…)`, the standalone reading is not
    // available and this is the only one left, which is where the rule earns
    // its keep.
    macro_modifier: $ => prec.dynamic(-2, field('name', $.identifier)),

    _declaration_modifiers: $ => choice(
      $._modifier,
      $._attribute,
      $.extension_specifier,
    ),

    _modifier: $ => choice(
      $.storage_class_specifier,
      $.type_qualifier,
      $.alignas_qualifier,
    ),

    storage_class_specifier: _ => choice(
      'extern', 'static', 'auto', 'register',
      'inline', '__inline', '__inline__', '__forceinline',
      '_Noreturn', 'thread_local', '_Thread_local', '__thread',
      'constexpr',
    ),

    type_qualifier: _ => choice(
      'const', '__const', '__const__',
      'volatile', '__volatile', '__volatile__',
      'restrict', '__restrict__', '__restrict',
      '_Atomic',
      // Clang's nullability qualifiers, which Apple's headers use and which
      // reach a distribution through vendored copies of them.
      '_Nonnull', '_Nullable', '_Null_unspecified',
    ),

    alignas_qualifier: $ => seq(
      choice('alignas', '_Alignas'),
      '(',
      choice($._expression, $.type_descriptor),
      ')',
    ),

    // `__extension__` silences a pedantic diagnostic. It is not a modifier
    // of the declaration's meaning and not an annotation on it, so it is
    // neither `_modifier` nor `_attribute`; the ledger records it.
    extension_specifier: _ => '__extension__',

    _attribute: $ => choice(
      $.attribute_specifier,
      $.attribute_declaration,
      $.ms_declspec_modifier,
    ),

    // `__attribute__((packed, aligned(4)))`. The contents are taken as
    // expressions because that is what they are — `aligned(4)` is a call in
    // every respect the syntax can see.
    attribute_specifier: $ => seq(
      choice('__attribute__', '__attribute'),
      '(',
      '(',
      commaSep(optional($.attribute)),
      ')',
      ')',
    ),

    attribute: $ => seq(
      optional(seq(field('prefix', $.identifier), '::')),
      field('name', $.identifier),
      optional(field('arguments', $.argument_list)),
    ),

    // C23's `[[nodiscard]]`, and the GNU/C++ spelling long before it.
    attribute_declaration: $ => seq('[[', commaSep1($.attribute), ']]'),

    // `__declspec(dllexport)` and `__declspec(align(8))`: the contents are
    // a name or a call of one, so they are taken as an expression for the
    // reason `attribute_specifier` gives — `align(8)` is a call in every
    // respect the syntax can see.
    ms_declspec_modifier: $ => choice(
      seq('__declspec', '(', choice($.identifier, $.call_expression), ')'),
      // The single-parenthesis `__attribute__` spelling keeps its bare
      // name: the doubled form beside it is `attribute_specifier`, whose
      // contents are already expressions, and offering a call here as well
      // gives `__attribute__((f(1)))` two readings.
      seq('__attribute__', '(', $.identifier, ')'),
    ),

    // ── types ────────────────────────────────────────────────────────
    // `_type` is threaded over the type SPECIFIERS. `type_descriptor` — a
    // specifier plus an abstract declarator, which is what a cast and a
    // `sizeof` take — is deliberately not a member: it may only appear in
    // an expression position, and one alternation cannot say that. It is
    // recorded as uncategorised in the ledger with that reason.
    _type: $ => choice(
      $.primitive_type,
      $.sized_type_specifier,
      $.struct_specifier,
      $.union_specifier,
      $.enum_specifier,
      $.typeof_specifier,
      $.atomic_type_specifier,
      $.macro_type_specifier,
      alias($.identifier, $.type_identifier),
    ),

    primitive_type: _ => token(choice(
      'void', 'char', 'int', 'float', 'double',
      'bool', '_Bool',
      // GCC's `__auto_type` is a type specifier that takes no operand: it
      // is `typeof` the initializer, and it belongs here rather than with
      // `typeof` because it has no parenthesised argument at all.
      '__auto_type',
      // <stdint.h> and friends are typedefs, not keywords, and are NOT
      // listed here: a grammar that hard-codes `uint32_t` cannot see the
      // package that typedefs its own.
    )),

    // `unsigned`, `long long`, `signed char`, `_Complex double`. The size
    // and sign words attach to an optional base type rather than replacing
    // it, which is what lets `long double _Complex` parse: `_Complex` and
    // `_Imaginary` (C99 6.7.2) appear BESIDE float and double, never
    // instead of them.
    sized_type_specifier: $ => choice(
      seq(
        repeat1($._size_or_sign),
        optional(field('type', $._sizeable_type)),
        repeat($._size_or_sign),
      ),
      // `double _Complex` — the base type first and the modifier after it,
      // which is the spelling C99 6.7.2 gives and glibc's `math.h` uses.
      seq(field('type', $._sizeable_type), repeat1($._size_or_sign)),
    ),

    // GCC's alternate spellings are here rather than in a tolerance list:
    // `typedef __signed__ char __s8;` is how every kernel UAPI header
    // spells it, and the double-underscore forms exist precisely so a
    // header can use the keyword in a translation unit that has `#define
    // signed` in it.
    _size_or_sign: _ => choice(
      'signed', '__signed', '__signed__',
      'unsigned',
      'long', 'short',
      '_Complex', '__complex__', '_Imaginary',
    ),

    _sizeable_type: $ => choice(
      $.primitive_type,
      alias($.identifier, $.type_identifier),
    ),

    // C23's `typeof`, spelled `__typeof` and `__typeof__` by GCC for the
    // three decades before that. The operand is an expression or a type,
    // and the expression case is the one that matters: `typeof(x->field)`
    // is how a kernel header writes a generic macro.
    typeof_specifier: $ => seq(
      choice('typeof', '__typeof', '__typeof__', 'typeof_unqual'),
      '(',
      field('value', choice($.type_descriptor, $._expression)),
      ')',
    ),

    // C11 6.7.2.4 gives `_Atomic` two jobs: the qualifier `_Atomic int x;`,
    // which `type_qualifier` has, and the type specifier `_Atomic(int) x;`,
    // which is this.
    atomic_type_specifier: $ => seq(
      '_Atomic',
      '(',
      field('type', $.type_descriptor),
      ')',
    ),

    // A macro used where a type goes: `GLIBC_TYPE(int) x;`. Not a type
    // constructor of the language, but the only reading available without
    // the macro's definition.
    //
    // The arguments after the first are the kernel UAPI headers':
    // `__DECLARE_FLEX_ARRAY(struct in6_addr, addr);` is a whole member
    // written as one macro call, and the tokens the parser is handed are a
    // type and a name. The first argument stays a `type_descriptor` and
    // keeps the `type` field, because that is what makes this a type
    // specifier at all; the rest are ordinary `_argument`s, which already
    // admit a type or an expression.
    macro_type_specifier: $ => prec.dynamic(-1, seq(
      field('name', $.identifier),
      '(',
      field('type', $.type_descriptor),
      repeat(seq(',', $._argument)),
      ')',
    )),

    struct_specifier: $ => seq(
      'struct',
      repeat($._attribute),
      choice(
        seq(
          field('name', alias($.identifier, $.type_identifier)),
          optional(field('body', $.field_declaration_list)),
        ),
        field('body', $.field_declaration_list),
      ),
    ),

    union_specifier: $ => seq(
      'union',
      repeat($._attribute),
      choice(
        seq(
          field('name', alias($.identifier, $.type_identifier)),
          optional(field('body', $.field_declaration_list)),
        ),
        field('body', $.field_declaration_list),
      ),
    ),

    enum_specifier: $ => seq(
      'enum',
      repeat($._attribute),
      choice(
        seq(
          field('name', alias($.identifier, $.type_identifier)),
          // C23 lets an enum name its underlying type.
          optional(seq(':', field('underlying_type', $._type))),
          optional(field('body', $.enumerator_list)),
        ),
        seq(
          optional(seq(':', field('underlying_type', $._type))),
          field('body', $.enumerator_list),
        ),
      ),
    ),

    // A type plus an abstract declarator, in the positions where a type is
    // an operand: a cast, `sizeof`, `_Alignof`, `offsetof`, a compound
    // literal, a `_Generic` association.
    type_descriptor: $ => seq(
      repeat($._type_qualifier_or_attribute),
      field('type', $._type),
      repeat($._type_qualifier_or_attribute),
      field('declarator', optional($._abstract_declarator)),
    ),

    _type_qualifier_or_attribute: $ => choice($.type_qualifier, $._attribute),

    field_declaration_list: $ => seq(
      '{',
      repeat($._member),
      '}',
    ),

    _member: $ => choice(
      $.field_declaration,
      $.static_assert_declaration,
      $._directive_in_field_declaration_list,
    ),

    _directive_in_field_declaration_list: $ => choice(
      $.preproc_include,
      $.preproc_def,
      $.preproc_function_def,
      $.preproc_undef,
      $.preproc_call,
      alias($.preproc_if_in_field_declaration_list, $.preproc_if),
      alias($.preproc_ifdef_in_field_declaration_list, $.preproc_ifdef),
    ),

    // A struct member is a declaration with two extra shapes: a bit-field
    // width, and NO declarator at all.
    //
    // Both are the standard's, not a tolerance. C11 6.7.2.1 writes a
    // struct-declarator as `declarator` or `declarator_opt :
    // constant-expression`, so `unsigned : 3;` is padding and `int : 0;`
    // forces alignment to the next storage unit. The declarator-less form
    // without a colon is the anonymous struct/union member — `struct { int
    // x; };` inside another struct — which C11 added and which every
    // tagged-union in a distribution uses.
    field_declaration: $ => seq(
      $._declaration_specifiers,
      commaSep($._field_declarator),
      ';',
    ),

    // One element of a member's declarator list. The bare `bitfield_clause`
    // alternative is not the same as the optional one beside it:
    // `int y : 2, : 4, z;` puts an unnamed bit-field in the MIDDLE of a
    // list, so the width has to be an element in its own right and not a
    // suffix on the declaration.
    _field_declarator: $ => choice(
      seq(field('declarator', $._declarator), optional($.bitfield_clause)),
      $.bitfield_clause,
    ),

    bitfield_clause: $ => seq(':', field('width', $._expression)),

    // An enumerator list carries directives between its enumerators, and
    // `#define` is the one that matters: the kernel's UAPI headers put one
    // after every enumerator so each name is testable with `#ifdef`. A
    // directive is not followed by a comma and may also follow the last
    // enumerator, so it needs a slot in the repeat AND in the tail.
    enumerator_list: $ => seq(
      '{',
      repeat(choice(
        seq($.enumerator, ','),
        $._directive_in_enumerator_list,
      )),
      // The tail holds the LAST enumerator, which may drop its comma — and
      // a conditional whose own last enumerator drops it, which is why the
      // `_no_comma` instantiation exists. Plain directives are not offered
      // here: they are already in the repeat above, and offering them twice
      // is an ambiguity with no reading to distinguish.
      // The tail may be followed by more directives — the UAPI idiom puts a
      // `#define` after the LAST enumerator too — and they hang off the
      // tail rather than sitting in their own repeat, because a repeat here
      // and the one above would both match `{ #define A }` with nothing to
      // choose between them.
      optional(seq(
        choice($.enumerator, $._conditional_in_enumerator_list_no_comma),
        repeat($._directive_in_enumerator_list),
      )),
      '}',
    ),

    _directive_in_enumerator_list: $ => choice(
      $.preproc_def,
      $.preproc_function_def,
      $.preproc_undef,
      $.preproc_call,
      alias($.preproc_if_in_enumerator_list, $.preproc_if),
      alias($.preproc_ifdef_in_enumerator_list, $.preproc_ifdef),
    ),

    _conditional_in_enumerator_list_no_comma: $ => choice(
      alias($.preproc_if_in_enumerator_list_no_comma, $.preproc_if),
      alias($.preproc_ifdef_in_enumerator_list_no_comma, $.preproc_ifdef),
    ),

    // The macro after the name is glib's availability marker —
    // `G_URI_FLAGS_SCHEME_NORMALIZE GLIB_AVAILABLE_ENUMERATOR_IN_2_68 = 1 << 8,`
    // — which expands to an attribute or to nothing. It sits exactly where
    // `__attribute__((deprecated))` sits on the line above it.
    // Right-associative because the run of markers is greedy: at a second
    // identifier the enumerator takes it rather than ending.
    enumerator: $ => prec.right(seq(
      field('name', $.identifier),
      repeat(choice($._attribute, $.macro_modifier)),
      optional(seq('=', field('value', $._expression))),
    )),

    // ── declarators ──────────────────────────────────────────────────
    // The concrete hierarchy: it bottoms out in a name.
    _declarator: $ => choice(
      $.attributed_declarator,
      $.macro_attributed_declarator,
      $.pointer_declarator,
      $.function_declarator,
      $.array_declarator,
      $.parenthesized_declarator,
      $.identifier,
    ),

    // And the abstract one, which bottoms out in nothing at all — the
    // `char *` in `va_arg(ap, char *)`, the `int (*)(void)` in a cast.
    _abstract_declarator: $ => choice(
      $.abstract_pointer_declarator,
      $.abstract_function_declarator,
      $.abstract_array_declarator,
      $.abstract_parenthesized_declarator,
    ),

    // Negative precedence: `T (x)` is a call of `T` far more often than it
    // is a parenthesised declarator of `x`, and only a context that has
    // already committed to a declaration should read it the other way.
    // The leading macro is the calling convention in front of the star, the
    // spelling every Windows-facing header uses and ncurses with it:
    // `void (NCURSES_API *_nc_check_termtype)(TERMTYPE *)`,
    // `BOOL (WINAPI *PGetFileInformationByName)(…)`.
    parenthesized_declarator: $ => prec.dynamic(PREC.PAREN_DECLARATOR, seq(
      '(',
      choice(
        $._declarator,
        // The macro must be followed by the star, which is what keeps this
        // out of `(f(x))`: at the open paren the macro reading is only alive
        // while a pointer declarator can still follow it.
        seq(repeat1($.macro_modifier), $.pointer_declarator),
      ),
      ')',
    )),

    abstract_parenthesized_declarator: $ => prec(1, seq(
      '(',
      $._abstract_declarator,
      ')',
    )),

    // The attribute after the `*` is GCC's: `int * __attribute__((nonnull))
    // f;` puts it on the pointer, not on the declaration.
    // The macro alongside the qualifiers is the calling convention and the
    // pointer-attribute macro: `zend_ast * ZEND_FASTCALL f (…)`,
    // `png_struct * PNG_RESTRICT png_structrp`. It sits exactly where
    // `const` and `__restrict` sit, expands to one of them or to an
    // attribute, and is admitted here only — after a `*` that has already
    // committed the parser to a declarator.
    pointer_declarator: $ => prec.dynamic(1, prec.right(seq(
      '*',
      repeat(choice($._type_qualifier_or_attribute, $.macro_modifier)),
      field('declarator', $._declarator),
    ))),

    abstract_pointer_declarator: $ => prec.dynamic(1, prec.right(seq(
      '*',
      repeat($._type_qualifier_or_attribute),
      field('declarator', optional($._abstract_declarator)),
    ))),

    function_declarator: $ => prec.right(1, seq(
      field('declarator', $._declarator),
      field('parameters', choice($.parameter_list, $.identifier_list)),
      repeat($._attribute),
    )),

    abstract_function_declarator: $ => prec.right(1, seq(
      field('declarator', optional($._abstract_declarator)),
      field('parameters', $.parameter_list),
      repeat($._attribute),
    )),

    array_declarator: $ => prec(1, seq(
      field('declarator', $._declarator),
      '[',
      repeat($._modifier),
      field('size', optional(choice($._expression, '*'))),
      ']',
    )),

    abstract_array_declarator: $ => prec(1, seq(
      field('declarator', optional($._abstract_declarator)),
      '[',
      repeat($._modifier),
      field('size', optional(choice($._expression, '*'))),
      ']',
    )),

    // Three positions GCC accepts an attribute in that the standard does
    // not, all of them common: after the declarator of an object
    // declaration (`int a __attribute__((unused));`), on a typedef, and on
    // a member.
    attributed_declarator: $ => prec.right(seq(
      $._declarator,
      repeat1(choice($._attribute, $.asm_label)),
    )),

    // `extern void aio_init (const struct aioinit *__init) __THROW __nonnull ((1));`
    // — the macro after the declarator, which is the single largest thing a
    // grammar meets in unpreprocessed C. `__THROW` sits on very nearly every
    // function glibc declares; `__ul_attribute__((warn_unused_result))`,
    // `_X_NONSTRING` and `__attribute_nonstring__` do the same in util-linux,
    // X11 and the kernel UAPI headers. Every one of them expands to an
    // attribute or to nothing, and none of them is in the file being parsed.
    //
    // The ledger used to reject this rule, and the reason it gave was right
    // about the rule it was rejecting: a bare identifier after ANY declarator
    // makes `int x y;` parse. This one is not that. The declarator it may
    // follow has to END IN `)` OR `]` — a function or array declarator — so
    // a name followed by a name is still not a declaration,
    // and `two-expressions-juxtaposed.c` still fails. A pointer reaches it
    // through nesting, the way an attribute does: `*alloca (size_t) __THROW`
    // is a pointer around this, not this around a pointer.
    //
    // What it does admit that C does not is `int f(void) g;` and
    // `int a[3] g;`, and the ledger declares them. That is the whole cost,
    // and it is the same shape and the same size as the over-acceptance
    // `unexpanded_macro` already carries at file scope.
    macro_attributed_declarator: $ => prec.right(choice(
      seq(
        field('declarator', choice(
          $.function_declarator,
          $.array_declarator,
          // `int (*close) __P((struct __db *));` — the K&R portability
          // macro on a parenthesised declarator, which ends in `)` like
          // the two above it.
          $.parenthesized_declarator,
          // And an already-attributed one, which is how glibc interleaves
          // them: `int abs (int) __THROW __attribute__ ((__const__)) __wur;`
          // is this rule around an `attributed_declarator` around this rule.
          $.attributed_declarator,
        )),
        repeat1(field('attribute', $.macro_attribute)),
      ),
      // And the prefix form, which is the calling convention between the
      // type and the name: `ZEND_API void ZEND_FASTCALL zend_ast_ref_destroy
      // (zend_ast_ref *ast);`. The macro is after the type, which is the one
      // place `_declaration_specifiers` must not admit it — that repetition
      // is shared with every declaration, and a macro there is exactly the
      // rule that makes `int x y;` parse.
      //
      // Here it cannot, because what follows the macro has to be a FUNCTION
      // declarator. `int x y;` needs `y` to carry a parameter list and it
      // does not. `int x y();` does parse, and that is the whole cost.
      seq(
        repeat1(field('modifier', $.macro_modifier)),
        field('declarator', $.function_declarator),
        repeat(field('attribute', $.macro_attribute)),
      ),
    )),

    // The macro itself, with the arguments it may carry: `__nonnull ((1))`
    // and `__ul_attribute__((warn_unused_result))` are one identifier and one
    // argument list, where the doubled parentheses are the outer list holding
    // a parenthesised expression.
    //
    // A node of its own rather than an `_attribute`, for the reason
    // `asm_label` gives below: `_attribute` is admitted at the start of a
    // statement, and a bare identifier there would give every expression
    // statement a second reading. The negative dynamic precedence keeps it
    // last wherever something real also fits.
    // Right-associative because the argument list is greedy: at
    // `__nonnull (` the parenthesis belongs to the macro rather than
    // starting the next thing.
    macro_attribute: $ => prec.dynamic(-1, prec.right(seq(
      field('name', $.identifier),
      optional(field('arguments', $.argument_list)),
    ))),

    // `extern int errno __asm__("__errno_location");` — GCC's assembler
    // name, which glibc puts on a great many declarations. Admitted only
    // here, after a declarator, and not through `_attribute`: an
    // `_attribute` is valid in half a dozen positions, and one of them is
    // the start of a statement, where `__asm__("nop");` would then have a
    // second reading as an attribute plus an empty statement.
    asm_label: $ => seq(
      choice('asm', '__asm__', '__asm'),
      '(',
      choice($.string_literal, $.concatenated_string),
      ')',
    ),

    // ── parameters ───────────────────────────────────────────────────
    // The nested parameter list is glibc's `__REDIRECT` family and
    // openssl's `OSSL_CORE_MAKE_FUNC`:
    //
    //   extern int __REDIRECT_NTH (aio_read, (struct aiocb *__aiocbp), aio_read64);
    //
    // A macro that takes a name, a parameter list and an alias, and expands
    // to a declaration with an `__asm__` label on it. Nothing here says that
    // is what it means — what the parser is handed is a declarator followed
    // by a parenthesised list whose middle element is itself a parenthesised
    // list of parameters, and reading it as one is the only reading there
    // is. This is the same concession `macro_type_specifier` and
    // `unexpanded_macro` make, in the one position left that needed it.
    //
    // The cost is `int f(int a, (int b));`, which clang rejects. It is
    // narrow because a parameter declaration cannot start with `(` — the
    // nested list is the only reading of that token, so nothing that parsed
    // before parses differently now.
    parameter_list: $ => seq(
      '(',
      commaSep(choice($.parameter_declaration, $.variadic_parameter, $.parameter_list)),
      ')',
    ),

    parameter_declaration: $ => seq(
      $._declaration_specifiers,
      optional(field('declarator', choice($._declarator, $._abstract_declarator))),
    ),

    variadic_parameter: _ => '...',

    // K&R: `int f(a, b) int a; char *b; { … }`. Still the definition style
    // of a great deal of the C a distribution ships. At `f(a)` nothing has
    // yet decided whether `a` is a type or a parameter name, and the two
    // readings are carried side by side until the body or the `;` settles
    // it — `[$._type, $.identifier_list]` is where that is declared.
    identifier_list: $ => seq(
      '(',
      commaSep1($.identifier),
      ')',
    ),

    // ── functions ────────────────────────────────────────────────────
    // The K&R form — `int f(a, b) int a; char *b; { … }` — is not a
    // curiosity: a distribution's C is full of it, and the parameter
    // declarations sitting between the declarator and the body are what
    // `identifier_list` above exists for.
    function_definition: $ => seq(
      $._declaration_specifiers,
      field('declarator', $._declarator),
      repeat(field('parameters', $.declaration)),
      field('body', $._body),
    ),

    // One member, matching rust and typescript: `_body` is the EXECUTABLE
    // body of a definition, not any braced region. A struct's
    // `field_declaration_list` and an enum's `enumerator_list` are not
    // bodies in that sense — their contents are `_member`s, which is the
    // role that answers for them.
    _body: $ => choice($.compound_statement),

    // ── names ────────────────────────────────────────────────────────
    // One member, and that is a fact about C rather than a shortfall. C has
    // a single lexical class of names: `foo` in `foo bar;` is a type only
    // because a typedef the parser cannot see says so. `type_identifier`
    // exists as an ALIAS in type position — where the position, not the
    // token, is what decides — and is covered by `_type` there. A naming
    // position admits exactly one node type, so `_name` has exactly one
    // member; what a name names is carried by the position and the field.
    _name: $ => $.identifier,

    identifier: _ => /[a-zA-Z_$][a-zA-Z0-9_$]*/,

    // ── statements ───────────────────────────────────────────────────
    _statement: $ => choice(
      $._declaration,
      $.compound_statement,
      $.expression_statement,
      $.labeled_statement,
      $.attributed_statement,
      $.asm_statement,
      $._control_flow,
    ),

    compound_statement: $ => seq(
      '{',
      repeat($._block_item),
      // C23 6.8.1, and every compiler for far longer: a label may end a
      // block with no statement after it — the `out:` that the `goto out;`
      // idiom leaves before the closing brace. Admitted HERE and nowhere
      // else, so a dangling label anywhere else stays an error.
      optional($.trailing_label),
      '}',
    ),

    trailing_label: $ => seq(field('label', $._name), ':'),

    expression_statement: $ => seq(
      optional(choice($._expression, $.comma_expression)),
      ';',
    ),

    labeled_statement: $ => seq(
      field('label', $._name),
      ':',
      $._statement,
    ),

    attributed_statement: $ => seq(repeat1($._attribute), $._statement),

    _control_flow: $ => choice($._branch, $._loop, $._jump),

    // `conditional_expression` is deliberately NOT here, and the reason is
    // structural rather than a judgement about `?:`. `_branch` nests inside
    // `_control_flow`, which nests inside `_statement` because that is what
    // C's control flow is — so a `?:` admitted here would make a bare
    // `a ? b : c` parse as a statement, with no semicolon in sight. One
    // alternation cannot be a statement in one member and an expression in
    // another. TypeScript, which has the same pair, draws the line here too.
    _branch: $ => choice(
      $.if_statement,
      $.switch_statement,
    ),

    // The dangling else, resolved the way C resolves it: `else` binds to
    // the nearest unmatched `if`, which is what `prec.right` says here.
    if_statement: $ => prec.right(seq(
      'if',
      field('condition', $.parenthesized_expression),
      field('consequence', $._statement),
      optional(field('alternative', $.else_clause)),
    )),

    else_clause: $ => seq('else', $._statement),

    switch_statement: $ => seq(
      'switch',
      field('condition', $.parenthesized_expression),
      field('body', alias($.switch_body, $.compound_statement)),
    ),

    // A switch body is a compound statement that may additionally hold case
    // labels, and `case` is admitted nowhere else — which is why this is a
    // rule of its own rather than a `case` alternative inside `_statement`.
    switch_body: $ => seq(
      '{',
      repeat(choice($.case_clause, $.default_clause, $._block_item)),
      '}',
    ),

    // `case 'a' ... 'z':` is a GNU extension and every character
    // classification switch in a distribution uses it. The range end is a
    // field of its own, so a plain `case` is unchanged.
    case_clause: $ => seq(
      'case',
      field('value', $._expression),
      optional(seq('...', field('end', $._expression))),
      ':',
      repeat($._block_item),
    ),

    default_clause: $ => seq(
      'default',
      ':',
      repeat($._block_item),
    ),

    _loop: $ => choice(
      $.while_statement,
      $.do_statement,
      $.for_statement,
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

    for_statement: $ => seq(
      'for',
      '(',
      choice(
        field('initializer', $.declaration),
        seq(field('initializer', optional(choice($._expression, $.comma_expression))), ';'),
      ),
      field('condition', optional(choice($._expression, $.comma_expression))),
      ';',
      field('update', optional(choice($._expression, $.comma_expression))),
      ')',
      field('body', $._statement),
    ),

    _jump: $ => choice(
      $.return_statement,
      $.break_statement,
      $.continue_statement,
      $.goto_statement,
    ),

    return_statement: $ => seq(
      'return',
      optional(field('value', choice($._expression, $.comma_expression))),
      ';',
    ),

    break_statement: _ => seq('break', ';'),
    continue_statement: _ => seq('continue', ';'),

    goto_statement: $ => seq(
      'goto',
      // `goto *p;` is GCC's computed goto, and the label variable it needs
      // comes from `&&label`, which is why `pointer_expression` is here.
      field('label', choice($._name, $.pointer_expression)),
      ';',
    ),

    // ── inline assembly ──────────────────────────────────────────────
    // GCC's extended asm, in every spelling it has. The alternate keywords
    // come doubled AND single — `__volatile__` and `__volatile` — and a
    // libc's per-architecture headers use the short one.
    asm_statement: $ => seq(
      choice('asm', '__asm__', '__asm'),
      repeat($.asm_qualifier),
      '(',
      field('assembly_code', choice($.string_literal, $.concatenated_string)),
      optional(seq(
        ':',
        commaSep(optional($.asm_operand)),
        optional(seq(
          ':',
          commaSep(optional($.asm_operand)),
          optional(seq(
            ':',
            commaSep(optional(choice($.string_literal, $.concatenated_string))),
            optional(seq(':', commaSep(optional($._name)))),
          )),
        )),
      )),
      ')',
      ';',
    ),

    asm_qualifier: _ => choice(
      'volatile', '__volatile__', '__volatile',
      'inline', '__inline__', '__inline',
      'goto',
    ),

    asm_operand: $ => seq(
      optional(seq('[', field('symbol', $._name), ']')),
      field('constraint', choice($.string_literal, $.concatenated_string)),
      '(',
      field('value', $._expression),
      ')',
    ),

    // ── expressions ──────────────────────────────────────────────────
    // `comma_expression` is deliberately NOT a member. C's own grammar
    // splits `expression` from `assignment-expression` for exactly this
    // reason: a comma inside an argument list separates arguments, so an
    // alternation that admitted the comma operator everywhere would make
    // `f(a, b)` ambiguous with `f((a, b))` at every call in the corpus. It
    // appears at the three positions C allows it and is recorded in the
    // ledger as uncategorised, with this reason.
    _expression: $ => choice(
      $.conditional_expression,
      $._assignment,
      $.binary_expression,
      $.unary_expression,
      $.update_expression,
      $.cast_expression,
      $.pointer_expression,
      $.sizeof_expression,
      $.alignof_expression,
      $.generic_expression,
      $.extension_expression,
      $.compound_literal_expression,
      $.parenthesized_expression,
      $._invocation,
      $._access,
      $._literal,
      $.identifier,
    ),

    comma_expression: $ => seq(
      field('left', $._expression),
      ',',
      field('right', choice($._expression, $.comma_expression)),
    ),

    conditional_expression: $ => prec.right(PREC.CONDITIONAL, seq(
      field('condition', $._expression),
      '?',
      // GCC's elvis: `a ?: b` is `a ? a : b`.
      optional(field('consequence', choice($._expression, $.comma_expression))),
      ':',
      field('alternative', $._expression),
    )),

    _assignment: $ => choice($.assignment_expression),

    assignment_expression: $ => prec.right(PREC.ASSIGNMENT, seq(
      field('left', $._assignment_target),
      field('operator', choice(
        '=', '*=', '/=', '%=', '+=', '-=', '<<=', '>>=', '&=', '^=', '|=',
      )),
      field('right', $._expression),
    )),

    // An assignment target is an lvalue expression. Spelled out rather than
    // taken as any `_expression` so that `1 = x` is not silently a parse.
    _assignment_target: $ => choice(
      $.identifier,
      $._access,
      $.pointer_expression,
      $.parenthesized_expression,
    ),

    binary_expression: $ => choice(...binaryOperators().map(([precedence, operator]) =>
      prec.left(precedence, seq(
        field('left', $._expression),
        field('operator', operator),
        field('right', $._expression),
      )),
    )),

    unary_expression: $ => prec.left(PREC.UNARY, seq(
      field('operator', choice('!', '~', '-', '+')),
      field('argument', $._expression),
    )),

    // `*p`, `&x`, and GCC's `&&label`. Separate from `unary_expression`
    // because both operators are also binary ones, so the node type is
    // what tells a reader which reading the parser took.
    pointer_expression: $ => prec.left(PREC.CAST, seq(
      field('operator', choice('*', '&', '&&')),
      field('argument', $._expression),
    )),

    update_expression: $ => prec.right(PREC.UNARY, choice(
      seq(field('operator', choice('--', '++')), field('argument', $._expression)),
      seq(field('argument', $._expression), field('operator', choice('--', '++'))),
    )),

    cast_expression: $ => prec(PREC.CAST, seq(
      '(',
      field('type', $.type_descriptor),
      ')',
      field('value', $._expression),
    )),

    sizeof_expression: $ => prec(PREC.SIZEOF, choice(
      seq('sizeof', field('value', $._expression)),
      seq('sizeof', '(', field('type', $.type_descriptor), ')'),
    )),

    alignof_expression: $ => prec(PREC.SIZEOF, seq(
      choice('alignof', '_Alignof', '__alignof', '__alignof__'),
      '(',
      field('type', $.type_descriptor),
      ')',
    )),

    // C11's `_Generic`. Its associations are type-to-expression, which is
    // the one place in C where a bare type name sits in a comma list.
    generic_expression: $ => prec(PREC.CALL, seq(
      '_Generic',
      '(',
      field('value', $._expression),
      repeat1(seq(',', $.generic_association)),
      ')',
    )),

    generic_association: $ => seq(
      field('type', choice($.type_descriptor, 'default')),
      ':',
      field('value', $._expression),
    ),

    // `__extension__ (expr)` — GCC's pedantic-diagnostic suppressor in
    // expression position.
    extension_expression: $ => prec.right(PREC.UNARY, seq(
      $.extension_specifier,
      $._expression,
    )),

    // `(struct s){ .a = 1 }` — a compound literal. It is not a cast and
    // the brace is the whole difference, which is why the two are a
    // declared conflict.
    compound_literal_expression: $ => prec(PREC.CALL, seq(
      '(',
      field('type', $.type_descriptor),
      ')',
      field('value', $.initializer_list),
    )),

    // A statement expression — `({ int t = a; t; })` — is admitted here
    // rather than as a node of its own, because that is what it is: a
    // parenthesised thing whose contents happen to be a block.
    parenthesized_expression: $ => seq(
      '(',
      choice($._expression, $.comma_expression, $.compound_statement),
      ')',
    ),

    _invocation: $ => $.call_expression,

    call_expression: $ => prec(PREC.CALL, seq(
      field('function', $._expression),
      field('arguments', $.argument_list),
    )),

    argument_list: $ => seq('(', commaSep($._argument), ')'),

    // Three things may sit where an argument goes, and two of them are
    // there because C's macros put them there. `va_arg(ap, char *)` and
    // `offsetof(struct s, m)` pass a TYPE; a package-local macro that
    // wraps a block passes a compound statement. Neither is a call in the
    // language's sense, and neither is distinguishable from one without
    // the macro's definition — so both are admitted, with the type reading
    // at a negative dynamic precedence so that anything readable as an
    // expression stays an expression.
    _argument: $ => choice(
      $._expression,
      $.compound_statement,
      $._type_argument,
    ),

    _type_argument: $ => prec.dynamic(-1, $.type_descriptor),

    _access: $ => choice($.field_expression, $.subscript_expression),

    field_expression: $ => prec(PREC.FIELD, seq(
      field('argument', $._expression),
      field('operator', choice('.', '->')),
      field('field', $._name),
    )),

    subscript_expression: $ => prec(PREC.SUBSCRIPT, seq(
      field('argument', $._expression),
      '[',
      field('index', choice($._expression, $.comma_expression)),
      ']',
    )),

    // ── initializers ─────────────────────────────────────────────────
    // A braced initializer whose elements are guarded by `#ifdef` is one of
    // the commonest shapes in a distribution's C — tables of syscalls,
    // ioctls, partition types, error strings — so the list is shaped like
    // the enumerator list: element-then-comma, a conditional, or a bare
    // directive, each with a comma-less tail.
    initializer_list: $ => seq(
      '{',
      repeat(choice(
        seq($._initializer_list_item, ','),
        $._directive_in_initializer_list,
      )),
      optional(seq(
        choice($._initializer_list_item, $._conditional_in_initializer_list_no_comma),
        repeat($._directive_in_initializer_list),
      )),
      '}',
    ),

    _initializer_list_item: $ => choice(
      $.initializer_pair,
      $._expression,
      $.initializer_list,
    ),

    _directive_in_initializer_list: $ => choice(
      $.preproc_def,
      $.preproc_function_def,
      $.preproc_undef,
      $.preproc_call,
      alias($.preproc_if_in_initializer_list, $.preproc_if),
      alias($.preproc_ifdef_in_initializer_list, $.preproc_ifdef),
    ),

    _conditional_in_initializer_list_no_comma: $ => choice(
      alias($.preproc_if_in_initializer_list_no_comma, $.preproc_if),
      alias($.preproc_ifdef_in_initializer_list_no_comma, $.preproc_ifdef),
    ),

    // `.field = v`, `[3] = v`, and GCC's `[0 ... 7] = v` range designator
    // and its obsolete `field: v` spelling, both of which real code uses.
    initializer_pair: $ => choice(
      seq(
        field('designator', repeat1(choice($.field_designator, $.subscript_designator))),
        '=',
        field('value', choice($._expression, $.initializer_list)),
      ),
      seq(
        field('designator', $._name),
        ':',
        field('value', choice($._expression, $.initializer_list)),
      ),
    ),

    field_designator: $ => seq('.', field('field', $._name)),

    subscript_designator: $ => seq(
      '[',
      field('index', $._expression),
      optional(seq('...', field('end', $._expression))),
      ']',
    ),

    // ── literals ─────────────────────────────────────────────────────
    // Every one of these satisfies `_literal`'s per-rule test: a C string
    // has no interpolation at any version, so unlike python's `string`
    // every instance of the rule is fully determined by its own text.
    _literal: $ => choice(
      $.number_literal,
      $.char_literal,
      $.string_literal,
      $.concatenated_string,
      $.true,
      $.false,
      $.null,
    ),

    // One token, and a permissive one on purpose. C's numeric syntax is
    // decided by the compiler's constant folder, not by the lexer: `0x1p-3`,
    // `1'000'000` (C23), `100ULL`, `1e10f` and `0b1010` are all one
    // pp-number, and a grammar that tried to spell the valid combinations
    // out would reject the next suffix a compiler adds.
    number_literal: _ => {
      const separator = "'";
      const hex = /[0-9a-fA-F]/;
      const decimal = /[0-9]/;
      const hexDigits = seq(repeat1(hex), repeat(seq(separator, repeat1(hex))));
      const decimalDigits = seq(repeat1(decimal), repeat(seq(separator, repeat1(decimal))));
      return token(seq(
        optional(/[-\+]/),
        optional(choice('0x', '0X', '0b', '0B')),
        choice(
          seq(
            choice(decimalDigits, seq('0b', decimalDigits), seq(choice('0x', '0X'), hexDigits)),
            optional(seq('.', optional(choice(decimalDigits, hexDigits)))),
          ),
          seq('.', decimalDigits),
        ),
        optional(seq(
          choice('e', 'E', 'p', 'P'),
          optional(seq(optional(/[-\+]/), hexDigits)),
        )),
        // The suffix set is open: `u`, `l`, `f`, `z` (C23), `i` (GNU
        // imaginary), and whatever arrives next.
        repeat(choice(/[uUlLfFdDzZiIjJ]/, seq('_', /[a-zA-Z_0-9]+/))),
      ));
    },

    // The prefixes are the encodings: `u8`, `u`, `U`, `L`. GCC and MSVC
    // both accept a multi-character constant (`'abcd'`), so the body is a
    // repeat rather than exactly one character.
    char_literal: $ => seq(
      choice('L\'', 'u\'', 'U\'', 'u8\'', '\''),
      repeat1(choice($.escape_sequence, /[^\n'\\]/)),
      '\'',
    ),

    string_literal: $ => seq(
      choice('L"', 'u"', 'U"', 'u8"', '"'),
      repeat(choice(
        token.immediate(prec(1, /[^\\"\n]+/)),
        $.escape_sequence,
        // A string may be continued across a line by a backslash, and a
        // macro that builds one uses it constantly.
        token.immediate(/\\\r?\n/),
      )),
      '"',
    ),

    // `"a" "b"` is ONE literal by C's own translation phase 6, and the
    // shape every long message and every `PRIu64`-style format string in
    // the corpus takes. A macro identifier is admitted between the pieces
    // because that is what the format-macro idiom looks like before the
    // preprocessor runs.
    concatenated_string: $ => prec.right(seq(
      $.string_literal,
      repeat1(choice($.string_literal, $.identifier)),
    )),

    escape_sequence: _ => token.immediate(seq(
      '\\',
      choice(
        /[^xuU0-7]/,
        /[0-7]{1,3}/,
        /x[0-9a-fA-F]{1,}/,
        /u[0-9a-fA-F]{4}/,
        /U[0-9a-fA-F]{8}/,
      ),
    )),

    // Only the keywords. `TRUE`, `FALSE` and `NULL` are MACROS, and
    // `_literal` is defined as a value fully determined by its own text —
    // theirs is determined by a `#define` the parser cannot see, and a
    // grammar that called them literals would be claiming to know what
    // `#define TRUE 2` says. They parse as the identifiers they are.
    true: _ => 'true',
    false: _ => 'false',
    null: _ => 'nullptr',

    // ── comments ─────────────────────────────────────────────────────
    // One node for both spellings. `//` is C99 and universal long before
    // it; the `/* */` body is written as a negated class rather than a
    // lazy `.*?` because tree-sitter's lexer has no laziness.
    comment: _ => token(choice(
      seq('//', /(\\+(.|\r?\n)|[^\\\n])*/),
      seq('/*', /[^*]*\*+([^/*][^*]*\*+)*/, '/'),
    )),
  },
});

// ── helpers ────────────────────────────────────────────────────────────

/**
 * A directive keyword token. The `#` may be followed by whitespace — `#  if
 * defined(X)` is legal and real headers indent their conditionals that way
 * — so the whole thing is one token rather than a `'#'` plus a word.
 *
 * @param {string} word
 */
function preprocessor(word) {
  return token(seq('#', /[ \t]*/, word));
}

/**
 * C's binary operator ladder, as `[precedence, operator]` pairs. Shared by
 * `binary_expression` and its preprocessor twin so the two can never drift
 * — a `#if a && b || c` must group the way the language groups it.
 */
function binaryOperators() {
  return [
    [PREC.LOGICAL_OR, '||'],
    [PREC.LOGICAL_AND, '&&'],
    [PREC.INCLUSIVE_OR, '|'],
    [PREC.EXCLUSIVE_OR, '^'],
    [PREC.BITWISE_AND, '&'],
    [PREC.EQUAL, choice('==', '!=')],
    [PREC.RELATIONAL, choice('>', '>=', '<=', '<')],
    [PREC.SHIFT, choice('<<', '>>')],
    [PREC.ADD, choice('+', '-')],
    [PREC.MULTIPLY, choice('*', '/', '%')],
  ];
}

/**
 * The five conditional rules, instantiated once per context they may
 * interrupt.
 *
 * This generator is the answer to the fact that decides this grammar's
 * shape: a `#if` does not enclose a *thing*, it encloses a RUN OF WHATEVER
 * WAS THERE — statements in a block, members in a struct, enumerators in an
 * enum, elements in an initializer list. tree-sitter has no rule
 * parameters, so each context needs its own copy, and each copy is aliased
 * back to the same public node name so a consumer sees one `preproc_if`
 * however deeply it is nested.
 *
 * Writing the seven copies by hand was the alternative. It is ~350 lines of
 * near-identical rules in which a single wrong `content` reference is
 * invisible, which is the failure this generator exists to make impossible.
 *
 * @param {string} suffix         the context's rule-name suffix, '' for top level
 * @param {(($: any) => any)} content   what one element of the enclosed run is
 * @param {number} precedence     lifts the no-comma variants above their comma'd twins
 */
function preprocIf(suffix, content, precedence = 0) {
  /** @param {any} $ */
  function alternative($) {
    return choice(
      alias($['preproc_else' + suffix], $.preproc_else),
      alias($['preproc_elif' + suffix], $.preproc_elif),
      alias($['preproc_elifdef' + suffix], $.preproc_elifdef),
    );
  }

  return {
    ['preproc_if' + suffix]: $ => prec(precedence, seq(
      preprocessor('if'),
      field('condition', $._preproc_expression),
      '\n',
      repeat(content($)),
      field('alternative', optional(alternative($))),
      preprocessor('endif'),
    )),

    ['preproc_ifdef' + suffix]: $ => prec(precedence, seq(
      choice(preprocessor('ifdef'), preprocessor('ifndef')),
      field('name', $.identifier),
      repeat(content($)),
      field('alternative', optional(alternative($))),
      preprocessor('endif'),
    )),

    ['preproc_else' + suffix]: $ => prec(precedence, seq(
      preprocessor('else'),
      repeat(content($)),
    )),

    ['preproc_elif' + suffix]: $ => prec(precedence, seq(
      preprocessor('elif'),
      field('condition', $._preproc_expression),
      '\n',
      repeat(content($)),
      field('alternative', optional(alternative($))),
    )),

    // C23 named `#elifdef` and `#elifndef`; clang and gcc shipped them
    // before it, and a header that uses one is otherwise a total loss.
    ['preproc_elifdef' + suffix]: $ => prec(precedence, seq(
      choice(preprocessor('elifdef'), preprocessor('elifndef')),
      field('name', $.identifier),
      repeat(content($)),
      field('alternative', optional(alternative($))),
    )),
  };
}

/** @param {RuleOrLiteral} rule */
function commaSep(rule) {
  return optional(commaSep1(rule));
}

/** @param {RuleOrLiteral} rule */
function commaSep1(rule) {
  return seq(rule, repeat(seq(',', rule)));
}
