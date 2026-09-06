## MAPPED -- lowered exactly (14)

- 6 production(s) end with a hidden newline in the tree-sitter lowering, which puts a trailing comment inside them; here their reach for extras runs to the end of their last line, so the trees agree
- scannerless: literals and lexical sorts are matched where the grammar puts them, with LAYOUT skipped before every context-free symbol, as SDF3 defines the language; no token stream exists to disagree with the parser
- 1 comment LAYOUT production(s) are recorded as extras when the layout skipper consumes them, and attached after the parse to the innermost node whose span holds them; tree-sitter attaches an extra to the node being reduced, which differs when a hidden token follows the comment
- ID rejects [def, else, global, if, pass, print, return, while]: the matched text is compared against the list, which is SDF3's reject production exactly
- Program.Program: layout constraints are checked in place on the positions the parser has; no variant, no scanner, no lexer state
- Stmt.Assign: layout constraints are checked in place on the positions the parser has; no variant, no scanner, no lexer state
- Stmt.Expr: layout constraints are checked in place on the positions the parser has; no variant, no scanner, no lexer state
- Stmt.Return: layout constraints are checked in place on the positions the parser has; no variant, no scanner, no lexer state
- Stmt.If: layout constraints are checked in place on the positions the parser has; no variant, no scanner, no lexer state
- Stmt.While: layout constraints are checked in place on the positions the parser has; no variant, no scanner, no lexer state
- Stmt.Def: layout constraints are checked in place on the positions the parser has; no variant, no scanner, no lexer state
- Else.ElseClause: layout constraints are checked in place on the positions the parser has; no variant, no scanner, no lexer state
- Block.Block: layout constraints are checked in place on the positions the parser has; no variant, no scanner, no lexer state
- Exp.Lt is non-assoc, and the precedence loop refuses a second operator of its group after it: `a == b == c` is a syntax error, as SDF3 says, where tree-sitter and ANTLR widened to left

