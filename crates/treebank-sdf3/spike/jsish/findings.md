## WIDENING -- tree-sitter accepts more than SDF3 here (1)

- Exp.Lt is non-assoc; tree-sitter has no non-associativity, lowered to PREC_LEFT so `a == b == c` parses where SDF3 rejects it

## DEVIATION -- the tree differs in shape from SDF3's AST (2)

- bracket production of Exp became the named node `exp_bracket`; SDF3's AST has no node for brackets, but a hidden supertype member may have only one visible child and `( Exp )` has three
- 1 named node(s) outside the vocabulary, ledgered as uncategorised: [int]

## EXTENSION -- a treebank addition outside SDF3 was used (30)

- Stmt.Function: placeholder label `name` became a field (not SDF3)
- Stmt.Function: placeholder label `parameters` became a field (not SDF3)
- Stmt.Function: placeholder label `body` became a field (not SDF3)
- Stmt.Var: placeholder label `name` became a field (not SDF3)
- Stmt.Var: placeholder label `value` became a field (not SDF3)
- Stmt.Let: placeholder label `name` became a field (not SDF3)
- Stmt.Let: placeholder label `value` became a field (not SDF3)
- Stmt.Assign: placeholder label `target` became a field (not SDF3)
- Stmt.Assign: placeholder label `value` became a field (not SDF3)
- Stmt.Print: placeholder label `value` became a field (not SDF3)
- Stmt.Return: placeholder label `value` became a field (not SDF3)
- Stmt.If: placeholder label `condition` became a field (not SDF3)
- Stmt.If: placeholder label `consequence` became a field (not SDF3)
- Stmt.If: placeholder label `alternative` became a field (not SDF3)
- Stmt.While: placeholder label `condition` became a field (not SDF3)
- Stmt.While: placeholder label `body` became a field (not SDF3)
- Else.ElseClause: placeholder label `body` became a field (not SDF3)
- Param.Param: placeholder label `name` became a field (not SDF3)
- Exp.Call: placeholder label `function` became a field (not SDF3)
- Exp.Call: placeholder label `arguments` became a field (not SDF3)
- Exp.Neg: placeholder label `operand` became a field (not SDF3)
- Exp.Mul: placeholder label `left` became a field (not SDF3)
- Exp.Mul: placeholder label `right` became a field (not SDF3)
- Exp.Add: placeholder label `left` became a field (not SDF3)
- Exp.Add: placeholder label `right` became a field (not SDF3)
- Exp.Sub: placeholder label `left` became a field (not SDF3)
- Exp.Sub: placeholder label `right` became a field (not SDF3)
- Exp.Lt: placeholder label `left` became a field (not SDF3)
- Exp.Lt: placeholder label `right` became a field (not SDF3)
- `vocabulary` section (not SDF3): 14 terms of treebank's vocabulary 0.1.0 bound to this module's sorts and constructors

## ABSORBED -- nothing emitted, tree-sitter gets the effect another way (5)

- `keyword -/- [class]`: tree-sitter's keyword extraction already refuses to lex a keyword that is a prefix of a longer word
- lexical restriction on ID: longest-match tokenisation gives the same effect
- lexical restriction on INT: longest-match tokenisation gives the same effect
- context-free restriction on LAYOUT?: extras are skipped greedily

## MAPPED -- lowered exactly (35)

- sort Program has the single constructor Program; collapsed to the named rule `program`
- a `{Elem Sep}*` list expanded to seq/repeat; the expansion has no name in grammar.json
- injection into Stmt became a supertype member with no node of its own
- sort Block has the single constructor Block; collapsed to the named rule `block`
- sort Else has the single constructor ElseClause; collapsed to the named rule `else_clause`
- sort Param has the single constructor Param; collapsed to the named rule `param`
- injection into Exp became a supertype member with no node of its own
- a `{Elem Sep}*` list expanded to seq/repeat; the expansion has no name in grammar.json
- Exp.Call at priority level 5 became PREC at that level
- Exp.Neg at priority level 4 became PREC at that level
- Exp.Mul at priority level 3 became PREC_LEFT at that level
- Exp.Add at priority level 2 became PREC_LEFT at that level
- Exp.Sub at priority level 2 became PREC_LEFT at that level
- Exp.Lt at priority level 1 became PREC_LEFT at that level
- lexical sort ID became the token `id` /[a-zA-Z_$](?:[a-zA-Z0-9_$])*/
- lexical sort INT became the token `int` /(?:[0-9])+/
- LAYOUT class became an extras pattern /[ \t\n\r]/
- LAYOUT production became the named extra `comment` /\/\/(?:[^\n\r])*/
- `ID = keyword {reject}` became `word: id` plus reserved.global = [else, function, if, let, return, var, while]
- `tokenize: "(),;{}"`: the reader split template literal runs at these characters, so each is its own token
- `_statement` is the sort Stmt: its supertype `_stmt` is named `_statement`
- `_expression` is the sort Exp: its supertype `_exp` is named `_expression`
- `_declaration` threaded as a supertype over [function]; every reference to a member now goes through it, and the tree is unchanged
- `_body` threaded as a supertype over [block]; every reference to a member now goes through it, and the tree is unchanged
- `_parameter` threaded as a supertype over [param]; every reference to a member now goes through it, and the tree is unchanged
- `_name` threaded as a supertype over [id]; every reference to a member now goes through it, and the tree is unchanged
- `_literal` threaded as a supertype over [exp_int]; every reference to a member now goes through it, and the tree is unchanged
- `_assignment` threaded as a supertype over [assign]; every reference to a member now goes through it, and the tree is unchanged
- `_invocation` threaded as a supertype over [call]; every reference to a member now goes through it, and the tree is unchanged
- `_branch` threaded as a supertype over [if]; every reference to a member now goes through it, and the tree is unchanged
- `_loop` threaded as a supertype over [while]; every reference to a member now goes through it, and the tree is unchanged
- `_jump` threaded as a supertype over [return]; every reference to a member now goes through it, and the tree is unchanged
- `_clause` is a facet: [else_clause] listed in roles.json
- `_control_flow` threaded as a supertype over [_branch, _loop, _jump]; every reference to a member now goes through it, and the tree is unchanged
- roles.json: 13 of 22 table-tier terms are supertypes, 5 facet(s), 24 named node(s), 1 uncategorised (vocabulary 0.1.0)

