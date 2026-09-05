## DEVIATION -- the tree differs in shape from SDF3's AST (3)

- injection into Stmt is a context node in ANTLR's tree (`inj_stmt_1`); the driver elides it when printing
- injection into Exp is a context node in ANTLR's tree (`inj_exp_2`); the driver elides it when printing
- LAYOUT goes to the hidden channel: comments are absent from ANTLR's tree, where tree-sitter shows them as extras

## ABSORBED -- nothing emitted, tree-sitter gets the effect another way (1)

- `ID = keyword {reject}`: parser literals outrank `ID` in ANTLR's lexer by construction

