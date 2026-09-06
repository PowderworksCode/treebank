// GENERATED from mysql/5.6.sdf3 by treebank-sdf3's ANTLR backend. Python3 target.
grammar Mysql_5_6;

options { caseInsensitive = true; }

script
    : statement* EOF
    ;

statement
    :     select  ';'  # stmt_select
    |     'INSERT'  (hints+=ignore)*  'INTO'  table=name  '('  columns+=name (',' columns+=name)*  ')'  'VALUES'  '('  values+=expression (',' values+=expression)*  ')'  (upsert=on_duplicate_key)?  ';'  # insert
    |     'UPDATE'  table=name  'SET'  assign (',' assign)*  (where_=where)?  ';'  # update
    |     'DELETE'  'FROM'  table=name  (where_=where)?  ';'  # delete
    |     'CREATE'  'TABLE'  table=name  '('  col_def (',' col_def)*  ')'  ';'  # create_table
    |     'DROP'  'TABLE'  table=name  ';'  # drop_table
    |     'REPLACE'  'INTO'  table=name  '('  columns+=name (',' columns+=name)*  ')'  'VALUES'  '('  values+=expression (',' values+=expression)*  ');'  # replace
    ;

select
    : 'SELECT'  (hints+=select_hint)*  items+=item (',' items+=item)*  (from_=from)?  (where_=where)?  (order_=order_by)?  (limit_=limit)?  (offset_=offset)?
    ;

item
    : expression  (alias_=alias)?
    ;

alias
    :     'AS'  name  # as
    |     name  # bare
    ;

from
    : 'FROM'  table=name
    ;

where
    : 'WHERE'  expression
    ;

order_by
    : 'ORDER'  'BY'  order (',' order)*
    ;

order
    : expression  (dir_=dir)?
    ;

dir
    :     'ASC'  # asc
    |     'DESC'  # desc
    ;

cte
    : name_=name  'AS'  '('  select  ')'
    ;

assign
    : column=name  '='  value=expression
    ;

col_def
    : name_=name  type
    ;

type
    :     'INT'  # type_int
    |     'VARCHAR'  '('  INT  ')'  # varchar
    |     'TEXT'  # text
    ;

name
    :     NAME  # ident_name
    |     BACKTICK  # quoted
    ;

expression
    :     function=NAME  '('  (arguments+=expression (',' arguments+=expression)*)?  ')'  # call
    |     '-'  expression  # neg
    |     left=expression  '*'  right=expression  # mul
    |     left=expression  '+'  right=expression  # add
    |     left=expression  '-'  right=expression  # sub
    |     left=expression  '='  right=expression  # eq
    |     left=expression  '<'  right=expression  # lt
    |     left=expression  '>'  right=expression  # gt
    |     left=expression  'LIKE'  right=expression  # like
    |     'NOT'  expression  # not
    |     left=expression  'AND'  right=expression  # and
    |     left=expression  'OR'  right=expression  # or
    |     name  # inj_exp_1
    |     table=name  '.'  column=name  # column
    |     '*'  # star
    |     INT  # exp_int
    |     STRING  # str
    |     'NULL'  # null
    |     '('  expression  ')'  # exp_bracket
    ;

limit
    : 'LIMIT'  count=INT
    ;

offset
    : 'OFFSET'  start=INT
    ;

ignore
    : 'IGNORE'
    ;

on_duplicate_key
    : 'ON'  'DUPLICATE'  'KEY'  'UPDATE'  assign (',' assign)*
    ;

select_hint
    :     'SQL_CACHE'  # cache
    |     'SQL_NO_CACHE'  # no_cache
    ;

BACKTICK : '`' (~[`])* '`' ;
fragment DQSTRING : '"' (~["])* '"' ;
INT : ([0-9])+ ;
WS1 : [ \t\n\r]+ -> channel(HIDDEN) ;
COMMENT2 : '--' (~[\n\r])* -> channel(HIDDEN) ;
COMMENT3 : '#' (~[\n\r])* -> channel(HIDDEN) ;
NAME : [a-zA-Z_] ([a-zA-Z0-9_])* ;
STRING : ( '\'' (('\'\'' | ~[']))* '\'' | DQSTRING ) ;
