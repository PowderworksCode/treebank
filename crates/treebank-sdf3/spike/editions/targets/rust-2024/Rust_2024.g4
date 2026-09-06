// GENERATED from rust/2024.sdf3 by treebank-sdf3's ANTLR backend. Python3 target.
grammar Rust_2024;

program
    : fn* EOF
    ;

fn
    : 'fn'  name=ID  '('  (parameters+=param (',' parameters+=param)*)?  ')'  (ret_=ret)?  body=block
    ;

ret
    : '->'  'i64'
    ;

param
    : name=ID  ':'  'i64'
    ;

block
    : '{'  statement*  (tail=expression)?  '}'
    ;

statement
    :     'let'  pattern=ID  '='  value=expression  ';'  # let
    |     'let'  'mut'  pattern=ID  '='  value=expression  ';'  # let_mut
    |     target=ID  '='  value=expression  ';'  # assign
    |     'if'  condition=expression  consequence=block  (alternative=else_clause)?  # if
    |     'while'  condition=expression  body=block  # while
    |     'return'  value=expression  ';'  # return
    |     'println!'  '('  '"'  '{'  '}'  '"'  ','  value=expression  ')'  ';'  # print
    |     expression  ';'  # expr
    |     fn  # inj_stmt_1
    |     block  # inj_stmt_2
    ;

else_clause
    : 'else'  body=block
    ;

expression
    :     function=expression  '('  (arguments+=expression (',' arguments+=expression)*)?  ')'  # call
    |     '-'  operand=expression  # neg
    |     left=expression  '*'  right=expression  # mul
    |     left=expression  '+'  right=expression  # add
    |     left=expression  '-'  right=expression  # sub
    |     left=expression  '<'  right=expression  # lt
    |     ID  # inj_exp_3
    |     INT  # exp_int
    |     block  # inj_exp_4
    |     '('  expression  ')'  # exp_bracket
    ;

ID : [a-zA-Z_] ([a-zA-Z0-9_])* ;
INT : ([0-9])+ ;
WS1 : [ \t\n\r]+ -> channel(HIDDEN) ;
COMMENT2 : '//' (~[\n\r])* -> channel(HIDDEN) ;
