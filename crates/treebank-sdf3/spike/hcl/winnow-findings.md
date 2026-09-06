## EXTENSION -- a treebank addition outside SDF3 was used (1)

- HeredocTemplate.HeredocTemplate: `delimiter(1, 3)` (not SDF3): the DELIM the opener matched is remembered, the symbols between stop where the closer with that word begins a line, and the closer must carry it

## MAPPED -- lowered exactly (5)

- kernel syntax: [ESCAPE_SEQUENCE, HBody, HEREDOC_END, HElse, HFor, HIf, HLit, Interp, QBody, QElse, QFor, QIf, QLit, QUOTE, _DIR_ELSE, _DIR_ENDFOR, _DIR_ENDIF, _DirFor, _DirIf, _DirOpen, _HCHUNK, _HPart, _InterpOpen, _QCHUNK, _QPart] are reached where no layout may precede them, and no layout is skipped there or between a kernel production's symbols -- SDF3's `syntax` section as written, with no scanner to derive
- [_NL] are lexical sorts whose text is LAYOUT: before one, only the layout that is not its own text is skipped, so a line break is a token where the grammar asks for one and layout elsewhere
- scannerless: literals and lexical sorts are matched where the grammar puts them, with LAYOUT skipped before every context-free symbol, as SDF3 defines the language; no token stream exists to disagree with the parser
- 0 comment LAYOUT production(s) are recorded as extras when the layout skipper consumes them, and attached after the parse to the innermost node whose span holds them; tree-sitter attaches an extra to the node being reduced, which differs when a hidden token follows the comment
- Exp.LegacyIndex: layout constraints are checked in place on the positions the parser has; no variant, no scanner, no lexer state

