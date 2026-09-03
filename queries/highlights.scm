; Syntax highlighting for every Treebank grammar, in one file.
;
; There are no language-specific node names here. Every pattern names a term
; from the shared vocabulary, so this same file highlights bash and C++ and
; Zig — which is the whole claim the vocabulary makes, in the form an editor
; can actually consume.
;
; Two kinds of term appear:
;
;   - A STRUCTURAL term is a real supertype threaded through the productions,
;     so tree-sitter matches it natively by derivation.
;   - A NOMINAL term is a list of node types in the grammar's terms.json. It
;     cannot be a rule, so it is expanded into an alternation of that
;     grammar's own members when the per-grammar file is generated.
;
; This file is the source. The files under crates/treebank-<lang>/queries/
; are GENERATED from it by `treebank queries --write` and checked by
; `treebank queries --check`; edit this one, never those.
;
; Order matters: tree-sitter takes the LAST matching pattern for a node, so
; the specific cases come after the general ones.

; --- the leaves a reader looks at first -----------------------------------

(_comment) @comment

; `_literal` BEFORE `_string`, and the order is the whole of it: the last
; matching pattern wins, and the two terms overlap wherever a language has a
; string literal that is also fully determined by its own text. In YAML they
; overlap completely — `_literal` is every scalar and `_string` is the three
; that are always strings — so with `_literal` last, `"quoted"` matched both
; and rendered as a NUMBER. Every quoted scalar in the language did.
(_literal) @number
(_string) @string

; --- the key of a pair ------------------------------------------------------
;
; A mapping key is not a value and should not be coloured like one. There is
; no vocabulary TERM for a key — `_clause` is the subordinate piece and the
; key is a field on it — so this reaches it through the field, which is also
; what makes it self-limiting: expansion drops the members that have no
; `key`, so a language whose clauses are `elif` and `case` contributes
; nothing here and only the mapping-shaped ones do.

; treebank: only-if _clause
(_clause key: (_) @property)

; --- names ----------------------------------------------------------------
;
; `_identifier` is every name-shaped token, so it is the fallback that later
; patterns narrow.

(_identifier) @variable

; --- types ------------------------------------------------------------------
;
; After `_identifier`, so a name in type position is coloured as a type
; rather than as the variable the broader pattern already claimed. Guarded
; because python declares no `_type` — its annotations are ordinary
; expressions — and YAML is what made the omission visible: a tag is the one
; piece of YAML syntax that says what a node IS, and nothing coloured it.

; treebank: only-if _type
(_type) @type

; --- things that are called ------------------------------------------------
;
; Guarded, because not every language treebank parses computes. YAML has no
; callable and no invocation, and a pattern naming a term its grammar does
; not declare is a QueryError at load time rather than a zero-match, so the
; block is dropped for the grammars that lack the term instead of the file
; failing to compile for them.

; treebank: only-if _callable
(_callable) @function

; treebank: only-if _invocation
(_invocation) @function.call

; --- control flow ----------------------------------------------------------
;
; These capture the whole construct rather than its keyword: the keyword is
; an anonymous token, and an anonymous token has no vocabulary term. A
; per-grammar supplement is where `if` and `while` themselves get coloured.
;
; Guarded for the same reason as the block above: a data language alters no
; sequential execution because it has none.

; treebank: only-if _loop
(_loop) @keyword.repeat

; treebank: only-if _branch
(_branch) @keyword.conditional

; treebank: only-if _jump
(_jump) @keyword.return

; treebank: only-if _directive
; And guarded once more, for a language that computes but never reaches
; outside its own file: HCL's `module` block is a block, and what its
; `source` means is the calling application's business rather than the
; syntax's.
(_directive) @keyword.import
