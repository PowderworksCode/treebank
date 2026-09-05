## WIDENING -- tree-sitter accepts more than SDF3 here (2)

- Exp.Eq is non-assoc; tree-sitter has no non-associativity, lowered to PREC_LEFT so `a == b == c` parses where SDF3 rejects it
- Exp.Lt is non-assoc; tree-sitter has no non-associativity, lowered to PREC_LEFT so `a == b == c` parses where SDF3 rejects it

## DEVIATION -- the tree differs in shape from SDF3's AST (1)

- bracket production of Exp became the named node `exp_bracket`; SDF3's AST has no node for brackets, but a hidden supertype member may have only one visible child and `( Exp )` has three

## EXTENSION -- a treebank addition outside SDF3 was used (30)

- Program.Program: placeholder label `body` became a field (not SDF3)
- Stmt.Let: placeholder label `name` became a field (not SDF3)
- Stmt.Let: placeholder label `value` became a field (not SDF3)
- Stmt.Assign: placeholder label `target` became a field (not SDF3)
- Stmt.Assign: placeholder label `value` became a field (not SDF3)
- Stmt.If: placeholder label `condition` became a field (not SDF3)
- Stmt.If: placeholder label `consequence` became a field (not SDF3)
- Stmt.If: placeholder label `alternative` became a field (not SDF3)
- Stmt.While: placeholder label `condition` became a field (not SDF3)
- Stmt.While: placeholder label `body` became a field (not SDF3)
- Stmt.Fun: placeholder label `name` became a field (not SDF3)
- Stmt.Fun: placeholder label `parameters` became a field (not SDF3)
- Stmt.Fun: placeholder label `body` became a field (not SDF3)
- Stmt.Return: placeholder label `value` became a field (not SDF3)
- Exp.Call: placeholder label `function` became a field (not SDF3)
- Exp.Call: placeholder label `arguments` became a field (not SDF3)
- Exp.Neg: placeholder label `operand` became a field (not SDF3)
- Exp.Not: placeholder label `operand` became a field (not SDF3)
- Exp.Mul: placeholder label `left` became a field (not SDF3)
- Exp.Mul: placeholder label `right` became a field (not SDF3)
- Exp.Div: placeholder label `left` became a field (not SDF3)
- Exp.Div: placeholder label `right` became a field (not SDF3)
- Exp.Add: placeholder label `left` became a field (not SDF3)
- Exp.Add: placeholder label `right` became a field (not SDF3)
- Exp.Sub: placeholder label `left` became a field (not SDF3)
- Exp.Sub: placeholder label `right` became a field (not SDF3)
- Exp.Eq: placeholder label `left` became a field (not SDF3)
- Exp.Eq: placeholder label `right` became a field (not SDF3)
- Exp.Lt: placeholder label `left` became a field (not SDF3)
- Exp.Lt: placeholder label `right` became a field (not SDF3)

## ABSORBED -- nothing emitted, tree-sitter gets the effect another way (5)

- `keyword -/- [class]`: tree-sitter's keyword extraction already refuses to lex a keyword that is a prefix of a longer word
- lexical restriction on ID: longest-match tokenisation gives the same effect
- lexical restriction on INT: longest-match tokenisation gives the same effect
- context-free restriction on LAYOUT?: extras are skipped greedily

## MAPPED -- lowered exactly (18)

- sort Program has the single constructor Program; collapsed to the named rule `program`
- a `{Elem Sep}*` list expanded to seq/repeat; the expansion has no name in grammar.json
- sort Block has the single constructor Block; collapsed to the named rule `block`
- injection into Exp became a supertype member with no node of its own
- a `{Elem Sep}*` list expanded to seq/repeat; the expansion has no name in grammar.json
- Exp.Neg at priority level 4 became PREC at that level
- Exp.Not at priority level 4 became PREC at that level
- Exp.Mul at priority level 3 became PREC_LEFT at that level
- Exp.Div at priority level 3 became PREC_LEFT at that level
- Exp.Add at priority level 2 became PREC_LEFT at that level
- Exp.Sub at priority level 2 became PREC_LEFT at that level
- Exp.Eq at priority level 1 became PREC_LEFT at that level
- Exp.Lt at priority level 1 became PREC_LEFT at that level
- lexical sort ID became the token `id` /[a-zA-Z_](?:[a-zA-Z0-9_])*/
- lexical sort INT became the token `int` /(?:[0-9])+/
- LAYOUT class became an extras pattern /[ \t\n\r]/
- LAYOUT production became the named extra `comment` /\/\/(?:[^\n\r])*/
- `ID = keyword {reject}` became `word: id` plus reserved.global = [else, fun, if, let, return, while]

