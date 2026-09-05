## DEVIATION -- the tree differs in shape from SDF3's AST (1)

- bracket production of Exp became the named node `exp_bracket`; SDF3's AST has no node for brackets, but a hidden supertype member may have only one visible child and `( Exp )` has three

## EXTENSION -- a treebank addition outside SDF3 was used (19)

- Stmt.Assign: placeholder label `target` became a field (not SDF3)
- Stmt.Assign: placeholder label `value` became a field (not SDF3)
- Exp.Array: placeholder label `elements` became a field (not SDF3)
- Exp.Index: placeholder label `receiver` became a field (not SDF3)
- Exp.Index: placeholder label `index` became a field (not SDF3)
- Exp.Call: placeholder label `method` became a field (not SDF3)
- Exp.Call: placeholder label `arguments` became a field (not SDF3)
- Exp.Command: placeholder label `method` became a field (not SDF3)
- Exp.Command: placeholder label `argument` became a field (not SDF3)
- Exp.Neg: placeholder label `operand` became a field (not SDF3)
- Exp.Mul: placeholder label `left` became a field (not SDF3)
- Exp.Mul: placeholder label `right` became a field (not SDF3)
- Exp.Div: placeholder label `left` became a field (not SDF3)
- Exp.Div: placeholder label `right` became a field (not SDF3)
- Exp.Add: placeholder label `left` became a field (not SDF3)
- Exp.Add: placeholder label `right` became a field (not SDF3)
- Exp.Sub: placeholder label `left` became a field (not SDF3)
- Exp.Sub: placeholder label `right` became a field (not SDF3)
- Arg.Splat: placeholder label `operand` became a field (not SDF3)

## ABSORBED -- nothing emitted, tree-sitter gets the effect another way (2)

- lexical restriction on ID: longest-match tokenisation gives the same effect
- lexical restriction on INT: longest-match tokenisation gives the same effect

## MAPPED -- lowered exactly (30)

- Exp.Index: symbols 1 and 2 adjacent: layout forbidden before the literal at 2
- Exp.Call: symbols 1 and 2 adjacent: layout forbidden before the literal at 2
- Exp.Command: symbols 1 and 2 separated: layout required before the first token of Arg, propagated to lexical REGEX, Exp.Array, Exp, Exp.Neg, Arg.Splat
- Exp.Neg: symbols 1 and 2 adjacent: layout forbidden after the literal at 1
- Arg.Splat: symbols 1 and 2 adjacent: layout forbidden after the literal at 1
- the spelling "(" is split into scanner-owned variants [_lparen_spaced, _lparen_adjacent]; each is aliased back to "(" in the tree
- the spelling "*" is split into scanner-owned variants [_star_spaced_tight, _star]; each is aliased back to "*" in the tree
- the spelling "-" is split into scanner-owned variants [_minus_spaced_tight, _minus]; each is aliased back to "-" in the tree
- the spelling "[" is split into scanner-owned variants [_lbracket_spaced, _lbracket_adjacent]; each is aliased back to "[" in the tree
- lexical sort REGEX opens with '/', a split spelling, so the scanner scans it whole as the named token `regex` (layout required before, forbidden after)
- sort Program has the single constructor Program; collapsed to the named rule `program`
- injection into Exp became a supertype member with no node of its own
- a `{Elem Sep}*` list expanded to seq/repeat; the expansion has no name in grammar.json
- Exp.Index at priority level 5 became PREC at that level
- a `{Elem Sep}*` list expanded to seq/repeat; the expansion has no name in grammar.json
- Exp.Call at priority level 5 became PREC at that level
- Exp.Command at priority level 1 became PREC at that level
- Exp.Command: `{prefer}` became dynamic precedence +1; it decides only where a conflict is declared
- Exp.Neg at priority level 4 became PREC at that level
- Exp.Mul at priority level 3 became PREC_LEFT at that level
- Exp.Div at priority level 3 became PREC_LEFT at that level
- Exp.Add at priority level 2 became PREC_LEFT at that level
- Exp.Sub at priority level 2 became PREC_LEFT at that level
- injection into Arg became a supertype member with no node of its own
- lexical sort ID became the token `id` /[a-z_](?:[a-zA-Z0-9_])*/
- lexical sort INT became the token `int` /(?:[0-9])+/
- LAYOUT class became an extras pattern /[ \t]/
- LAYOUT production became the named extra `comment` /#(?:[^\n])*/
- lexical sort NL became the token `nl` /[\n](?:(?:[ \t\n]|#(?:[^\n])*))*/
- lexical sort REGEX is scanned by the generated scanner as `regex`; no token rule emitted

