/**
 * treebank-bash: a from-scratch grammar for GNU bash 5.x, carrying the
 * treebank vocabulary (DESIGN.md §3) in its parse table.
 *
 * Shell is not like the other languages here. There is no expression
 * grammar to speak of: almost everything is a WORD, and what a word means
 * is decided by position and by expansion at run time. So the vocabulary
 * lands differently — `_expression` covers the things that produce a value
 * (expansions, substitutions, arithmetic), while the bulk of a script is
 * `_statement` and `_invocation`.
 *
 * Omissions and the reasons for them are in ledger.toml's roles_note.
 */

/// <reference types="tree-sitter-cli/dsl" />
// @ts-check

const tb = require('../treebank-core/vocabulary/supertypes.js');

const PREC = {
  list: 1,        // ; & && ||
  pipeline: 2,    // | |&
  redirect: 3,
  word: 4,
  expansion: 5,
};

module.exports = grammar({
  name: 'bash',

  // Bash's own word boundary. `extras` deliberately does NOT include a
  // newline: a newline is a command terminator here, not whitespace.
  extras: $ => [$.comment, /[ \t]/, /\\\r?\n/],

  externals: $ => [
    $.heredoc_start,
    $.heredoc_body,
    $.heredoc_end,
    $._concat,
    $._assignment_name,
    $._file_descriptor,
    $._error_sentinel,
  ],

  supertypes: $ => tb.assertTableTerms([
    '_statement',
    '_expression',
    // `_name` is NOT threaded. Shell has no name category distinct from
    // words: a command name, a function name and a loop variable are all
    // just words, and `variable_name` exists only inside an expansion,
    // where it cannot be confused with anything. Threading `_name` through
    // the word positions would put `variable_name` back into the lexer's
    // choice at a command's head, which is the one place it must not be.
    '_literal',
    '_argument',
    '_body',
    '_control_flow',
    '_branch',
    '_loop',
    '_jump',
    '_assignment',
    '_invocation',
    '_access',
    '_directive',
  ]).map((name) => $[name]),

  conflicts: $ => [
    [$._statements, $._terminator],
    [$.string],
    [$._statement, $._body],
    [$._statements],
    [$.command, $._assignment],
    [$.continue_statement],
    [$.break_statement],
    [$.return_statement],
    [$.expansion],
    [$.variable_assignment],],

  rules: {
    // A file of nothing but comments and blank lines is a program: the
    // newline token is a real terminator here, not whitespace, so it needs
    // somewhere to land when no statement ever arrives.
    program: $ => optional(choice($._statements, repeat1('\n'))),

    // Newlines are terminators here, not whitespace, so every blank line
    // and every block that opens on its own line has to be tolerated
    // explicitly. Without the `repeat('\n')` a function body written over
    // more than one line does not parse, which is most of them.
    _statements: $ => prec.right(seq(
      repeat('\n'),
      repeat(seq($._statement, $._terminator, repeat('\n'))),
      $._statement,
      optional($._terminator),
      repeat('\n'),
    )),

    // `;;` is NOT here. It terminates a case ITEM, not a statement, and
    // listing it meant `optional($._statements)` inside a case_item ate the
    // item's own terminator: a whole multi-item `case` collapsed into one
    // case_item, and the realistic multi-line form did not parse at all.
    // No corpus file ever produced a clean case_statement.
    _terminator: $ => choice(';', '\n', '&'),

    // ── statements ───────────────────────────────────────────────────
    _statement: $ => choice(
      $._control_flow,
      $._invocation,
      $._assignment,
      $.list,
      $.pipeline,
      $.subshell,
      $.compound_statement,
      $.function_definition,
      $.negated_command,
      $._directive,
      $.arithmetic_command,
      $.test_command,
    ),

    // `set -e` and friends are not directives; a shebang is.
    _directive: $ => $.shebang,
    shebang: _ => token(prec(2, seq('#!', /[^\r\n]*/))),

    // A newline may follow the operator: `cmd &&` at the end of a line
    // continues on the next, which is how any long chain is written.
    list: $ => prec.left(PREC.list, seq(
      field('left', $._statement),
      field('operator', choice('&&', '||')),
      repeat('\n'),
      field('right', $._statement),
    )),

    pipeline: $ => prec.left(PREC.pipeline, seq(
      field('left', $._statement),
      field('operator', choice('|', '|&')),
      repeat('\n'),
      field('right', $._statement),
    )),

    negated_command: $ => seq('!', $._statement),

    // Any compound command may carry trailing redirects:
    // `{ ...; } 4>&1` and `(...) 2>/dev/null` are everyday shell.
    subshell: $ => prec.left(seq('(', $._statements, ')', repeat($.redirect))),
    compound_statement: $ => prec.left(seq('{', $._statements, '}', repeat($.redirect))),

    _body: $ => choice($.do_group, $.compound_statement, $.subshell),

    do_group: $ => seq('do', optional($._statements), 'done'),

    // ── control flow ─────────────────────────────────────────────────
    _control_flow: $ => choice($._branch, $._loop, $._jump),

    _branch: $ => choice($.if_statement, $.case_statement),

    if_statement: $ => seq(
      'if',
      field('condition', $._statements),
      'then',
      field('consequence', optional($._statements)),
      repeat(field('alternative', $.elif_clause)),
      optional(field('alternative', $.else_clause)),
      'fi',
    ),

    elif_clause: $ => seq('elif', $._statements, 'then', optional($._statements)),
    else_clause: $ => seq('else', optional($._statements)),

    case_statement: $ => seq(
      'case',
      field('value', $._word_like),
      repeat('\n'),
      'in',
      repeat('\n'),
      repeat($.case_item),
      // The LAST item may drop its terminator: `case x in a) echo ;; b) echo
      // esac` is legal. Spelling that as a separate rule rather than adding
      // `esac` to the item's terminator choice, which is what this grammar
      // did before -- that made the item CONSUME the `esac` the statement
      // still needed, so a case wanted two of them and no real file had one.
      optional(alias($._case_item_last, $.case_item)),
      'esac',
    ),

    _case_item_last: $ => seq(
      optional('('),
      field('pattern', $._case_patterns),
      ')',
      optional($._case_body),
    ),

    // The last item may drop its terminator -- `case x in a) echo ;; b) echo
    // esac` is legal -- so the terminator is optional and the newlines that
    // follow it belong to the item. Without those `repeat('\n')` the rule
    // admitted only a single-line case.
    case_item: $ => seq(
      optional('('),
      field('pattern', $._case_patterns),
      ')',
      optional($._case_body),
      choice(';;', ';&', ';;&'),
      repeat('\n'),
    ),

    // An item body is either statements or nothing but blank lines --
    // `a)` followed by a newline and `;;` is common, and `_statements`
    // cannot match it because it requires at least one statement.
    _case_body: $ => choice($._statements, repeat1('\n')),

    _case_patterns: $ => seq($._word_like, repeat(seq('|', $._word_like))),

    _loop: $ => choice($.for_statement, $.c_style_for_statement, $.while_statement, $.until_statement),

    for_statement: $ => seq(
      choice('for', 'select'),
      field('variable', alias($.word, $.variable_name)),
      optional(seq('in', field('value', repeat1($._word_like)))),
      $._terminator,
      field('body', $._body),
    ),

    c_style_for_statement: $ => seq(
      'for',
      '((',
      optional($._arithmetic),
      ';',
      optional($._arithmetic),
      ';',
      optional($._arithmetic),
      '))',
      optional($._terminator),
      field('body', choice($._body, $._statement)),
    ),

    while_statement: $ => seq('while', field('condition', $._statements), field('body', $.do_group)),
    until_statement: $ => seq('until', field('condition', $._statements), field('body', $.do_group)),

    _jump: $ => choice($.return_statement, $.break_statement, $.continue_statement),

    // These are builtins, not keywords, so they are recognised by name and
    // only where a command may start. `return 1` in a function and
    // `break 2` in a loop are the shapes that matter.
    return_statement: $ => seq('return', optional(field('value', $._word_like))),
    break_statement: $ => seq('break', optional(field('value', $._word_like))),
    continue_statement: $ => seq('continue', optional(field('value', $._word_like))),

    // ── functions ────────────────────────────────────────────────────
    // The body may open on its own line -- `clean()\n{ ... }` is the
    // common K&R-ish shell style and 408 corpus files -- so the newlines
    // between the parens and the body belong to the definition.
    function_definition: $ => choice(
      seq('function', field('name', $.word), optional(seq('(', ')')), repeat('\n'), field('body', $._body)),
      seq(field('name', $.word), '(', ')', repeat('\n'), field('body', $._body)),
    ),

    // ── commands ─────────────────────────────────────────────────────
    _invocation: $ => $.command,

    command: $ => prec.left(seq(
      repeat($.variable_assignment),
      field('name', $._word_like),
      repeat(field('argument', $._argument)),
    )),

    _argument: $ => choice($._word_like, $.redirect),

    _assignment: $ => choice($.variable_assignment, $.declaration_command),

    variable_assignment: $ => seq(
      field('name', alias($._assignment_name, $.variable_name)),
      // `a[0]=1`: the scanner fences the NAME and peeks through the
      // brackets to find the `=`; the index itself is parsed here, with
      // the same restricted word the subscript rule uses.
      optional(seq('[', field('index', repeat1(choice($._expression, alias($._index_word, $.word)))), ']')),
      choice('=', '+='),
      field('value', optional(choice($._word_like, $.array))),
    ),

    // `local`, `declare`, `export` and friends take assignments as well as
    // words, which no ordinary command does.
    declaration_command: $ => prec.left(seq(
      choice('declare', 'typeset', 'export', 'readonly', 'local'),
      repeat(choice($.variable_assignment, $._word_like)),
    )),

    // Newlines inside the parentheses: an array written one element per
    // line is the common form for anything longer than three items.
    array: $ => seq('(', repeat(choice($._word_like, '\n')), ')'),

    // ── redirection ──────────────────────────────────────────────────
    redirect: $ => prec.left(PREC.redirect, choice(
      seq(
        optional(field('descriptor', alias($._file_descriptor, $.file_descriptor))),
        field('operator', choice('<', '>', '>>', '&>', '&>>', '<&', '>&', '>|', '<>')),
        field('destination', $._word_like),
      ),
      $.heredoc_redirect,
      // The herestring: `<<< word` feeds the word as stdin.
      seq(field('operator', '<<<'), field('destination', $._word_like)),
    )),

    heredoc_redirect: $ => seq(
      optional(field('descriptor', alias($._file_descriptor, $.file_descriptor))),
      choice('<<', '<<-'),
      $.heredoc_start,
      optional($.heredoc_body),
      optional($.heredoc_end),
    ),


    // ── tests and arithmetic ─────────────────────────────────────────
    test_command: $ => choice(
      seq('[[', $._conditional, ']]'),
      // `[` is a command and its arguments are words -- but `!=`, `=` and
      // `!` are operator characters the word token excludes, so they have
      // to be spelled: `[ "$a" != "$b" ]` errored on the `!=`.
      seq('[', repeat(choice($._word_like, '!', '!=', '==', '=', '-a', '-o', '(', ')')), ']'),
    ),

    _conditional: $ => repeat1(choice(
      $._word_like,
      '!', '&&', '||', '==', '!=', '=~', '<', '>', '-a', '-o', '(', ')',
    )),

    arithmetic_command: $ => seq('((', optional($._arithmetic), '))'),

    // Arithmetic is a language of its own inside `(( ))`. It is taken as a
    // run of operators and operands rather than a precedence ladder: the
    // shape a consumer wants here is "this is arithmetic", and building a
    // second expression grammar to say `a+b*c` groups one way earns nothing
    // a query would ask for.
    //
    // The OPERANDS are not `_word_like`, though: `word` admits `*` and `+`
    // as ordinary characters, so `$A*5+$B` lexed its middle as one word
    // `*5+` and the token stream inside the parens was fiction. A bare name
    // in arithmetic IS a variable reference, so the operands are names,
    // numbers and expansions, and the operators get to be themselves.
    _arithmetic: $ => repeat1(choice(
      $.variable_name,
      // Hex and explicit-base literals are arithmetic-only spellings:
      // `0x1f`, `2#101`, `10#99`. The plain `number` token stops at the
      // digits and the tail read as a variable_name.
      alias(token(prec(1, /0[xX][0-9a-fA-F]+|[0-9]+#[0-9a-zA-Z@_]+/)), $.number),
      $._expression,
      '+', '-', '*', '/', '%', '**', '=', '+=', '-=', '*=', '/=', '%=',
      '==', '!=', '<', '>', '<=', '>=', '&&', '||', '!', '~', '^', '&', '|',
      '<<', '>>', '++', '--', '?', ':', ',', '(', ')',
    )),

    // ── words and expansions ─────────────────────────────────────────
    // Everything in shell is a word. `_word_like` is the one place that
    // says what may sit where an argument, a value or a pattern goes.
    _word_like: $ => choice(
      $.word,
      $._expression,
      $.concatenation,
      $.brace_expression,
    ),

    concatenation: $ => prec(-1, seq(
      $._word_part,
      repeat1(seq($._concat, $._word_part)),
    )),

    _word_part: $ => choice($.word, $._expression, $.brace_expression),

    // A bare word: anything not special. The negated class is the grammar
    // — shell has no keyword list at the lexical level, only positions.
    // The first character may not be `#`: a hash begins a comment only at
    // the START of a word -- `foo#bar` is one word to bash, `foo #bar` is
    // a word and a comment. With `#` in the first-char class, `# comment`
    // lexed as a command NAMED `#` with the words as arguments: no error,
    // no comment node, in every file -- invisible to the sweep (nothing
    // errors) and to the span oracle (our extra nodes are not its
    // business). The ledger records that raising the comment token's
    // precedence instead made the sweep WORSE, because that steals the
    // mid-word hashes too; the word-start restriction is bash's own rule.
    word: _ => token(prec(-1, seq(
      choice(
        /[^\s'"<>{}()$`|&;!\\\[\]#]/,
        /\\[^\r\n]/,
        /\[/,
        /\]/,
      ),
      repeat(choice(
        /[^\s'"<>{}()$`|&;!\\\[\]]/,
        /\\[^\r\n]/,
        /\[/,
        /\]/,
      )),
    ))),

    // Only inside an expansion, where nothing else can be there. At a
    // command's head this token must not exist: `echo` matches it as
    // readily as it matches `word`, the lexer has to choose, and choosing
    // this one loses the command reading permanently.
    variable_name: _ => token(prec(1, /[a-zA-Z_][a-zA-Z0-9_]*/)),

    // `{a,b}` and `{1..5}`: one token, requiring the comma or the range,
    // so a braceless `{x}` stays the plain word bash treats it as. Nested
    // expansions are beyond a single token and stay a recorded gap.
    brace_expression: _ => token(prec(1, choice(
      /\{[^{}\s,]*,[^{}\s]*\}/,
      /\{[^{}\s.]*\.\.[^{}\s]*\}/,
    ))),

    _literal: $ => choice($.string, $.raw_string, $.ansi_c_string, $.number),

    number: _ => token(/[0-9]+/),

    // Double quotes: expansions happen inside, so the parts are real nodes.
    string: $ => seq(
      '"',
      repeat(choice(
        $._expansion_like,
        $.escape_sequence,
        token.immediate(prec(1, /[^"$`\\]+/)),
      )),
      '"',
    ),

    // Single quotes: nothing expands, so there is nothing to look inside.
    raw_string: _ => token(seq("'", /[^']*/, "'")),

    ansi_c_string: _ => token(seq("$'", repeat(choice(/[^'\\]/, /\\./)), "'")),

    escape_sequence: _ => token.immediate(/\\./),

    // ── expansion is where shell's values come from ──────────────────
    // `_literal` nests here because the vocabulary requires it to
    // (DESIGN.md §3.3 rule 4) and because it is true of shell: a quoted
    // string is a thing that produces a value, which is what separates the
    // expression side of this grammar from the word side.
    _expression: $ => choice(
      $._literal,
      $._access,
      $.command_substitution,
      $.arithmetic_expansion,
      $.process_substitution,
    ),

    _access: $ => choice($.simple_expansion, $.expansion),

    // What may appear INSIDE a double-quoted string: expansions, and not
    // literals. `_literal` sits under `_expression` to satisfy the
    // vocabulary's containment rule, which would otherwise let a string
    // contain a string.
    _expansion_like: $ => choice(
      $._access,
      $.command_substitution,
      $.arithmetic_expansion,
    ),

    // `$x`, `$1`, `$@`
    simple_expansion: $ => seq(
      '$',
      field('name', choice(
        $.variable_name,
        $.special_variable,
      )),
    ),

    special_variable: _ => token.immediate(/[0-9*@#?$!_-]/),

    // `${x}`, `${x:-d}`, `${x[@]}`, `${#x}`, `${x@Q}`
    expansion: $ => seq(
      '${',
      optional(choice('#', '!')),
      field('name', optional(choice($.variable_name, $.special_variable, $.subscript))),
      optional($._expansion_tail),
      '}',
    ),

    // The index may NOT be `_word_like`. `word` admits `[` and `]` (they
    // are ordinary characters in a bash word), so it matches `0]` in
    // `${arr[0]}` -- two characters against the one-character `]` token,
    // and the lexer takes the longer match every time. The closing bracket
    // was then never available and no corpus file ever produced a
    // `subscript` node. This token stops at the bracket instead; it is only
    // valid inside an index, so the higher precedence cannot leak into an
    // ordinary word.
    subscript: $ => seq(
      field('name', $.variable_name),
      '[',
      field('index', repeat1(choice($._expression, alias($._index_word, $.word)))),
      ']',
    ),

    _index_word: _ => token(prec(1, repeat1(choice(
      /[^\s'"<>{}()$`|&;!\\\[\]]/,
      /\\[^\r\n]/,
    )))),

    _expansion_tail: $ => repeat1(choice(
      $._word_like,
      ':', '-', '=', '?', '+', '#', '%', '/', '^', ',', '@', '*',
    )),

    command_substitution: $ => choice(
      seq('$(', $._statements, ')'),
      seq('`', $._statements, '`'),
    ),

    arithmetic_expansion: $ => seq('$((', optional($._arithmetic), '))'),

    process_substitution: $ => seq(choice('<(', '>('), $._statements, ')'),

    // ── comments ─────────────────────────────────────────────────────
    // `#` is an ordinary character in the word class, so `# a comment`
    // lexes as a command named `#`. Raising this above `word` was tried and
    // made the sweep WORSE — 4,607 passing files down to 3,036 — so the
    // fix is not a precedence one and the cause is recorded in ledger.toml
    // rather than guessed at again.
    comment: _ => token(prec(-2, seq('#', /[^\r\n]*/))),
  },
});
