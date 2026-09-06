## WIDENING -- tree-sitter accepts more than SDF3 here (5)

- Exp.Eq is non-assoc; ANTLR has no non-associativity, lowered as left-associative
- Exp.Lt is non-assoc; ANTLR has no non-associativity, lowered as left-associative
- Exp.Gt is non-assoc; ANTLR has no non-associativity, lowered as left-associative
- Exp.Like is non-assoc; ANTLR has no non-associativity, lowered as left-associative
- Exp.ILike is non-assoc; ANTLR has no non-associativity, lowered as left-associative

## DEVIATION -- the tree differs in shape from SDF3's AST (2)

- injection into Exp is a context node in ANTLR's tree (`inj_exp_1`); the driver elides it when printing
- LAYOUT goes to the hidden channel: comments are absent from ANTLR's tree, where tree-sitter shows them as extras

## ABSORBED -- nothing emitted, tree-sitter gets the effect another way (1)

- `NAME = keyword {reject}`: parser literals outrank `NAME` in ANTLR's lexer by construction

