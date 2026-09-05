// GENERATED from rustish.sdf3 by treebank-sdf3's ANTLR backend. Python3 target.
grammar Rustish;

program
    : fn* EOF
    ;

fn
    : 'fn' name=ID '(' (parameters+=param (',' parameters+=param)*)? ')' (ret_=ret)? body=block
    ;

ret
    : '->' 'i64'
    ;

param
    : name=ID ':' 'i64'
    ;

block
    : '{' stmt* (tail=exp)? '}'
    ;

stmt
    :     'let' pattern=ID '=' value=exp ';'  # let
    |     'println!' '(' '"' '{' '}' '"' ',' value=exp ')' ';'  # print
    |     exp ';'  # expr
    |     fn  # inj_stmt_1
    |     block  # inj_stmt_2
    ;

exp
    :     function=exp '(' (arguments+=exp (',' arguments+=exp)*)? ')'  # call
    |     '-' operand=exp  # neg
    |     left=exp '*' right=exp  # mul
    |     left=exp '+' right=exp  # add
    |     left=exp '-' right=exp  # sub
    |     ID  # inj_exp_3
    |     INT  # exp_int
    |     block  # inj_exp_4
    |     '(' exp ')'  # exp_bracket
    ;

ID : [a-zA-Z_] ([a-zA-Z0-9_])* ;
INT : ([0-9])+ ;
WS1 : [ \t\n\r]+ -> channel(HIDDEN) ;
COMMENT2 : '//' (~[\n\r])* -> channel(HIDDEN) ;
