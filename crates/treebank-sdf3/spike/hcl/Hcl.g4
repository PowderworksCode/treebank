// GENERATED from hcl.sdf3 by treebank-sdf3's ANTLR backend. Python3 target.
grammar Hcl;

config_file
    : H_NL* (declaration H_NL)* H_NL*  declaration? EOF
    ;

declaration
    :     attribute  # inj_decl_1
    |     block  # inj_decl_2
    ;

attribute
    : name_=name H_NL*  '=' H_NL*  value=expression
    ;

block
    : type=name H_NL*  (label+=h_label)* H_NL*  body_=body
    ;

h_label
    :     name  # inj_h_label_3
    |     STRING_LIT  # inj_h_label_4
    ;

body
    :     '{'  H_NL H_NL*  (declaration H_NL)* H_NL*  '}'  # body_alt1
    |     '{' H_NL*  attribute? H_NL*  '}'  # body_alt2
    ;

name
    :     h_identifier_kw  # inj_name_5
    ;

expression
    :     operand=expression H_NL*  '.' H_NL*  name_=name  # get_attr
    |     operand=expression H_NL*  '[' H_NL*  key=expression H_NL*  ']'  # index
    |     operand=expression H_NL*  key=H_LEGACY_KEY  # legacy_index
    |     <assoc=right> operand=expression H_NL*  '.' H_NL*  '*' H_NL*  h_splat_name*  # attr_splat
    |     <assoc=right> operand=expression H_NL*  '[' H_NL*  '*' H_NL*  ']' H_NL*  h_splat_suffix*  # full_splat
    |     operator__h_un_op=h_un_op H_NL*  operand=expression  # unary_expression
    |     left=expression H_NL*  operator__h_bin_op_mul=h_bin_op_mul H_NL*  right=expression  # binary_expression
    |     left=expression H_NL*  operator__h_bin_op_add=h_bin_op_add H_NL*  right=expression  # binary_expression
    |     left=expression H_NL*  operator__h_bin_op_cmp=h_bin_op_cmp H_NL*  right=expression  # binary_expression
    |     left=expression H_NL*  operator__h_bin_op_eq=h_bin_op_eq H_NL*  right=expression  # binary_expression
    |     left=expression H_NL*  operator__h_bin_op_and=h_bin_op_and H_NL*  right=expression  # binary_expression
    |     left=expression H_NL*  operator__h_bin_op_or=h_bin_op_or H_NL*  right=expression  # binary_expression
    |     <assoc=right> condition=expression H_NL*  '?' H_NL*  consequence=expression H_NL*  ':' H_NL*  alternative=expression  # conditional
    |     literal  # inj_exp_6
    |     h_identifier_kw  # inj_exp_7
    |     quoted_template  # inj_exp_8
    |     heredoc_template  # inj_exp_9
    |     tuple  # inj_exp_10
    |     object  # inj_exp_11
    |     for_tuple_expr  # inj_exp_12
    |     for_object_expr  # inj_exp_13
    |     function=function_name H_NL*  '(' H_NL*  arguments? H_NL*  ')'  # function_call
    |     '(' H_NL*  expression H_NL*  ')'  # parenthesized_expression
    ;

h_un_op
    :     '-'  # inj_h_un_op_14
    |     '!'  # inj_h_un_op_15
    ;

h_bin_op_mul
    :     '*'  # inj_h_bin_op_mul_16
    |     '/'  # inj_h_bin_op_mul_17
    |     '%'  # inj_h_bin_op_mul_18
    ;

h_bin_op_add
    :     '+'  # inj_h_bin_op_add_19
    |     '-'  # inj_h_bin_op_add_20
    ;

h_bin_op_cmp
    :     '>'  # inj_h_bin_op_cmp_21
    |     '>='  # inj_h_bin_op_cmp_22
    |     '<'  # inj_h_bin_op_cmp_23
    |     '<='  # inj_h_bin_op_cmp_24
    ;

h_bin_op_eq
    :     '=='  # inj_h_bin_op_eq_25
    |     '!='  # inj_h_bin_op_eq_26
    ;

h_bin_op_and
    :     '&&'  # inj_h_bin_op_and_27
    ;

h_bin_op_or
    :     '||'  # inj_h_bin_op_or_28
    ;

h_splat_name
    :     '.' H_NL*  name_=name  # inj_h_splat_name_29
    ;

h_splat_suffix
    :     '.' H_NL*  name_=name  # inj_h_splat_suffix_30
    |     '[' H_NL*  key=expression H_NL*  ']'  # inj_h_splat_suffix_31
    ;

literal
    :     INTEGER  # inj_literal_32
    |     FLOAT  # inj_literal_33
    |     'true'  # true
    |     'false'  # false
    |     'null'  # null
    ;

function_name
    : name ('::' name)*
    ;

arguments
    : argument (',' argument)* H_NL*  (',' | ellipsis)?
    ;

argument
    :     expression  # inj_argument_34
    ;

ellipsis
    : '...'
    ;

tuple
    : '[' H_NL*  (expression (',' expression)* ','?)? H_NL*  ']'
    ;

object
    : '{'  H_NL? H_NL*  h_obj_elems? H_NL*  '}'
    ;

h_obj_elems
    :     object_elem  (h_obj_sep object_elem)*  h_obj_sep?  # inj_h_obj_elems_35
    ;

h_obj_sep
    :     ','  H_NL?  # inj_h_obj_sep_36
    |     H_NL  # inj_h_obj_sep_37
    ;

object_elem
    : key=expression H_NL*  h_obj_assign H_NL*  value=expression
    ;

h_obj_assign
    :     '='  # inj_h_obj_assign_38
    |     ':'  # inj_h_obj_assign_39
    ;

for_tuple_expr
    : '[' H_NL*  h_for_intro H_NL*  result=expression H_NL*  (condition=for_cond)? H_NL*  ']'
    ;

for_object_expr
    : '{'  H_NL? H_NL*  h_for_intro H_NL*  key=expression H_NL*  '=>' H_NL*  value=expression H_NL*  (grouping=ellipsis)? H_NL*  (condition=for_cond)? H_NL*  '}'
    ;

h_for_intro
    :     'for' H_NL*  binding=name H_NL*  h_for_second? H_NL*  'in' H_NL*  collection=expression H_NL*  ':'  # inj_h_for_intro_40
    ;

h_for_second
    :     ',' H_NL*  binding=name  # inj_h_for_second_41
    ;

for_cond
    : 'if' H_NL*  condition=expression
    ;

template_interpolation
    : h_interp_open H_NL*  expression_=expression H_NL*  h_interp_close
    ;

h_interp_open
    :     '${~'  # inj_h_interp_open_42
    |     '${'  # inj_h_interp_open_43
    ;

h_interp_close
    :     '~}'  # inj_h_interp_close_44
    |     '}'  # inj_h_interp_close_45
    ;

h_dir_open
    :     '%{~'  # inj_h_dir_open_46
    |     '%{'  # inj_h_dir_open_47
    ;

h_dir_close
    :     '~}'  # inj_h_dir_close_48
    |     '}'  # inj_h_dir_close_49
    ;

h_dir_if
    :     h_dir_open H_NL*  'if' H_NL*  condition=expression H_NL*  h_dir_close  # inj_h_dir_if_50
    ;

h_dir_for
    :     h_dir_open H_NL*  'for' H_NL*  binding=name H_NL*  h_for_second? H_NL*  'in' H_NL*  collection=expression H_NL*  h_dir_close  # inj_h_dir_for_51
    ;

h_q_part
    :     template_literal  # inj_h_q_part_52
    |     template_interpolation  # inj_h_q_part_53
    |     template_if  # inj_h_q_part_54
    |     template_for  # inj_h_q_part_55
    ;

h_h_part
    :     h_lit_template_literal  # inj_h_h_part_56
    |     template_interpolation  # inj_h_h_part_57
    |     h_if_template_if  # inj_h_h_part_58
    |     h_for_template_for  # inj_h_h_part_59
    ;

quoted_template
    : QUOTE  h_q_part*  QUOTE
    ;

template_literal
    : (H_QCHUNK | ESCAPE_SEQUENCE)+
    ;

template_if
    : h_dir_if  (consequence=template_body)?  else_clause?  H_DIR_ENDIF
    ;

else_clause
    : H_DIR_ELSE  (alternative=template_body)?
    ;

template_for
    : h_dir_for  (body_=template_body)?  H_DIR_ENDFOR
    ;

template_body
    : h_q_part+
    ;

heredoc_template
    : HEREDOC_START  h_h_part*  HEREDOC_END
    ;

h_lit_template_literal
    : H_HCHUNK+
    ;

h_if_template_if
    : h_dir_if  (consequence=h_body_template_body)?  h_else_else_clause?  H_DIR_ENDIF
    ;

h_else_else_clause
    : H_DIR_ELSE  (alternative=h_body_template_body)?
    ;

h_for_template_for
    : h_dir_for  (body_=h_body_template_body)?  H_DIR_ENDFOR
    ;

h_body_template_body
    : h_h_part+
    ;

h_identifier_kw
    : IDENTIFIER | 'true' | 'false' | 'null' | 'for' | 'in' | 'if'
    ;

H_NL : ( ([\r])? [\n] ((([ \t])* ([\r])? [\n]))* ) ;
fragment DELIM : ([a-zA-Z0-9_\-])+ ;
ESCAPE_SEQUENCE : '\u0003' ;  // kernel-owned: needs a lexer mode
FLOAT : ( ([0-9])+ '.' ([0-9])+ (([eE] ([\-+])? ([0-9])+))? | ([0-9])+ [eE] ([\-+])? ([0-9])+ ) ;
HEREDOC_END : '\u0004' ;  // kernel-owned: needs a lexer mode
HEREDOC_START : '\u0005' ;  // kernel-owned: needs a lexer mode
fragment HEX : [0-9a-fA-F] ;
IDENTIFIER : [a-zA-Z_] ([a-zA-Z0-9_\-])* ;
INTEGER : ([0-9])+ ;
WS1 : [ \t]+ -> channel(HIDDEN) ;
COMMENT3 : ('#' | '//') (~[\n\r])* -> channel(HIDDEN) ;
COMMENT4 : '/*' ((~[*] | ([*])+ ~[*/]))* ([*])+ '/' -> channel(HIDDEN) ;
QUOTE : '\u0006' ;  // kernel-owned: needs a lexer mode
STRING_LIT : '"' ((~["\\\r\n] | '\\' [nrt"\\] | '\\u' HEX HEX HEX HEX | '\\U' HEX HEX HEX HEX HEX HEX HEX HEX))* '"' ;
H_DIR_ELSE : '\u0007' ;  // kernel-owned: needs a lexer mode
H_DIR_ENDFOR : '\u0008' ;  // kernel-owned: needs a lexer mode
H_DIR_ENDIF : '\u0009' ;  // kernel-owned: needs a lexer mode
H_HCHUNK : '\u000a' ;  // kernel-owned: needs a lexer mode
fragment H_HTEXT : ~[$%\n\r] ;
H_LEGACY_KEY : '.' ([0-9])+ ;
H_QCHUNK : '\u000b' ;  // kernel-owned: needs a lexer mode
fragment H_QESC : ( '$${' | '%%{' ) ;
fragment H_QSIGIL : [$%] ;
fragment H_QTEXT : ~["\\$%\n\r] ;
