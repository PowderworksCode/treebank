## UNSUPPORTED -- the grammar is missing something (1)

- kernel syntax reaches [ESCAPE_SEQUENCE, HEREDOC_END, HEREDOC_START, QUOTE, _DIR_ELSE, _DIR_ENDFOR, _DIR_ENDIF, _HCHUNK, _QCHUNK] where no layout may precede them; tree-sitter's scanner lexes them in a mode of their own, and ANTLR would need lexer modes, which this lowering does not derive. Their tokens are declared unmatchable so the grammar compiles, and every construct that needs them is a parse error here

## WIDENING -- tree-sitter accepts more than SDF3 here (1)

- through `h_identifier_kw` a keyword is an identifier wherever an identifier is admitted, even where SDF3's `{prefer}` would take the keyword reading: `[for, a]` parses as a tuple here and is rejected by tree-sitter's keyword extraction

## DEVIATION -- the tree differs in shape from SDF3's AST (60)

- injection into Decl is a context node in ANTLR's tree (`inj_decl_1`); the driver elides it when printing
- injection into Decl is a context node in ANTLR's tree (`inj_decl_2`); the driver elides it when printing
- injection into _Label is a context node in ANTLR's tree (`inj_h_label_3`); the driver elides it when printing
- injection into _Label is a context node in ANTLR's tree (`inj_h_label_4`); the driver elides it when printing
- injection into Name is a context node in ANTLR's tree (`inj_name_5`); the driver elides it when printing
- injection into Exp is a context node in ANTLR's tree (`inj_exp_6`); the driver elides it when printing
- injection into Exp is a context node in ANTLR's tree (`inj_exp_7`); the driver elides it when printing
- injection into Exp is a context node in ANTLR's tree (`inj_exp_8`); the driver elides it when printing
- injection into Exp is a context node in ANTLR's tree (`inj_exp_9`); the driver elides it when printing
- injection into Exp is a context node in ANTLR's tree (`inj_exp_10`); the driver elides it when printing
- injection into Exp is a context node in ANTLR's tree (`inj_exp_11`); the driver elides it when printing
- injection into Exp is a context node in ANTLR's tree (`inj_exp_12`); the driver elides it when printing
- injection into Exp is a context node in ANTLR's tree (`inj_exp_13`); the driver elides it when printing
- injection into _UnOp is a context node in ANTLR's tree (`inj_h_un_op_14`); the driver elides it when printing
- injection into _UnOp is a context node in ANTLR's tree (`inj_h_un_op_15`); the driver elides it when printing
- injection into _BinOpMul is a context node in ANTLR's tree (`inj_h_bin_op_mul_16`); the driver elides it when printing
- injection into _BinOpMul is a context node in ANTLR's tree (`inj_h_bin_op_mul_17`); the driver elides it when printing
- injection into _BinOpMul is a context node in ANTLR's tree (`inj_h_bin_op_mul_18`); the driver elides it when printing
- injection into _BinOpAdd is a context node in ANTLR's tree (`inj_h_bin_op_add_19`); the driver elides it when printing
- injection into _BinOpAdd is a context node in ANTLR's tree (`inj_h_bin_op_add_20`); the driver elides it when printing
- injection into _BinOpCmp is a context node in ANTLR's tree (`inj_h_bin_op_cmp_21`); the driver elides it when printing
- injection into _BinOpCmp is a context node in ANTLR's tree (`inj_h_bin_op_cmp_22`); the driver elides it when printing
- injection into _BinOpCmp is a context node in ANTLR's tree (`inj_h_bin_op_cmp_23`); the driver elides it when printing
- injection into _BinOpCmp is a context node in ANTLR's tree (`inj_h_bin_op_cmp_24`); the driver elides it when printing
- injection into _BinOpEq is a context node in ANTLR's tree (`inj_h_bin_op_eq_25`); the driver elides it when printing
- injection into _BinOpEq is a context node in ANTLR's tree (`inj_h_bin_op_eq_26`); the driver elides it when printing
- injection into _BinOpAnd is a context node in ANTLR's tree (`inj_h_bin_op_and_27`); the driver elides it when printing
- injection into _BinOpOr is a context node in ANTLR's tree (`inj_h_bin_op_or_28`); the driver elides it when printing
- injection into _SplatName is a context node in ANTLR's tree (`inj_h_splat_name_29`); the driver elides it when printing
- injection into _SplatSuffix is a context node in ANTLR's tree (`inj_h_splat_suffix_30`); the driver elides it when printing
- injection into _SplatSuffix is a context node in ANTLR's tree (`inj_h_splat_suffix_31`); the driver elides it when printing
- injection into Literal is a context node in ANTLR's tree (`inj_literal_32`); the driver elides it when printing
- injection into Literal is a context node in ANTLR's tree (`inj_literal_33`); the driver elides it when printing
- injection into Argument is a context node in ANTLR's tree (`inj_argument_34`); the driver elides it when printing
- injection into _ObjElems is a context node in ANTLR's tree (`inj_h_obj_elems_35`); the driver elides it when printing
- injection into _ObjSep is a context node in ANTLR's tree (`inj_h_obj_sep_36`); the driver elides it when printing
- injection into _ObjSep is a context node in ANTLR's tree (`inj_h_obj_sep_37`); the driver elides it when printing
- injection into _ObjAssign is a context node in ANTLR's tree (`inj_h_obj_assign_38`); the driver elides it when printing
- injection into _ObjAssign is a context node in ANTLR's tree (`inj_h_obj_assign_39`); the driver elides it when printing
- injection into _ForIntro is a context node in ANTLR's tree (`inj_h_for_intro_40`); the driver elides it when printing
- injection into _ForSecond is a context node in ANTLR's tree (`inj_h_for_second_41`); the driver elides it when printing
- injection into _InterpOpen is a context node in ANTLR's tree (`inj_h_interp_open_42`); the driver elides it when printing
- injection into _InterpOpen is a context node in ANTLR's tree (`inj_h_interp_open_43`); the driver elides it when printing
- injection into _InterpClose is a context node in ANTLR's tree (`inj_h_interp_close_44`); the driver elides it when printing
- injection into _InterpClose is a context node in ANTLR's tree (`inj_h_interp_close_45`); the driver elides it when printing
- injection into _DirOpen is a context node in ANTLR's tree (`inj_h_dir_open_46`); the driver elides it when printing
- injection into _DirOpen is a context node in ANTLR's tree (`inj_h_dir_open_47`); the driver elides it when printing
- injection into _DirClose is a context node in ANTLR's tree (`inj_h_dir_close_48`); the driver elides it when printing
- injection into _DirClose is a context node in ANTLR's tree (`inj_h_dir_close_49`); the driver elides it when printing
- injection into _DirIf is a context node in ANTLR's tree (`inj_h_dir_if_50`); the driver elides it when printing
- injection into _DirFor is a context node in ANTLR's tree (`inj_h_dir_for_51`); the driver elides it when printing
- injection into _QPart is a context node in ANTLR's tree (`inj_h_q_part_52`); the driver elides it when printing
- injection into _QPart is a context node in ANTLR's tree (`inj_h_q_part_53`); the driver elides it when printing
- injection into _QPart is a context node in ANTLR's tree (`inj_h_q_part_54`); the driver elides it when printing
- injection into _QPart is a context node in ANTLR's tree (`inj_h_q_part_55`); the driver elides it when printing
- injection into _HPart is a context node in ANTLR's tree (`inj_h_h_part_56`); the driver elides it when printing
- injection into _HPart is a context node in ANTLR's tree (`inj_h_h_part_57`); the driver elides it when printing
- injection into _HPart is a context node in ANTLR's tree (`inj_h_h_part_58`); the driver elides it when printing
- injection into _HPart is a context node in ANTLR's tree (`inj_h_h_part_59`); the driver elides it when printing
- LAYOUT goes to the hidden channel: comments are absent from ANTLR's tree, where tree-sitter shows them as extras

## MAPPED -- lowered exactly (3)

- `IDENTIFIER = keyword {prefer}`: every IDENTIFIER position goes through `h_identifier_kw`, which admits the 6 keyword literals as well, since ANTLR's lexer gives a literal its own token everywhere; where both readings are viable ALL(*) takes the earlier alternative, the keyword's where its production precedes the identifier's, and where they are alternatives of different rules the outer rule's order decides
- lexical sort _NL's text is LAYOUT: it is the token `H_NL`, its first characters are no longer whitespace, and `H_NL*` stands at every position where layout is admitted -- SDF3's `LAYOUT?` between context-free symbols, made explicit for the one kind of layout that is also a token
- lexical sorts referenced by lexical syntax only became `fragment` rules: [DELIM, HEX, _HTEXT, _QESC, _QSIGIL, _QTEXT]

