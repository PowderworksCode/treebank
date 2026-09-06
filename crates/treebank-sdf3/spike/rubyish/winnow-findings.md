## MAPPED -- lowered exactly (8)

- scannerless: literals and lexical sorts are matched where the grammar puts them, with LAYOUT skipped before every context-free symbol, as SDF3 defines the language; no token stream exists to disagree with the parser
- 1 comment LAYOUT production(s) are recorded as extras when the layout skipper consumes them, and attached after the parse to the innermost node whose span holds them; tree-sitter attaches an extra to the node being reduced, which differs when a hidden token follows the comment
- Exp.Command: `{prefer}` became ordered choice within `Exp`; a PEG takes the first alternative that matches, so an ambiguity decided in an ancestor sort follows that sort's source order, which the attribute does not reach
- Exp.Command: layout constraints are checked in place on the positions the parser has; no variant, no scanner, no lexer state
- Exp.Call: layout constraints are checked in place on the positions the parser has; no variant, no scanner, no lexer state
- Exp.Neg: layout constraints are checked in place on the positions the parser has; no variant, no scanner, no lexer state
- Exp.Index: layout constraints are checked in place on the positions the parser has; no variant, no scanner, no lexer state
- Arg.Splat: layout constraints are checked in place on the positions the parser has; no variant, no scanner, no lexer state

