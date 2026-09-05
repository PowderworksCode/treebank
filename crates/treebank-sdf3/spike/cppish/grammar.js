// GENERATED from an SDF3 module by treebank-sdf3; grammar.json is the
// artifact the parser is generated from, this file is for reading.

module.exports = grammar({
  name: "cppish",
  word: $ => $.id,
  extras: $ => [/[ \t\n\r]/, $.comment],
  supertypes: $ => [$._stmt, $._type, $._exp],
  conflicts: $ => [[$.template_id, $._exp]],
  reserved: { global: $ => ["char", "int"] },
  rules: {
    program: $ => repeat($._stmt),

    decl: $ => seq(
      field("type", $._type),
      field("name", $.id),
      ";"
    ),

    assign: $ => seq(
      field("target", $.id),
      "=",
      field("value", $._exp),
      ";"
    ),

    expr_stmt: $ => seq(
      $._exp,
      ";"
    ),

    _stmt: $ => choice(
      $.decl,
      $.assign,
      $.expr_stmt
    ),

    int_type: $ => "int",

    char_type: $ => "char",

    template_id: $ => prec.dynamic(1, seq(
      field("name", $.id),
      "<",
      field("arguments", seq(
        $._type,
        repeat(seq(
          ",",
          $._type
        ))
      )),
      ">"
    )),

    _type: $ => choice(
      $.int_type,
      $.char_type,
      $.id,
      $.template_id
    ),

    exp_num: $ => $.num,

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

    shr: $ => prec.left(2, seq(
      field("left", $._exp),
      ">>",
      field("right", $._exp)
    )),

    add: $ => prec.left(3, seq(
      field("left", $._exp),
      "+",
      field("right", $._exp)
    )),

    lt: $ => prec.left(1, seq(
      field("left", $._exp),
      "<",
      field("right", $._exp)
    )),

    gt: $ => prec.left(1, seq(
      field("left", $._exp),
      ">",
      field("right", $._exp)
    )),

    _exp: $ => choice(
      $.id,
      $.exp_num,
      $.call,
      $.shr,
      $.add,
      $.lt,
      $.gt
    ),

    id: $ => /[a-zA-Z_](?:[a-zA-Z0-9_])*/,

    comment: $ => /\/\/(?:[^\n\r])*/,

    num: $ => /(?:[0-9])+/,

  },
});
