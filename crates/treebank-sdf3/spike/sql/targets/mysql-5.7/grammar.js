// GENERATED from an SDF3 module by treebank-sdf3; grammar.json is the
// artifact the parser is generated from, this file is for reading.

module.exports = grammar({
  name: "mysql_5_7",
  word: $ => $.name,
  extras: $ => [/[ \t\n\r]/, $.comment, $.comment_2],
  supertypes: $ => [$._statement, $._type, $._name, $._expression, $._literal, $._invocation, $._declaration, $._assignment, $._modifier],
  reserved: { global: $ => [$._kw_and, $._kw_as, $._kw_asc, $._kw_by, $._kw_create, $._kw_delete, $._kw_desc, $._kw_drop, $._kw_duplicate, $._kw_from, $._kw_ignore, $._kw_insert, $._kw_int, $._kw_into, $._kw_key, $._kw_like, $._kw_limit, $._kw_not, $._kw_null, $._kw_offset, $._kw_on, $._kw_or, $._kw_order, $._kw_replace, $._kw_select, $._kw_set, $._kw_sql_cache, $._kw_sql_no_cache, $._kw_table, $._kw_text, $._kw_update, $._kw_values, $._kw_varchar, $._kw_where] },
  rules: {
    script: $ => repeat($._statement),

    stmt_select: $ => seq(
      $.select,
      ";"
    ),

    insert: $ => seq(
      alias($._kw_insert, "INSERT"),
      field("hints", repeat($._modifier)),
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
      field("upsert", optional($.on_duplicate_key)),
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
      ";"
    ),

    delete: $ => seq(
      alias($._kw_delete, "DELETE"),
      alias($._kw_from, "FROM"),
      field("table", $._name),
      field("where", optional($.where)),
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

    replace: $ => seq(
      alias($._kw_replace, "REPLACE"),
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
      ");"
    ),

    _statement: $ => choice(
      $.stmt_select,
      $.insert,
      $.update,
      $.delete,
      $._declaration,
      $.drop_table,
      $.replace
    ),

    select: $ => seq(
      alias($._kw_select, "SELECT"),
      field("hints", repeat($._modifier)),
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

    quoted: $ => $.backtick,

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

    call: $ => prec(10, seq(
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

    neg: $ => prec(9, seq(
      "-",
      $._expression
    )),

    mul: $ => prec.left(8, seq(
      field("left", $._expression),
      "*",
      field("right", $._expression)
    )),

    add: $ => prec.left(7, seq(
      field("left", $._expression),
      "+",
      field("right", $._expression)
    )),

    sub: $ => prec.left(7, seq(
      field("left", $._expression),
      "-",
      field("right", $._expression)
    )),

    eq: $ => prec.left(6, seq(
      field("left", $._expression),
      "=",
      field("right", $._expression)
    )),

    lt: $ => prec.left(6, seq(
      field("left", $._expression),
      "<",
      field("right", $._expression)
    )),

    gt: $ => prec.left(6, seq(
      field("left", $._expression),
      ">",
      field("right", $._expression)
    )),

    like: $ => prec.left(6, seq(
      field("left", $._expression),
      alias($._kw_like, "LIKE"),
      field("right", $._expression)
    )),

    not: $ => prec(5, seq(
      alias($._kw_not, "NOT"),
      $._expression
    )),

    and: $ => prec.left(4, seq(
      field("left", $._expression),
      alias($._kw_and, "AND"),
      field("right", $._expression)
    )),

    or: $ => prec.left(3, seq(
      field("left", $._expression),
      alias($._kw_or, "OR"),
      field("right", $._expression)
    )),

    exp_bracket: $ => seq(
      "(",
      $._expression,
      ")"
    ),

    arrow: $ => prec.left(2, seq(
      field("left", $._expression),
      "->",
      field("right", $._expression)
    )),

    arrow_text: $ => prec.left(2, seq(
      field("left", $._expression),
      "->>",
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
      $.arrow,
      $.arrow_text
    ),

    limit: $ => seq(
      alias($._kw_limit, "LIMIT"),
      field("count", $.int)
    ),

    offset: $ => seq(
      alias($._kw_offset, "OFFSET"),
      field("start", $.int)
    ),

    ignore: $ => alias($._kw_ignore, "IGNORE"),

    on_duplicate_key: $ => seq(
      alias($._kw_on, "ON"),
      alias($._kw_duplicate, "DUPLICATE"),
      alias($._kw_key, "KEY"),
      alias($._kw_update, "UPDATE"),
      seq(
        $._assignment,
        repeat(seq(
          ",",
          $._assignment
        ))
      )
    ),

    cache: $ => alias($._kw_sql_cache, "SQL_CACHE"),

    no_cache: $ => alias($._kw_sql_no_cache, "SQL_NO_CACHE"),

    _modifier: $ => choice(
      $.asc,
      $.desc,
      $.cache,
      $.no_cache,
      $.ignore
    ),

    backtick: $ => /`(?:[^`])*`/,

    int: $ => /(?:[0-9])+/,

    comment: $ => /--(?:[^\n\r])*/,

    comment_2: $ => /#(?:[^\n\r])*/,

    name: $ => /[a-zA-Z_](?:[a-zA-Z0-9_])*/,

    string: $ => /(?:'(?:(?:''|[^']))*'|(?:"(?:[^"])*"))/,

    _kw_and: $ => token(prec(1, /[aA][nN][dD]/)),

    _kw_as: $ => token(prec(1, /[aA][sS]/)),

    _kw_asc: $ => token(prec(1, /[aA][sS][cC]/)),

    _kw_by: $ => token(prec(1, /[bB][yY]/)),

    _kw_create: $ => token(prec(1, /[cC][rR][eE][aA][tT][eE]/)),

    _kw_delete: $ => token(prec(1, /[dD][eE][lL][eE][tT][eE]/)),

    _kw_desc: $ => token(prec(1, /[dD][eE][sS][cC]/)),

    _kw_drop: $ => token(prec(1, /[dD][rR][oO][pP]/)),

    _kw_duplicate: $ => token(prec(1, /[dD][uU][pP][lL][iI][cC][aA][tT][eE]/)),

    _kw_from: $ => token(prec(1, /[fF][rR][oO][mM]/)),

    _kw_ignore: $ => token(prec(1, /[iI][gG][nN][oO][rR][eE]/)),

    _kw_insert: $ => token(prec(1, /[iI][nN][sS][eE][rR][tT]/)),

    _kw_int: $ => token(prec(1, /[iI][nN][tT]/)),

    _kw_into: $ => token(prec(1, /[iI][nN][tT][oO]/)),

    _kw_key: $ => token(prec(1, /[kK][eE][yY]/)),

    _kw_like: $ => token(prec(1, /[lL][iI][kK][eE]/)),

    _kw_limit: $ => token(prec(1, /[lL][iI][mM][iI][tT]/)),

    _kw_not: $ => token(prec(1, /[nN][oO][tT]/)),

    _kw_null: $ => token(prec(1, /[nN][uU][lL][lL]/)),

    _kw_offset: $ => token(prec(1, /[oO][fF][fF][sS][eE][tT]/)),

    _kw_on: $ => token(prec(1, /[oO][nN]/)),

    _kw_or: $ => token(prec(1, /[oO][rR]/)),

    _kw_order: $ => token(prec(1, /[oO][rR][dD][eE][rR]/)),

    _kw_replace: $ => token(prec(1, /[rR][eE][pP][lL][aA][cC][eE]/)),

    _kw_select: $ => token(prec(1, /[sS][eE][lL][eE][cC][tT]/)),

    _kw_set: $ => token(prec(1, /[sS][eE][tT]/)),

    _kw_sql_cache: $ => token(prec(1, /[sS][qQ][lL]_[cC][aA][cC][hH][eE]/)),

    _kw_sql_no_cache: $ => token(prec(1, /[sS][qQ][lL]_[nN][oO]_[cC][aA][cC][hH][eE]/)),

    _kw_table: $ => token(prec(1, /[tT][aA][bB][lL][eE]/)),

    _kw_text: $ => token(prec(1, /[tT][eE][xX][tT]/)),

    _kw_update: $ => token(prec(1, /[uU][pP][dD][aA][tT][eE]/)),

    _kw_values: $ => token(prec(1, /[vV][aA][lL][uU][eE][sS]/)),

    _kw_varchar: $ => token(prec(1, /[vV][aA][rR][cC][hH][aA][rR]/)),

    _kw_where: $ => token(prec(1, /[wW][hH][eE][rR][eE]/)),

  },
});
