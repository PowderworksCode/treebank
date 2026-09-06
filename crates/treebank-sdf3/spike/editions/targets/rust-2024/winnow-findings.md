## MAPPED -- lowered exactly (4)

- scannerless: literals and lexical sorts are matched where the grammar puts them, with LAYOUT skipped before every context-free symbol, as SDF3 defines the language; no token stream exists to disagree with the parser
- 1 comment LAYOUT production(s) are recorded as extras when the layout skipper consumes them, and attached after the parse to the innermost node whose span holds them; tree-sitter attaches an extra to the node being reduced, which differs when a hidden token follows the comment
- ID rejects [async, await, dyn, try, gen, else, fn, i64, if, let, mut, return, while]: the matched text is compared against the list, which is SDF3's reject production exactly
- Exp.Lt is non-assoc, and the precedence loop refuses a second operator of its group after it: `a == b == c` is a syntax error, as SDF3 says, where tree-sitter and ANTLR widened to left

