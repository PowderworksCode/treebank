; Syntax highlighting for every Treebank grammar, in one file.
;
; There are no language-specific node names here. Every pattern names a term
; from the shared vocabulary, so this same file highlights bash and C++ and
; Zig — which is the whole claim the vocabulary makes, in the form an editor
; can actually consume.
;
; Two kinds of term appear:
;
;   - A SUPERTYPE is a real rule threaded through the productions, so
;     tree-sitter matches it natively by derivation.
;   - A FACET is a list in the grammar's roles.json. It cannot be a rule, so
;     it is expanded into an alternation of that grammar's own members when
;     the per-grammar file is generated.
;
; This file is the source. The files under crates/treebank-<lang>/queries/
; are GENERATED from it by `treebank queries --write` and checked by
; `treebank queries --check`; edit this one, never those.
;
; Order matters: tree-sitter takes the LAST matching pattern for a node, so
; the specific cases come after the general ones.

; --- the leaves a reader looks at first -----------------------------------

(_comment) @comment
(_string) @string
(_literal) @number

; --- names ----------------------------------------------------------------
;
; `_identifier` is every name-shaped token, so it is the fallback that later
; patterns narrow.

(_identifier) @variable

; --- things that are called ------------------------------------------------

(_callable) @function
(_invocation) @function.call

; --- control flow ----------------------------------------------------------
;
; These capture the whole construct rather than its keyword: the keyword is
; an anonymous token, and an anonymous token has no vocabulary term. A
; per-grammar supplement is where `if` and `while` themselves get coloured.

(_loop) @keyword.repeat
(_branch) @keyword.conditional
(_jump) @keyword.return
(_directive) @keyword.import
