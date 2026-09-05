// GENERATED from an SDF3 module by treebank-sdf3; grammar.json is the
// artifact the parser is generated from, this file is for reading.

module.exports = grammar({
  name: "rustish",
  word: $ => $.id,
  extras: $ => [/[ \t\n\r]/, $.comment],
  supertypes: $ => [$._stmt, $._exp],
  conflicts: $ => [[$._stmt, $._exp]],
  reserved: { global: $ => ["fn", "i64", "let"] },
  rules: {
    program: $ => repeat($.fn),

    fn: $ => seq(
      "fn",
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
      field("ret", optional($.ret)),
      field("body", $.block)
    ),

    ret: $ => seq(
      "->",
      "i64"
    ),

    param: $ => seq(
      field("name", $.id),
      ":",
      "i64"
    ),

    block: $ => seq(
      "{",
      repeat($._stmt),
      field("tail", optional($._exp)),
      "}"
    ),

    let: $ => seq(
      "let",
      field("pattern", $.id),
      "=",
      field("value", $._exp),
      ";"
    ),

    print: $ => seq(
      "println!",
      "(",
      "\"",
      "{",
      "}",
      "\"",
      ",",
      field("value", $._exp),
      ")",
      ";"
    ),

    expr: $ => seq(
      $._exp,
      ";"
    ),

    _stmt: $ => choice(
      $.let,
      $.print,
      $.expr,
      $.fn,
      $.block
    ),

    exp_int: $ => $.int,

    call: $ => prec(4, seq(
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

    neg: $ => prec(3, seq(
      "-",
      field("operand", $._exp)
    )),

    mul: $ => prec.left(2, seq(
      field("left", $._exp),
      "*",
      field("right", $._exp)
    )),

    add: $ => prec.left(1, seq(
      field("left", $._exp),
      "+",
      field("right", $._exp)
    )),

    sub: $ => prec.left(1, seq(
      field("left", $._exp),
      "-",
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
      $.block,
      $.call,
      $.neg,
      $.mul,
      $.add,
      $.sub,
      $.exp_bracket
    ),

    id: $ => /[a-zA-Z_](?:[a-zA-Z0-9_])*/,

    int: $ => /(?:[0-9])+/,

    comment: $ => /\/\/(?:[^\n\r])*/,

  },
});
