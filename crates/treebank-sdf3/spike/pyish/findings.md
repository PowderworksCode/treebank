## WIDENING -- tree-sitter accepts more than SDF3 here (3)

- the scanner applies the offside rule to every element of an aligned list, since it ends an element at a line break by the next line's column alone; [Stmt.Global, Stmt.Pass, Stmt.Print] declare no `offside` and get it anyway
- where no `_newline` can end a statement the scanner is not consulted and a line break is layout, so inside brackets a line may continue at any column: Python's implicit line joining, which the offside rule rejects
- Exp.Lt is non-assoc; tree-sitter has no non-associativity, lowered to PREC_LEFT so `a == b == c` parses where SDF3 rejects it

## DEVIATION -- the tree differs in shape from SDF3's AST (3)

- the outermost aligned list is aligned at column 0, as in CPython, where SDF3 aligns it at its first line's column: a file indented throughout parses its second line as a continuation of its first
- a tab is one column, as tree-sitter's lexer counts; CPython uses tab stops of eight
- bracket production of Exp became the named node `exp_bracket`; SDF3's AST has no node for brackets, but a hidden supertype member may have only one visible child and `( Exp )` has three

## EXTENSION -- a treebank addition outside SDF3 was used (26)

- Stmt.Assign: placeholder label `target` became a field (not SDF3)
- Stmt.Assign: placeholder label `value` became a field (not SDF3)
- Stmt.Return: placeholder label `value` became a field (not SDF3)
- Stmt.Global: placeholder label `names` became a field (not SDF3)
- Stmt.Print: placeholder label `value` became a field (not SDF3)
- Stmt.If: placeholder label `condition` became a field (not SDF3)
- Stmt.If: placeholder label `consequence` became a field (not SDF3)
- Stmt.If: placeholder label `alternative` became a field (not SDF3)
- Stmt.While: placeholder label `condition` became a field (not SDF3)
- Stmt.While: placeholder label `body` became a field (not SDF3)
- Stmt.Def: placeholder label `name` became a field (not SDF3)
- Stmt.Def: placeholder label `parameters` became a field (not SDF3)
- Stmt.Def: placeholder label `body` became a field (not SDF3)
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

## ABSORBED -- nothing emitted, tree-sitter gets the effect another way (4)

- `keyword -/- [class]`: tree-sitter's keyword extraction already refuses to lex a keyword that is a prefix of a longer word
- lexical restriction on ID: longest-match tokenisation gives the same effect
- lexical restriction on INT: longest-match tokenisation gives the same effect
- context-free restriction on LAYOUT?: extras are skipped greedily

## MAPPED -- lowered exactly (30)

- Program.Program: `align-list 1`: every Stmt in the list starts a line at the list's column, so each production of Stmt ends with `_newline` unless an indented block already ends it
- Stmt.Assign: `offside 1 2 3`: a line break followed by a deeper line continues the statement and one at the open column ends it; the scanner decides by the next line's column
- Stmt.Expr: `offside 1`: a line break followed by a deeper line continues the statement and one at the open column ends it; the scanner decides by the next line's column
- Stmt.Return: `offside 1 2`: a line break followed by a deeper line continues the statement and one at the open column ends it; the scanner decides by the next line's column
- Stmt.If: `indent 1 4`: symbol 4 is wrapped as `_indent .. _dedent`; the scanner opens a block when the next line is deeper than the open column and the parser can accept `_indent` (after the literal ":")
- Stmt.If: `align 1 5`: symbol 5 follows an indented block, so it sits at symbol 1's column by the indent stack; a dedent to a column no open block has is an error
- Stmt.While: `indent 1 4`: symbol 4 is wrapped as `_indent .. _dedent`; the scanner opens a block when the next line is deeper than the open column and the parser can accept `_indent` (after the literal ":")
- Stmt.Def: `indent 1 7`: symbol 7 is wrapped as `_indent .. _dedent`; the scanner opens a block when the next line is deeper than the open column and the parser can accept `_indent` (after the literal ":")
- Else.ElseClause: `indent 1 3`: symbol 3 is wrapped as `_indent .. _dedent`; the scanner opens a block when the next line is deeper than the open column and the parser can accept `_indent` (after the literal ":")
- Block.Block: `align-list 1`: every Stmt in the list starts a line at the list's column, so each production of Stmt ends with `_newline` unless an indented block already ends it
- sort Program has the single constructor Program; collapsed to the named rule `program`
- a `{Elem Sep}+` list expanded to seq/repeat; the expansion has no name in grammar.json
- a `{Elem Sep}*` list expanded to seq/repeat; the expansion has no name in grammar.json
- sort Else has the single constructor ElseClause; collapsed to the named rule `else_clause`
- sort Block has the single constructor Block; collapsed to the named rule `block`
- sort Param has the single constructor Param; collapsed to the named rule `param`
- injection into Exp became a supertype member with no node of its own
- a `{Elem Sep}*` list expanded to seq/repeat; the expansion has no name in grammar.json
- Exp.Call at priority level 5 became PREC at that level
- Exp.Neg at priority level 4 became PREC at that level
- Exp.Mul at priority level 3 became PREC_LEFT at that level
- Exp.Add at priority level 2 became PREC_LEFT at that level
- Exp.Sub at priority level 2 became PREC_LEFT at that level
- Exp.Lt at priority level 1 became PREC_LEFT at that level
- lexical sort ID became the token `id` /[a-zA-Z_](?:[a-zA-Z0-9_])*/
- lexical sort INT became the token `int` /(?:[0-9])+/
- LAYOUT class became an extras pattern /[ \t\n\r]/
- LAYOUT production became the named extra `comment` /#(?:[^\n\r])*/
- `ID = keyword {reject}` became `word: id` plus reserved.global = [def, else, global, if, pass, print, return, while]
- `tokenize: "():,"`: the reader split template literal runs at these characters, so each is its own token

