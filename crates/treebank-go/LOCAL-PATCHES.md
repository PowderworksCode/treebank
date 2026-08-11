# Local patches — tree-sitter-go

Upstream: [tree-sitter/tree-sitter-go](https://github.com/tree-sitter/tree-sitter-go)
pinned at `1547678a9da59885853f5f5cc8a99cc203fa2e2c` (v0.25.0).
`ledger.json` is the machine-readable version of this file, with the sweep
evidence for each patch.

tree-sitter-go has **no external scanner** — it is pure `grammar.js`. Every
fix here is therefore a declarative grammar change, not a C state machine.

## 0001 — treebank redistribution notice (packaging)

Prepends the standard warning to `README.md`: this tree is an automatically
generated, patched redistribution maintained by
[treebank](https://treebank.dev), not the upstream project. Applies first and
touches no grammar code.

## 0002 — treebank crate identity (packaging)

Upstream owns `tree-sitter-go` on crates.io, so the redistribution publishes
as `treebank-grammar-go` with its own `repository`, `homepage` and
`description`. `[lib] name` stays `tree_sitter_go` so the crate is a drop-in
replacement. `include` gains `ledger.json`, `LOCAL-PATCHES.md` and `patches/*`
so provenance travels inside the published tarball, and loses upstream's
duplicated `LICENSE` entry. `Cargo.lock` carries the matching rename and
nothing else. The published version string is deliberately not in the tree;
`publish.sh` derives it from crates.io. See [PUBLISHING.md](../../PUBLISHING.md).

## 0003 — `new` and `make` with non-type arguments (grammar)

**Repro**

```go
package a

func f() {
	_ = new("hello")
	_ = new(int32(1000))
	var make func(int) *X
	_ = make(n - 1)
}
```

**Diagnosis.** `call_expression` special-cases the literal tokens `new` and
`make` and routes them to `special_argument_list`, whose first element was
strictly `$._type`. But `new` and `make` are *predeclared identifiers*, not
keywords, and two things follow from that:

1. **Go 1.26 added `new(expr)`** — `new(x)` returns a pointer to a copy of
   `x` — so the first argument of a genuine builtin `new` call is now often
   an expression. Verified against the toolchain: a module declaring
   `go 1.26` compiles and runs `new("hello")` and `new(int32(1000))`.
2. **They can be shadowed.** `k8s.io/kube-openapi` vendors a test that
   declares `var make func(int) *X` and calls `make(n - 1)`.

Both shapes reached `special_argument_list` and had no path through it.

**Fix.** Widen that first element to
`choice($._type, prec.dynamic(-2, $._expression))`.

The dynamic precedence is the whole subtlety. Without it, a *bare identifier*
argument resolves to the expression reading and `new(int)` regresses from
`(type_identifier)` to `(identifier)` — a tree-shape change every consumer
query would feel. At `-2` the expression branch loses to the type branch,
which carries `prec.dynamic(-1)` on `_type_identifier` inside `_simple_type`.
So every pre-existing shape is preserved and only genuinely-non-type
arguments take the new path. Verified node by node:

| expression | node | |
|---|---|---|
| `make([]int, 3)` | `slice_type` | unchanged |
| `make(map[string]int)` | `map_type` | unchanged |
| `make(chan int, 1)` | `channel_type` | unchanged |
| `new(int)` | `type_identifier` | unchanged |
| `new(struct{ A int })` | `struct_type` | unchanged |
| `new("hello")` | `interpreted_string_literal` | new |
| `new(int32(1000))` | `call_expression` | new |

**Evidence.** One line of `grammar.js` closes all 6 clusters and all 17 gap
files; sweep goes 67,989/68,006 → **68,006/68,006**. The 17 were 15 Go-1.26
`new(expr)` uses across `k8s.io/kubernetes` and `k8s.io/apiserver`, plus 2
shadowed-builtin calls in `k8s.io/kube-openapi`. All 67 upstream corpus tests
still pass, plus the new one.

Not yet reported upstream. v0.25.0 predates Go 1.26, so the `new(expr)` half
is upstream falling behind the language rather than a mistake — which is the
case treebank exists for.

## Not patched, deliberately

Four kinds of invalid Go that `go/parser` rejects and this grammar accepts.
See `ledger.json`'s `grammar_is_broader_than_the_oracle` for the full
evidence; the short version:

| accepted | why it stays |
|---|---|
| no `package` clause | upstream's stated design — `source_file` admits top-level statements so editors can parse partial snippets (#63) |
| two `package` clauses | same rule, same decision |
| `func f(a ...int, b int)` | spec-conformant (`ParameterDecl = [ IdentifierList ] [ "..." ] Type`) **and** asserted by an upstream corpus test. Attempted and reverted: the fix works and holds the sweep, but deletes an upstream test case for zero corpus benefit |
| `func f()` `\n` `{ }` | Go's automatic semicolon insertion is a *lexer* rule, and this grammar has no external scanner |

All four are accepts-invalid, never rejects-valid, so none can inflate
`gap_files`. The cost is that `test/negative/` cannot pin them.
