// GENERATED from cppish.sdf3 by treebank-sdf3's ANTLR backend. Python3 target.
grammar Cppish;

program
    : stmt* EOF
    ;

stmt
    :     type_=type  name=ID  ';'  # decl
    |     target=ID  '='  value=exp  ';'  # assign
    |     exp  ';'  # expr_stmt
    ;

type
    :     name=ID  '<'  arguments+=type (',' arguments+=type)*  '>'  # template_id
    |     'int'  # int_type
    |     'char'  # char_type
    |     ID  # inj_type_1
    ;

exp
    :     left=exp  '+'  right=exp  # add
    |     left=exp  '>>'  right=exp  # shr
    |     left=exp  '<'  right=exp  # lt
    |     left=exp  '>'  right=exp  # gt
    |     ID  # inj_exp_2
    |     NUM  # exp_num
    |     function=ID  '('  (arguments+=exp (',' arguments+=exp)*)?  ')'  # call
    ;

ID : [a-zA-Z_] ([a-zA-Z0-9_])* ;
WS1 : [ \t\n\r]+ -> channel(HIDDEN) ;
COMMENT2 : '//' (~[\n\r])* -> channel(HIDDEN) ;
NUM : ([0-9])+ ;
