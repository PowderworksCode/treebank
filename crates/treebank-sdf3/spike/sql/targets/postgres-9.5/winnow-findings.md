## MAPPED -- lowered exactly (9)

- scannerless: literals and lexical sorts are matched where the grammar puts them, with LAYOUT skipped before every context-free symbol, as SDF3 defines the language; no token stream exists to disagree with the parser
- `keyword = case-insensitive` became `Caseless` on every word-shaped literal, and the keyword rejection compares case-insensitively
- 1 comment LAYOUT production(s) are recorded as extras when the layout skipper consumes them, and attached after the parse to the innermost node whose span holds them; tree-sitter attaches an extra to the node being reduced, which differs when a hidden token follows the comment
- NAME rejects [AND, AS, ASC, BY, CONFLICT, CREATE, DELETE, DESC, DO, DROP, FROM, ILIKE, INSERT, INT, INTO, LIKE, LIMIT, NOT, NOTHING, NULL, OFFSET, OIDS, ON, OR, ORDER, OVER, PARTITION, RETURNING, SELECT, SET, TABLE, TEXT, UPDATE, VALUES, VARCHAR, WHERE, WITH, WITHOUT]: the matched text is compared against the list, which is SDF3's reject production exactly
- Exp.Eq is non-assoc, and the precedence loop refuses a second operator of its group after it: `a == b == c` is a syntax error, as SDF3 says, where tree-sitter and ANTLR widened to left
- Exp.Lt is non-assoc, and the precedence loop refuses a second operator of its group after it: `a == b == c` is a syntax error, as SDF3 says, where tree-sitter and ANTLR widened to left
- Exp.Gt is non-assoc, and the precedence loop refuses a second operator of its group after it: `a == b == c` is a syntax error, as SDF3 says, where tree-sitter and ANTLR widened to left
- Exp.Like is non-assoc, and the precedence loop refuses a second operator of its group after it: `a == b == c` is a syntax error, as SDF3 says, where tree-sitter and ANTLR widened to left
- Exp.ILike is non-assoc, and the precedence loop refuses a second operator of its group after it: `a == b == c` is a syntax error, as SDF3 says, where tree-sitter and ANTLR widened to left

