; Scopes, definitions and references for every Treebank grammar, in one file.
;
; This is the query an editor uses to answer "what does this name refer to" --
; rename, go-to-definition, highlight-references. It is the hardest of the
; standard query files to write, because it is about a language's binding
; structure rather than its surface, and it is normally written once per
; language by someone who knows that language well.
;
; It is written once here because the vocabulary already carries the three
; things it needs. `_scope` is where names live, `_binding` is what introduces
; one, and `_identifier` is what mentions one.
;
; Those three were called universal until JSON arrived, and JSON has none of
; them: no scopes, no bindings, and no identifiers at all, since the thing
; that looks like a name in `{"a": 1}` is a string literal. So every pattern
; here is conditional and stands alone between blank lines, and JSON's
; generated file is the comments with nothing under them — which is the
; correct answer for a language in which no name can refer to anything.
;
; The captures are the conventional ones: an editor that understands
; nvim-treesitter's locals will understand these.
;
; This file is the source. The files under crates/treebank-<lang>/queries/ are
; GENERATED from it by `treebank queries --write`; edit this one, never those.

; --- where names live ------------------------------------------------------
;
; A scope is anything that can contain a binding without leaking it. What
; counts differs per language -- a block, a function body, a module, a `do`
; end -- and each grammar declares its own members.

; treebank: only-if _scope
(_scope) @local.scope

; --- what introduces a name ------------------------------------------------
;
; The capture goes on the NAME, not on the construct: an editor renames the
; identifier, and needs the range of the thing it would rewrite. Members that
; have no `name` field are dropped when this is expanded, which is why a
; language whose bindings are positional loses those patterns rather than
; producing a pattern that cannot match.

; treebank: only-if _callable
(_callable name: (_) @local.definition.function)

; treebank: only-if _binding
(_binding name: (_) @local.definition.var)

; A parameter is a definition, and the one place a caller's name meets the
; callee's. bash has no term for it: a bash function takes `$1`, not a named
; parameter, so this is omitted there rather than dropped from everyone.
; treebank: only-if _parameter
(_parameter name: (_) @local.definition.parameter)

; --- what mentions a name --------------------------------------------------
;
; Every name-shaped token is a reference until something more specific claims
; it. The locals engine resolves each against the definitions in scope, so
; over-capturing here is cheap and under-capturing is not.

; treebank: only-if _identifier
(_identifier) @local.reference
