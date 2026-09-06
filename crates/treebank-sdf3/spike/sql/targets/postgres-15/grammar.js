// GENERATED from an SDF3 module by treebank-sdf3; grammar.json is the
// artifact the parser is generated from, this file is for reading.

module.exports = grammar({
  name: "postgres_15",
  word: $ => $.name,
  extras: $ => [/[ \t\n\r]/, $.comment],
  supertypes: $ => [$._statement, $._type, $._name, $._expression, $._literal, $._invocation, $._declaration, $._assignment, $._modifier],
  reserved: { global: $ => [$._kw_and, $._kw_as, $._kw_asc, $._kw_by, $._kw_conflict, $._kw_create, $._kw_delete, $._kw_desc, $._kw_do, $._kw_drop, $._kw_from, $._kw_ilike, $._kw_insert, $._kw_int, $._kw_into, $._kw_like, $._kw_limit, $._kw_matched, $._kw_merge, $._kw_not, $._kw_nothing, $._kw_null, $._kw_offset, $._kw_oids, $._kw_on, $._kw_or, $._kw_order, $._kw_over, $._kw_partition, $._kw_returning, $._kw_select, $._kw_set, $._kw_table, $._kw_text, $._kw_then, $._kw_update, $._kw_using, $._kw_values, $._kw_varchar, $._kw_when, $._kw_where, $._kw_with, $._kw_without] },
  rules: {
    script: $ => repeat($._statement),

    stmt_select: $ => seq(
      field("with", optional($.with)),
      $.select,
      ";"
    ),

    insert: $ => seq(
      alias($._kw_insert, "INSERT"),
      alias($._kw_into, "INTO"),
      field("table", $._name),
      "(",
      field("columns", seq(
        $._name,
        repeat(seq(
          ",",
          $._name
        ))
      )),
      ")",
      alias($._kw_values, "VALUES"),
      "(",
      field("values", seq(
        $._expression,
        repeat(seq(
          ",",
          $._expression
        ))
      )),
      ")",
      field("upsert", optional($._upsert)),
      field("returning", optional($.returning)),
      ";"
    ),

    update: $ => seq(
      alias($._kw_update, "UPDATE"),
      field("table", $._name),
      alias($._kw_set, "SET"),
      seq(
        $._assignment,
        repeat(seq(
          ",",
          $._assignment
        ))
      ),
      field("where", optional($.where)),
      field("returning", optional($.returning)),
      ";"
    ),

    delete: $ => seq(
      alias($._kw_delete, "DELETE"),
      alias($._kw_from, "FROM"),
      field("table", $._name),
      field("where", optional($.where)),
      field("returning", optional($.returning)),
      ";"
    ),

    create_table: $ => seq(
      alias($._kw_create, "CREATE"),
      alias($._kw_table, "TABLE"),
      field("table", $._name),
      "(",
      seq(
        $.col_def,
        repeat(seq(
          ",",
          $.col_def
        ))
      ),
      ")",
      field("tail", optional($._modifier)),
      ";"
    ),

    _declaration: $ => choice(
      $.create_table
    ),

    drop_table: $ => seq(
      alias($._kw_drop, "DROP"),
      alias($._kw_table, "TABLE"),
      field("table", $._name),
      ";"
    ),

    merge: $ => seq(
      alias($._kw_merge, "MERGE"),
      alias($._kw_into, "INTO"),
      field("target", $._name),
      alias($._kw_using, "USING"),
      field("source", $._name),
      alias($._kw_on, "ON"),
      $._expression,
      repeat1($._when),
      ";"
    ),

    _statement: $ => choice(
      $.stmt_select,
      $.insert,
      $.update,
      $.delete,
      $._declaration,
      $.drop_table,
      $.merge
    ),

    select: $ => seq(
      alias($._kw_select, "SELECT"),
      field("items", seq(
        $.item,
        repeat(seq(
          ",",
          $.item
        ))
      )),
      field("from", optional($.from)),
      field("where", optional($.where)),
      field("order", optional($.order_by)),
      field("limit", optional($.limit)),
      field("offset", optional($.offset))
    ),

    item: $ => seq(
      $._expression,
      field("alias", optional($._alias))
    ),

    as: $ => seq(
      alias($._kw_as, "AS"),
      $._name
    ),

    bare: $ => $._name,

    _alias: $ => choice(
      $.as,
      $.bare
    ),

    from: $ => seq(
      alias($._kw_from, "FROM"),
      field("table", $._name)
    ),

    where: $ => seq(
      alias($._kw_where, "WHERE"),
      $._expression
    ),

    order_by: $ => seq(
      alias($._kw_order, "ORDER"),
      alias($._kw_by, "BY"),
      seq(
        $.order,
        repeat(seq(
          ",",
          $.order
        ))
      )
    ),

    order: $ => seq(
      $._expression,
      field("dir", optional($._modifier))
    ),

    asc: $ => alias($._kw_asc, "ASC"),

    desc: $ => alias($._kw_desc, "DESC"),

    cte: $ => seq(
      field("name", $._name),
      alias($._kw_as, "AS"),
      "(",
      $.select,
      ")"
    ),

    assign: $ => seq(
      field("column", $._name),
      "=",
      field("value", $._expression)
    ),

    _assignment: $ => choice(
      $.assign
    ),

    col_def: $ => seq(
      field("name", $._name),
      $._type
    ),

    type_int: $ => alias($._kw_int, "INT"),

    varchar: $ => seq(
      alias($._kw_varchar, "VARCHAR"),
      "(",
      $.int,
      ")"
    ),

    text: $ => alias($._kw_text, "TEXT"),

    _type: $ => choice(
      $.type_int,
      $.varchar,
      $.text
    ),

    ident_name: $ => $.name,

    quoted: $ => $.dquoted,

    _name: $ => choice(
      $.ident_name,
      $.quoted
    ),

    column: $ => seq(
      field("table", $._name),
      ".",
      field("column", $._name)
    ),

    star: $ => "*",

    exp_int: $ => $.int,

    str: $ => $.string,

    null: $ => alias($._kw_null, "NULL"),

    _literal: $ => choice(
      $.exp_int,
      $.str,
      $.null
    ),

    call: $ => prec(18, seq(
      field("function", $.name),
      "(",
      field("arguments", optional(seq(
        $._expression,
        repeat(seq(
          ",",
          $._expression
        ))
      ))),
      ")"
    )),

    _invocation: $ => choice(
      $.call
    ),

    neg: $ => prec(17, seq(
      "-",
      $._expression
    )),

    mul: $ => prec.left(16, seq(
      field("left", $._expression),
      "*",
      field("right", $._expression)
    )),

    add: $ => prec.left(15, seq(
      field("left", $._expression),
      "+",
      field("right", $._expression)
    )),

    sub: $ => prec.left(15, seq(
      field("left", $._expression),
      "-",
      field("right", $._expression)
    )),

    eq: $ => prec.left(14, seq(
      field("left", $._expression),
      "=",
      field("right", $._expression)
    )),

    lt: $ => prec.left(14, seq(
      field("left", $._expression),
      "<",
      field("right", $._expression)
    )),

    gt: $ => prec.left(14, seq(
      field("left", $._expression),
      ">",
      field("right", $._expression)
    )),

    like: $ => prec.left(14, seq(
      field("left", $._expression),
      alias($._kw_like, "LIKE"),
      field("right", $._expression)
    )),

    not: $ => prec(13, seq(
      alias($._kw_not, "NOT"),
      $._expression
    )),

    and: $ => prec.left(12, seq(
      field("left", $._expression),
      alias($._kw_and, "AND"),
      field("right", $._expression)
    )),

    or: $ => prec.left(11, seq(
      field("left", $._expression),
      alias($._kw_or, "OR"),
      field("right", $._expression)
    )),

    exp_bracket: $ => seq(
      "(",
      $._expression,
      ")"
    ),

    over: $ => prec(10, seq(
      $._expression,
      alias($._kw_over, "OVER"),
      "(",
      field("partition", optional($.partition)),
      field("order", optional($.order_by)),
      ")"
    )),

    arrow: $ => prec.left(8, seq(
      field("left", $._expression),
      "->",
      field("right", $._expression)
    )),

    arrow_text: $ => prec.left(8, seq(
      field("left", $._expression),
      "->>",
      field("right", $._expression)
    )),

    cast: $ => prec(6, seq(
      $._expression,
      "::",
      $._type
    )),

    i_like: $ => prec.left(4, seq(
      field("left", $._expression),
      alias($._kw_ilike, "ILIKE"),
      field("right", $._expression)
    )),

    _expression: $ => choice(
      $._name,
      $.column,
      $.star,
      $._literal,
      $._invocation,
      $.neg,
      $.mul,
      $.add,
      $.sub,
      $.eq,
      $.lt,
      $.gt,
      $.like,
      $.not,
      $.and,
      $.or,
      $.exp_bracket,
      $.over,
      $.arrow,
      $.arrow_text,
      $.cast,
      $.i_like
    ),

    limit: $ => seq(
      alias($._kw_limit, "LIMIT"),
      field("count", $.int)
    ),

    offset: $ => seq(
      alias($._kw_offset, "OFFSET"),
      field("start", $.int)
    ),

    with: $ => seq(
      alias($._kw_with, "WITH"),
      seq(
        $.cte,
        repeat(seq(
          ",",
          $.cte
        ))
      )
    ),

    partition: $ => seq(
      alias($._kw_partition, "PARTITION"),
      alias($._kw_by, "BY"),
      seq(
        $._expression,
        repeat(seq(
          ",",
          $._expression
        ))
      )
    ),

    returning: $ => seq(
      alias($._kw_returning, "RETURNING"),
      seq(
        $.item,
        repeat(seq(
          ",",
          $.item
        ))
      )
    ),

    without_oids: $ => seq(
      alias($._kw_without, "WITHOUT"),
      alias($._kw_oids, "OIDS")
    ),

    _modifier: $ => choice(
      $.asc,
      $.desc,
      $.without_oids
    ),

    nothing: $ => seq(
      alias($._kw_on, "ON"),
      alias($._kw_conflict, "CONFLICT"),
      alias($._kw_do, "DO"),
      alias($._kw_nothing, "NOTHING")
    ),

    upsert_update: $ => seq(
      alias($._kw_on, "ON"),
      alias($._kw_conflict, "CONFLICT"),
      "(",
      seq(
        $._name,
        repeat(seq(
          ",",
          $._name
        ))
      ),
      ")",
      alias($._kw_do, "DO"),
      alias($._kw_update, "UPDATE"),
      alias($._kw_set, "SET"),
      seq(
        $._assignment,
        repeat(seq(
          ",",
          $._assignment
        ))
      )
    ),

    _upsert: $ => choice(
      $.nothing,
      $.upsert_update
    ),

    matched: $ => seq(
      alias($._kw_when, "WHEN"),
      alias($._kw_matched, "MATCHED"),
      alias($._kw_then, "THEN"),
      alias($._kw_update, "UPDATE"),
      alias($._kw_set, "SET"),
      seq(
        $._assignment,
        repeat(seq(
          ",",
          $._assignment
        ))
      )
    ),

    not_matched: $ => seq(
      alias($._kw_when, "WHEN"),
      alias($._kw_not, "NOT"),
      alias($._kw_matched, "MATCHED"),
      alias($._kw_then, "THEN"),
      alias($._kw_insert, "INSERT"),
      "(",
      field("columns", seq(
        $._name,
        repeat(seq(
          ",",
          $._name
        ))
      )),
      ")",
      alias($._kw_values, "VALUES"),
      "(",
      field("values", seq(
        $._expression,
        repeat(seq(
          ",",
          $._expression
        ))
      )),
      ")"
    ),

    _when: $ => choice(
      $.matched,
      $.not_matched
    ),

    dquoted: $ => /"(?:[^"])*"/,

    int: $ => /(?:[0-9])+/,

    comment: $ => /--(?:[^\n\r])*/,

    name: $ => /[a-zA-Z_](?:[a-zA-Z0-9_])*/,

    string: $ => /(?:'(?:(?:''|[^']))*'|(?:\$\$(?:[^$])*\$\$))/,

    _kw_and: $ => token(prec(1, /[aA][nN][dD]/)),

    _kw_as: $ => token(prec(1, /[aA][sS]/)),

    _kw_asc: $ => token(prec(1, /[aA][sS][cC]/)),

    _kw_by: $ => token(prec(1, /[bB][yY]/)),

    _kw_conflict: $ => token(prec(1, /[cC][oO][nN][fF][lL][iI][cC][tT]/)),

    _kw_create: $ => token(prec(1, /[cC][rR][eE][aA][tT][eE]/)),

    _kw_delete: $ => token(prec(1, /[dD][eE][lL][eE][tT][eE]/)),

    _kw_desc: $ => token(prec(1, /[dD][eE][sS][cC]/)),

    _kw_do: $ => token(prec(1, /[dD][oO]/)),

    _kw_drop: $ => token(prec(1, /[dD][rR][oO][pP]/)),

    _kw_from: $ => token(prec(1, /[fF][rR][oO][mM]/)),

    _kw_ilike: $ => token(prec(1, /[iI][lL][iI][kK][eE]/)),

    _kw_insert: $ => token(prec(1, /[iI][nN][sS][eE][rR][tT]/)),

    _kw_int: $ => token(prec(1, /[iI][nN][tT]/)),

    _kw_into: $ => token(prec(1, /[iI][nN][tT][oO]/)),

    _kw_like: $ => token(prec(1, /[lL][iI][kK][eE]/)),

    _kw_limit: $ => token(prec(1, /[lL][iI][mM][iI][tT]/)),

    _kw_matched: $ => token(prec(1, /[mM][aA][tT][cC][hH][eE][dD]/)),

    _kw_merge: $ => token(prec(1, /[mM][eE][rR][gG][eE]/)),

    _kw_not: $ => token(prec(1, /[nN][oO][tT]/)),

    _kw_nothing: $ => token(prec(1, /[nN][oO][tT][hH][iI][nN][gG]/)),

    _kw_null: $ => token(prec(1, /[nN][uU][lL][lL]/)),

    _kw_offset: $ => token(prec(1, /[oO][fF][fF][sS][eE][tT]/)),

    _kw_oids: $ => token(prec(1, /[oO][iI][dD][sS]/)),

    _kw_on: $ => token(prec(1, /[oO][nN]/)),

    _kw_or: $ => token(prec(1, /[oO][rR]/)),

    _kw_order: $ => token(prec(1, /[oO][rR][dD][eE][rR]/)),

    _kw_over: $ => token(prec(1, /[oO][vV][eE][rR]/)),

    _kw_partition: $ => token(prec(1, /[pP][aA][rR][tT][iI][tT][iI][oO][nN]/)),

    _kw_returning: $ => token(prec(1, /[rR][eE][tT][uU][rR][nN][iI][nN][gG]/)),

    _kw_select: $ => token(prec(1, /[sS][eE][lL][eE][cC][tT]/)),

    _kw_set: $ => token(prec(1, /[sS][eE][tT]/)),

    _kw_table: $ => token(prec(1, /[tT][aA][bB][lL][eE]/)),

    _kw_text: $ => token(prec(1, /[tT][eE][xX][tT]/)),

    _kw_then: $ => token(prec(1, /[tT][hH][eE][nN]/)),

    _kw_update: $ => token(prec(1, /[uU][pP][dD][aA][tT][eE]/)),

    _kw_using: $ => token(prec(1, /[uU][sS][iI][nN][gG]/)),

    _kw_values: $ => token(prec(1, /[vV][aA][lL][uU][eE][sS]/)),

    _kw_varchar: $ => token(prec(1, /[vV][aA][rR][cC][hH][aA][rR]/)),

    _kw_when: $ => token(prec(1, /[wW][hH][eE][nN]/)),

    _kw_where: $ => token(prec(1, /[wW][hH][eE][rR][eE]/)),

    _kw_with: $ => token(prec(1, /[wW][iI][tT][hH]/)),

    _kw_without: $ => token(prec(1, /[wW][iI][tT][hH][oO][uU][tT]/)),

  },
});
