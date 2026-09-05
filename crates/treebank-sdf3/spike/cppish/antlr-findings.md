## DEVIATION -- the tree differs in shape from SDF3's AST (3)

- injection into Type is a context node in ANTLR's tree (`inj_type_1`); the driver elides it when printing
- injection into Exp is a context node in ANTLR's tree (`inj_exp_2`); the driver elides it when printing
- LAYOUT goes to the hidden channel: comments are absent from ANTLR's tree, where tree-sitter shows them as extras

## ABSORBED -- nothing emitted, tree-sitter gets the effect another way (1)

- `ID = keyword {reject}`: parser literals outrank `ID` in ANTLR's lexer by construction

## MAPPED -- lowered exactly (1)

- Type.TemplateId: `{prefer}` became alternative order within `type`; ALL(*) takes the first alternative that can match, so an ambiguity decided in an ancestor rule follows that rule's source order, which this attribute does not reach

