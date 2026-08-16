# The node ontology

*Status: proposal. Nothing below is implemented. This document must be settled
before any owned `grammar.js` is written, because a grammar written before the
vocabulary is decided is a one-off that paid the cost of ownership for none of
the benefit.*

Treebank is writing its own grammars so that the ontology can be **enforced in
the parse table** rather than reconstructed afterwards by a query layer that
drifts. This document says what the vocabulary is, what each term means, what
it deliberately excludes, and how the enforcement is mechanical.

---

## 1. What the twenty-two existing vocabularies actually say

Every `supertypes:` block in every grammar tree on disk, read rather than
recalled. Twenty grammars live in this worktree's `crates/`; `erlang` and
`haskell` are in flight in sibling sessions and are included because they are
the two most opinionated vocabularies in the set.

| grammar | n | spelling | vocabulary |
|---|---|---|---|
| bash | 3 | all hidden | `_statement _expression _primary_expression` |
| c | 7 | 3 public | `expression statement type_specifier` + 4 `_*declarator` |
| csharp | 9 | all public | `declaration expression non_lvalue_expression lvalue_expression literal statement type type_declaration pattern` |
| elixir | 0 | — | — |
| erlang | 22 | all hidden | `_form _expr _expr_max _name _bit_type _bit_expr _desc _string_like …` |
| go | 5 | all hidden | `_expression _type _simple_type _statement _simple_statement` |
| haskell | 14 | all public | `expression pattern type quantified_type constraint constraints type_param declaration decl class_decl instance_decl statement qualifier guard` |
| html | 0 | — | — |
| java | 9 | 5 public | `expression declaration statement primary_expression module_directive` + `_literal _type _simple_type _unannotated_type` |
| javascript | 5 | all public | `statement declaration expression primary_expression pattern` |
| json | 1 | hidden | `_value` |
| lua | 4 | all public | `statement expression declaration variable` |
| php | 5 | all public | `statement expression primary_expression type literal` |
| python | 6 | 4 public | `expression primary_expression pattern parameter` + `_simple_statement _compound_statement` |
| rbs | 0 | — | — |
| ruby | 16 | all hidden | `_statement _arg _expression _primary _lhs _variable _method_name _call_operator _simple_numeric _nonlocal_variable` + 6 `_pattern_*` |
| rust | 6 | all hidden | `_expression _type _literal _literal_pattern _declaration_statement _pattern` |
| scala | 3 | 1 public | `expression` + `_definition _pattern` |
| toml | 0 | — | — |
| typescript | +2 | public | javascript's five, plus `type primary_type` |
| yaml | 0 | — | — |
| zig | 4 | all public | `statement expression type_expression primary_type_expression` |

**126 declarations, 17 grammars, 5 abstaining.** Term frequency, unifying
`_x` with `x`: `expression` 16, `statement` 12, `type` 9, some
`primary_*` tier 8, `declaration`/`definition` 7, `pattern` 7, `literal` 4,
`value` 1.

Three things fall out of reading them side by side.

### 1.1 The underscore is part of the query name, and nothing else

Measured, tree-sitter-cli 0.25.10:

- `(_value)` against tree-sitter-json **matches** — 4 captures on
  `{"a": [1, true]}`. A hidden supertype is queryable.
- `(_expression)` against tree-sitter-rust matches; `(expression)` is
  `Query error at 1:2. Invalid node type expression`.
- The same JSON grammar with `_value` renamed to `value` and left in
  `supertypes` produces a **byte-identical parse tree** and a
  `node-types.json` that differs in the name string and nothing else.

So the public/hidden split buys no tree difference, no `node-types.json`
structure difference, and no capability difference. Its entire effect is that
`(expression)` finds expressions in java, csharp, javascript, lua, php, scala,
zig, c and haskell, and is a hard query error in rust, go, python, bash, ruby
and erlang. **A single cross-language query cannot be written against the
current set.** That is the cheapest possible defect and it is present in six of
seventeen vocabularies.

### 1.2 `declaration` means four incompatible things

Read out of `node-types.json`, not out of the name:

| grammar | `declaration` contains | locals | imports |
|---|---|---|---|
| java | type-level only: `class_declaration`, `enum_declaration`, `record_declaration`, `interface_declaration`, `annotation_type_declaration`, `module_declaration`, `package_declaration`, `import_declaration` | `local_variable_declaration` is a **statement** | **inside `declaration`** |
| csharp | types **and** members: `class_declaration` … `method_declaration`, `field_declaration`, `property_declaration`, `event_declaration`, `using_directive` | `local_declaration_statement` is a **statement** | **inside `declaration`** |
| javascript | anything that binds in a scope: `class_declaration`, `function_declaration`, `generator_function_declaration`, `lexical_declaration`, `variable_declaration`, `using_declaration` | **inside `declaration`** | `import_statement`/`export_statement` are **statements** |
| lua | `function_declaration`, `variable_declaration`, `implicit_variable_declaration` | **inside `declaration`** | n/a |
| rust | `_declaration_statement`: every item, plus `let_declaration`, `macro_invocation`, `attribute_item`, `empty_statement` — i.e. "block content that is not an expression" | **inside** | `use_declaration` **inside** |
| scala | `_definition`: `class_definition`, `object_definition`, `trait_definition`, `given_definition`, `val_definition`, `val_declaration`, `var_definition`, `var_declaration`, `function_definition`, `function_declaration`, `type_definition`, `import_declaration`, `export_declaration`, `package_clause`, `package_object`, `extension_definition`, `enum_definition` | n/a | **inside** |
| haskell | top-level only; `decl` = `bind`/`function`/`signature`; `class_decl` and `instance_decl` are separate context-scoped supertypes | `decl` | not a declaration |

Four grammars, four different cuts, and each is defensible on its own terms:
*top-level only* (java), *not-a-statement* (rust), *binds a name* (javascript,
lua), *has a name and a body or a signature* (scala, haskell). The one thing
none of them is, is the same as another. This is the term the whole ontology
inherits from, which is why it is the first question below.

### 1.3 Most declared supertypes are parser plumbing, not ontology

`_primary_expression` (bash, java, javascript, php, python, ruby, zig),
`_expr_max` (erlang), `_simple_type` / `_unannotated_type` (go, java),
`_simple_statement` (go), `_lhs` / `_arg` / `_call_operator` (ruby),
`_bit_type` / `_bit_expr` / `_desc` / `_string_like` /
`_deprecated_fun_arity` (erlang), the four `_*declarator`s (c).

These name a precedence tier or a parse-table split. They are load-bearing for
the *grammar author* and meaningless to a *consumer*: nobody wants "every node
at erlang's maximal expression precedence". Of erlang's 22 supertypes roughly
20 are tiers; of ruby's 16, at least 9. Declaring them is how a vocabulary
stops being a vocabulary.

---

## 2. Proposed vocabulary

### Rule 0 — the list is closed

An owned grammar may declare supertypes **only** from the list in §3. It may
omit any of them (JSON uses two). It may not invent one. If a language
genuinely needs a term that is not on the list, the term is added to the list
**for every language**, with a definition and an exclusion clause. That is a
change to the ontology, not to one grammar — and it is the mechanism that
makes this a vocabulary rather than twenty-one private opinions.

### Rule 1 — ontology supertypes are public

No leading underscore on any term in §3. §1.1 measures the entire cost of the
underscore as "the cross-language query does not compile", and the entire
benefit as nothing.

### Rule 2 — plumbing stays hidden and undeclared

A precedence tier may exist as a hidden rule. It may not appear in
`supertypes:`. The test is a consumer question: if no one would ever ask for
"all nodes of this kind", it is not a kind.

### Rule 3 — coverage is enumerated, not assumed

Every named, non-`extras` node in `node-types.json` is either under an ontology
supertype, or listed in the ledger's `ontology.uncategorised` with a one-line
reason. Nothing is silently outside the vocabulary. This is the rule that keeps
the ontology honest as the grammar grows, and it is the one that is checkable.

### Rule 4 — supertypes may nest

Already true in tree-sitter and already used: rust's `_literal` sits inside
`_expression`. §3 states the required containments; the checker asserts them.

---

## 3. The terms

Nine terms, in three groups. For each: what it means, and what it excludes.

### Group A — the syntactic categories

**`expression`** — a construct that denotes a value.
*Excludes* type syntax; constructs that exist only for effect and denote
nothing (`return`, `break`, `let`); patterns. *Note* that where a language
makes control flow an expression (rust `if`/`match`/`block`, ruby everything,
scala) it is an `expression` here too — the ontology follows the language, not
a house style.

**`statement`** — a construct executed for its effect as one element of a
sequence.
*Excludes* an expression used as an expression; includes the wrapper node when
a language has one (`expression_statement`). Languages with no such
category — JSON, TOML, YAML, HTML, and arguably Haskell and Erlang — simply do
not declare it. The vocabulary never obliges a language to have a construct.

**`type`** — syntax appearing in type position.
*Excludes* the *definition* of a type (`struct_item` is a `definition`; only
its use sites are `type`s). *Excludes* expressions that merely evaluate to a
type: Python annotations are ordinary expressions, so **Python declares no
`type`**, which is the honest answer and matches upstream.

**`pattern`** — syntax in destructuring or matching position: the left of a
binding, a `match`/`case` arm, a destructuring parameter.
*Excludes* type patterns (those are `type`). *Absorbs* python's separate
`parameter` supertype where the language allows destructuring parameters; where
a language's parameter list cannot destructure, parameters are plain nodes and
`pattern` is not declared.

### Group B — the binding categories

This is where the existing vocabularies disagree (§1.2), so the proposal states
its cut sharply.

**`definition`** — introduces a named entity **and supplies its content**.

**`declaration`** — introduces a named entity and **does not supply its
content**: a signature, a prototype, an abstract member, an `extern`, a
`declare`.

The cut is *syntactic* — does this rule have a body? — so a grammar can decide
it at generate time, which is the whole point. And it is not invented: **four
of the six hardest languages already draw exactly this line in their concrete
node names**, they just do not lift it into a supertype.

| language | `declaration` | `definition` |
|---|---|---|
| c | `declaration` (prototype) | `function_definition` |
| rust | `function_signature_item` | `function_item` |
| haskell | `signature` | `function`, `bind` |
| scala | `val_declaration`, `var_declaration`, `function_declaration` | `val_definition`, `function_definition`, `class_definition` … |

Under this cut, java's `class_declaration` is a **`definition`** (it has a
body) despite its name; the ontology term and the node name are allowed to
disagree, and the node name stays whatever the language calls it.

Consequences that must be accepted rather than discovered later:

- **Some rules straddle it.** JavaScript's `lexical_declaration` covers both
  `let x = 1` and `let x;`. A supertype is per-rule, so one of them is
  mislabelled unless the rule is split. **Proposal: the cut applies only where
  the grammar rule can decide it**; where a single rule spans both, it is a
  `definition`, and the ledger records the straddle. Splitting the rule would
  diverge the tree shape from every existing tool for no consumer gain.
- **Multiplicity is not the ontology's business.** A Haskell function is a set
  of equations and tree-sitter-haskell parses one `function` node per equation.
  Each is a `definition`. A name may therefore have several. The alternative —
  a synthetic grouping node — diverges from every Haskell tool and is
  rejected.
- **Named-ness is required.** Rust's `impl_item` and Ruby's `singleton_class`
  supply content without binding a name. Under the definition as written they
  are outside the vocabulary. That is a real loss — "where is this trait
  implemented" is the query — and it is open question 5 below.

**`directive`** — affects the compilation unit or its environment rather than
binding a name in it: imports, exports, `package`, `#include`, `use`, `using`,
`require`, pragmas, module attributes, preprocessor conditionals, shebangs.

Today the same construct lives in three different places: java and csharp put
imports under `declaration`, javascript puts them under `statement`, erlang
gives them their own `_preprocessor_directive`. `directive` gives them one
home, and separates the two questions a consumer actually asks — *what does
this file depend on* and *what does this file define* — which today are the
same query in java and different queries in javascript.

*Excludes* nothing that binds a name **in the file's own namespace**. This is
the boundary that is genuinely arguable, because `import numpy as np` does bind
`np`; see open question 3.

### Group C — the leaves

**`literal`** — an expression whose value is fully determined by its own text:
no name resolution, no evaluation, no substitution — **for every instance of
the rule**.

The per-rule quantifier is what makes it decidable. It excludes rust's
`array_expression` (`[a, b]` is the same rule as `[1, 2]`) and includes JSON's
`array` (no JSON array can contain a name). It excludes any string rule that
can carry an interpolation node — so Python's `string` is not a `literal`,
because `f"{x}"` is the same rule. It excludes `-1`, which is unary minus over
a literal; rust's `negative_literal` exists precisely because patterns need the
folded form, and it is a `literal` where it appears.

*Required containment*: `literal ⊆ expression`.

**`value`** — reserved; see open question 2. Either data languages use
`expression`/`literal` like everyone else, or `value` is the ninth term and
`expression` is not declared by JSON, TOML, YAML or HTML.

---

## 4. The two first grammars, under this vocabulary

### JSON

Upstream declares one hidden supertype, `_value`, over
`object array number string true false null`.

Under §3, every one of those seven is a `literal` by the per-rule test — no
JSON construct can contain a name or a computation — and `_value` is exactly
the set of things that denote a value. So JSON declares **`expression` and
`literal`, with `literal ⊆ expression` and the two sets equal**, or, under
question 2's alternative, `value` and `literal`.

That equality is not a degenerate result. It is the ontology stating the thing
that makes JSON a data language, in the same terms it states everything else,
and it makes `(literal)` a query that means the same thing over a `.json` file
and a `.rs` file.

`pair`, `document`, `string_content`, `escape_sequence`, `comment` are
uncategorised and ledgered.

### TOML

Upstream declares nothing at all. Under §3:

- `string integer float boolean offset_date_time local_date_time local_date
  local_time array inline_table` → `literal` (and TOML's containers are
  transitively literal, same as JSON's).
- `pair` binds a key to a value and supplies it → **`definition`**.
- `table` and `table_array_element` name a table and supply its content →
  **`definition`**.
- `bare_key`, `quoted_key`, `dotted_key` are names. There is no `name` term in
  §3; they are uncategorised, and the absence is open question 6.

`(definition)` over a TOML file then returns every key and every table header,
and over a Rust file returns every item. Whether that is the ontology paying
off or the ontology overreaching is open question 2.

---

## 5. How this is enforced rather than described

`treebank ontology <grammar-dir>`, run per grammar in CI beside
`treebank ledger`:

1. every declared supertype is in §3's closed list;
2. no declared supertype name begins with `_`;
3. every named non-`extras` node in `node-types.json` is under some ontology
   supertype or listed in `ledger.ontology.uncategorised` with a reason;
4. the containments in §3 hold (`literal ⊆ expression`);
5. the ledger's declared vocabulary matches the generated `node-types.json`
   exactly — a supertype the ledger claims and the grammar does not produce is
   a failure, and so is the reverse.

For **vendored** grammars the same command runs in *report* mode: it prints the
drift and does not fail. That is the coexistence story — eighteen grammars keep
their inherited vocabularies and are measured against this one, two are owned
and are held to it — and it means the ontology has a number attached to it
from day one rather than only after the eighteenth grammar is rewritten.

---

## 6. Open questions — these are the user's call

1. **The `declaration`/`definition` cut.** Proposed: *has a body*. Alternatives
   that real grammars use: *top-level vs local* (java), *not-an-expression*
   (rust), *binds a name* (javascript), or collapse the two into a single
   `definition` and lose the signature/implementation distinction.
2. **Do data languages share the code vocabulary?** Proposed: yes — JSON and
   TOML declare `expression`/`literal`, and TOML's `pair` is a `definition`.
   Alternative: a separate `value` term, data languages declare no
   `expression`, and `(definition)` never matches in a config file.
3. **Where do imports go?** Proposed: a new `directive` term. Alternatives:
   `definition` (they do bind names), or `statement` (javascript's answer), or
   leave them uncategorised.
4. **Compatibility.** Owned grammars rename `_value` → `value`,
   `_expression` → `expression`. Parse consumers are unaffected (trees are
   identical, measured §1.1); **query consumers break**. Take the break, or
   carry upstream's spelling as an alias through a transition?
5. **Unnamed definitions.** Rust `impl` blocks, Ruby singleton classes, Java
   anonymous classes: outside the vocabulary as written. Widen `definition` to
   drop the name requirement, or accept the loss?
6. **Is there a `name` term?** Erlang declares `_name`; nothing else does. TOML
   keys, identifiers, qualified paths all want it. Ninth term, or not?
