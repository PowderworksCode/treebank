// GENERATED from an SDF3 module by treebank-sdf3; grammar.json is the
// artifact the parser is generated from, this file is for reading.

module.exports = grammar({
  name: "pyish",
  word: $ => $.id,
  extras: $ => [/[ \t\n\r]/, $.comment],
  supertypes: $ => [$._stmt, $._exp],
  externals: $ => [$._newline, $._indent, $._dedent, $._error_sentinel],
  reserved: { global: $ => ["def", "else", "global", "if", "pass", "return", "while"] },
  rules: {
    program: $ => repeat($._stmt),

    assign: $ => seq(
      field("target", $.id),
      "=",
      field("value", $._exp),
      $._newline
    ),

    expr: $ => seq(
      $._exp,
      $._newline
    ),

    return: $ => seq(
      "return",
      field("value", $._exp),
      $._newline
    ),

    global: $ => seq(
      "global",
      field("names", seq(
        $.id,
        repeat(seq(
          ",",
          $.id
        ))
      )),
      $._newline
    ),

    pass: $ => seq(
      "pass",
      $._newline
    ),

    if: $ => seq(
      "if",
      field("condition", $._exp),
      ":",
      $._indent,
      field("consequence", $.block),
      $._dedent,
      field("alternative", optional($.else_clause))
    ),

    while: $ => seq(
      "while",
      field("condition", $._exp),
      ":",
      $._indent,
      field("body", $.block),
      $._dedent
    ),

    def: $ => seq(
      "def",
      field("name", $.id),
      "(",
      field("parameters", optional(seq(
        $.param,
        repeat(seq(
          ",",
          $.param
        ))
      ))),
      ")",
      ":",
      $._indent,
      field("body", $.block),
      $._dedent
    ),

    _stmt: $ => choice(
      $.assign,
      $.expr,
      $.return,
      $.global,
      $.pass,
      $.if,
      $.while,
      $.def
    ),

    else_clause: $ => seq(
      "else",
      ":",
      $._indent,
      field("body", $.block),
      $._dedent
    ),

    block: $ => repeat1($._stmt),

    param: $ => field("name", $.id),

    exp_int: $ => $.int,

    call: $ => prec(5, seq(
      field("function", $._exp),
      "(",
      field("arguments", optional(seq(
        $._exp,
        repeat(seq(
          ",",
          $._exp
        ))
      ))),
      ")"
    )),

    neg: $ => prec(4, seq(
      "-",
      field("operand", $._exp)
    )),

    mul: $ => prec.left(3, seq(
      field("left", $._exp),
      "*",
      field("right", $._exp)
    )),

    add: $ => prec.left(2, seq(
      field("left", $._exp),
      "+",
      field("right", $._exp)
    )),

    sub: $ => prec.left(2, seq(
      field("left", $._exp),
      "-",
      field("right", $._exp)
    )),

    lt: $ => prec.left(1, seq(
      field("left", $._exp),
      "<",
      field("right", $._exp)
    )),

    exp_bracket: $ => seq(
      "(",
      $._exp,
      ")"
    ),

    _exp: $ => choice(
      $.id,
      $.exp_int,
      $.call,
      $.neg,
      $.mul,
      $.add,
      $.sub,
      $.lt,
      $.exp_bracket
    ),

    id: $ => /[a-zA-Z_](?:[a-zA-Z0-9_])*/,

    int: $ => /(?:[0-9])+/,

    comment: $ => /#(?:[^\n\r])*/,

  },
});
