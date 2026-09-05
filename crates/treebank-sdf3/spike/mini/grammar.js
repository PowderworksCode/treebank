// GENERATED from an SDF3 module by treebank-sdf3; grammar.json is the
// artifact the parser is generated from, this file is for reading.

module.exports = grammar({
  name: "mini",
  word: $ => $.id,
  extras: $ => [/[ \t\n\r]/, $.comment],
  supertypes: $ => [$._stmt, $._exp],
  reserved: { global: $ => ["else", "fun", "if", "let", "return", "while"] },
  rules: {
    program: $ => field("body", repeat($._stmt)),

    let: $ => seq(
      "let",
      field("name", $.id),
      "=",
      field("value", $._exp),
      ";"
    ),

    assign: $ => seq(
      field("target", $.id),
      "=",
      field("value", $._exp),
      ";"
    ),

    if: $ => seq(
      "if",
      "(",
      field("condition", $._exp),
      ")",
      field("consequence", $.block),
      "else",
      field("alternative", $.block)
    ),

    while: $ => seq(
      "while",
      "(",
      field("condition", $._exp),
      ")",
      field("body", $.block)
    ),

    fun: $ => seq(
      "fun",
      field("name", $.id),
      "(",
      field("parameters", optional(seq(
        $.id,
        repeat(seq(
          ",",
          $.id
        ))
      ))),
      ")",
      field("body", $.block)
    ),

    return: $ => seq(
      "return",
      field("value", $._exp),
      ";"
    ),

    expr: $ => seq(
      $._exp,
      ";"
    ),

    _stmt: $ => choice(
      $.let,
      $.assign,
      $.if,
      $.while,
      $.fun,
      $.return,
      $.expr
    ),

    block: $ => seq(
      "{",
      repeat($._stmt),
      "}"
    ),

    exp_int: $ => $.int,

    call: $ => seq(
      field("function", $.id),
      "(",
      field("arguments", optional(seq(
        $._exp,
        repeat(seq(
          ",",
          $._exp
        ))
      ))),
      ")"
    ),

    neg: $ => prec(4, seq(
      "-",
      field("operand", $._exp)
    )),

    not: $ => prec(4, seq(
      "!",
      field("operand", $._exp)
    )),

    mul: $ => prec.left(3, seq(
      field("left", $._exp),
      "*",
      field("right", $._exp)
    )),

    div: $ => prec.left(3, seq(
      field("left", $._exp),
      "/",
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

    eq: $ => prec.left(1, seq(
      field("left", $._exp),
      "==",
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
      $.not,
      $.mul,
      $.div,
      $.add,
      $.sub,
      $.eq,
      $.lt,
      $.exp_bracket
    ),

    id: $ => /[a-zA-Z_](?:[a-zA-Z0-9_])*/,

    int: $ => /(?:[0-9])+/,

    comment: $ => /\/\/(?:[^\n\r])*/,

  },
});
