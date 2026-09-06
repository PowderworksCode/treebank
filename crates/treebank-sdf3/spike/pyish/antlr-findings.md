## WIDENING -- tree-sitter accepts more than SDF3 here (1)

- Exp.Lt is non-assoc; ANTLR has no non-associativity, lowered as left-associative

## DEVIATION -- the tree differs in shape from SDF3's AST (3)

- the lexer cannot ask the parser whether `_indent` is valid, so a deeper line opens a block only after one of [":"] (the literals before an indented symbol) and continues the statement otherwise; tree-sitter's scanner decides the same question by validity
- injection into Exp is a context node in ANTLR's tree (`inj_exp_1`); the driver elides it when printing
- LAYOUT goes to the hidden channel: comments are absent from ANTLR's tree, where tree-sitter shows them as extras

## ABSORBED -- nothing emitted, tree-sitter gets the effect another way (1)

- `ID = keyword {reject}`: parser literals outrank `ID` in ANTLR's lexer by construction

## MAPPED -- lowered exactly (1)

- indent/align-list/align/offside became `H_NEWLINE`, `H_INDENT` and `H_DEDENT` from an indent stack in the lexer, as CPython's tokenizer keeps one; the parser rules are shaped exactly as the tree-sitter lowering's

