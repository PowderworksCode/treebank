# The C validity oracle

Answer to the first-move question, written before any grammar work:
**what does the oracle assert, and how does a missing header avoid being
counted as invalid syntax?**

## What the oracle asserts

> **Claim.** This file contains no C *syntax* error, judged by clang's
> parser, in the GNU C dialect, given the include paths we supplied.

It deliberately does **not** claim:

- that the file compiles (types resolve, symbols link, macros exist);
- that the file is valid under some abstract "C standard" reading — real
  corpus C is GNU C, so the dialect is `gnu17` and `__attribute__`,
  statement expressions, `__auto_type` and K&R definitions are all valid;
- that the file is valid *in isolation*. C has no such notion. `foo * bar;`
  is a declaration or a multiplication depending on a typedef that arrives
  through `#include`. The oracle's verdict is relative to the include
  environment, and the environment is part of the recorded evidence.

That is a weaker claim than rust's (`syn` accepts a file outright) or
java/csharp's (a parse phase that genuinely is context-free). It is the
strongest claim C admits, and the sweep numbers must be read as
"syntax-valid under this include environment", not "valid C".

## Why gcc cannot make that claim, measured

`gcc -fsyntax-only` is the tool `DESIGN.md` names, and it is the wrong one.
Two measured failures, gcc 13.3.0:

| probe | gcc says | verdict if believed |
|---|---|---|
| `#include <absent.h>` then a use of a typedef from it | `fatal error: … No such file or directory` / **`compilation terminated`** | invalid — and nothing after line 1 was ever parsed |
| `MYLIB_EXPORT int f(void);` (macro from a header we lack) | `error: expected ';' before 'int'` | invalid — *in the syntax diagnostic class* |

The first is the brief's warning. The second is worse and kills the brief's
"distinguish gcc's diagnostic classes" option outright: a **missing macro
definition produces a genuine syntax-class diagnostic**, textually
indistinguishable from the real syntax error in `int main(void) { int x = ;
return x }` (`error: expected ';' before '}' token`). Classifying gcc's
English message text cannot separate them, because at gcc's level of
recovery they are the same event. gcc also emits no stable diagnostic IDs,
so any classifier would be regex-over-prose.

## What the oracle is instead

`tools/c-oracle` — a small C program against **libclang**, the analogue of
the Roslyn (`csharp`) and `JavacTask.parse` (`java`) oracles: the reference
implementation's own front end, driven directly rather than through the
compiler driver. libclang is already on this box (`libclang-20-dev`,
20.1.2); `clang-20` and `clang-21` are installed too, contrary to the
brief's note — only the unversioned `clang` name is absent.

Three libclang properties do the work, all verified by probe:

1. **`CXTranslationUnit_KeepGoing`** makes a missing include non-fatal, so
   the rest of the file is still parsed. gcc terminates; libclang continues
   and reports what it found downstream.
2. **Structured diagnostic categories** — `clang_getDiagnosticCategoryText`
   returns `Parse Issue`, `Semantic Issue`, `Lexical or Preprocessor Issue`
   from clang's own diagnostic tables. This is data, not message text.
3. Clang's parser **recovers into the semantic category** where gcc falls
   into the syntax one: the undefined-macro declaration above yields
   `Semantic Issue: unknown type name 'MYLIB_EXPORT'`, not `expected ';'`.

### The rule

Collect every diagnostic at severity ≥ error and bucket it by clang's
category. Then, on counts alone:

- **valid** ⟺ `parse == 0`. Semantic and preprocessor errors are ignored
  wholesale — that is the entire point.
- **invalid** ⟺ `parse > 0` **and** `semantic + lexpp + userdef + other == 0`.
- **indeterminate** ⟺ otherwise: the parser stumbled, but something else
  also went wrong, so we cannot tell a real syntax error from a macro we
  never saw.

The four buckets are clang's own category names: `Parse Issue`,
`Semantic Issue`, `Lexical or Preprocessor Issue`, `User-Defined Issue`.

**No diagnostic message text is matched, anywhere.** The rule reads clang's
category buckets and nothing else. An earlier draft of this rule special-cased
the `'x.h' file not found` message to identify missing context; that was
strictly worse, because `#error "unsupported platform"` — which real C hits
constantly once an `#if defined(...)` branch goes unguarded — is also a
`Lexical or Preprocessor Issue`, and its message is arbitrary user prose. The
categorical rule treats it as missing context automatically, which is right.

Any error whose category is none of those four counts as `other`, pushes the
file to indeterminate rather than invalid, and gets its name reported. A
clang change surfaces as a visible count instead of a silent
misclassification — and that tripwire has already fired once, usefully:
`#error "requires the vendor SDK"` turned out to be **`User-Defined
Issue`**, a fourth category I had not predicted, found by running the probe
rather than by reasoning about it. It is now named and counted, and treated
as missing context, because a `#error` is precisely the author stating that
this configuration is unsupported. `other` is back to being a pure tripwire.

The cost of being purely categorical is that any semantic or preprocessor
error at all demotes a genuine syntax error to indeterminate. That direction
is deliberate: it never invents invalidity.

The `unresolved` guard is not decoration. Measured, on a two-line file that
is unambiguously valid C:

```c
#include <mylib.h>                                    /* header we hold */
MYLIB_EXPORT mylib_size_t mylib_len(const char *s) { return s ? 1 : 0; }
```

| include env | libclang diagnostics | rule |
|---|---|---|
| no `-I` | `Lex/PP: 'mylib.h' file not found` + `Semantic: unknown type name 'MYLIB_EXPORT'` + **`Parse: expected ';' after top level declarator`** | indeterminate (a bare parse-issue rule would say **invalid** — the exact trap) |
| `-Ipkg/include` | *none* | **valid** |

And on the four probes:

| probe | categories | rule |
|---|---|---|
| missing header, typedefs from it | Lex/PP + Semantic×2, no Parse | valid ✓ |
| `foo * bar;`, `foo` unknown (the lexer hack) | Semantic×2, no Parse | valid ✓ |
| undefined macro declarations | Semantic×3 + Parse×1 | indeterminate ✓ |
| `int x = ; return x }` | Parse×2 only | **invalid** ✓ |
| GNU C: `__auto_type`, `({…})`, K&R defs, `__attribute__` | none | valid ✓ |
| `#error "requires the vendor SDK"` on an unmatched `#if` | User-Defined×1 | valid ✓ |
| C header: bitfields, `__attribute__((nonnull))`, include guard | none | valid ✓ |
| C++ header: `namespace` + `template class` | Parse×1 + Semantic×1 | indeterminate |

That last row is why `.h` files get a C++ filter in `classify()` rather than
being fed in raw: a C++ header does **not** come back cleanly `invalid`, it
comes back `indeterminate`, so unfiltered headers would inflate the one
bucket whose size decides whether C is sweepable at all.

## How a missing header stops being "invalid syntax" — three layers

1. **Supply the include paths.** Per corpus file: the file's own directory,
   the package root, and any top-level `include/`, `inc/`, `src/` — plus
   clang's resource-dir builtins and system headers. The table above is the
   measurement that this converts "invalid" straight to "valid", not merely
   to "indeterminate". **No build system is executed**: no `./configure`, no
   `cmake`, so a generated `config.h` is simply absent and its absence shows
   up as indeterminate rather than as a fabricated verdict. Running upstream
   build scripts would resolve more, but it would execute arbitrary code and
   make verdicts depend on which build deps this machine happened to have —
   unreproducible in a way `ledger.json` could not capture.
2. **Never let a semantic error mean invalid.** Missing types, missing
   symbols, missing macros-used-as-types all land in `Semantic Issue` and
   are discarded regardless of how many there are.
3. **Refuse to guess.** Where context is provably missing *and* the parser
   still stumbled, the answer is indeterminate — not "invalid", and not
   "valid" either.

## What this costs, stated plainly

- **Under-counted invalidity.** A file with both a real syntax error and an
  unresolved include is indeterminate, not invalid. Measured: `#include
  <nope.h>` + `int x = ; return x }` → Lex/PP + Parse×2 → indeterminate.
  So "invalid" is a floor, not a count. The negative corpus covers the
  accepts-invalid-code direction, and its files are self-contained by
  construction, so this blind spot does not weaken it.
- **The oracle has real rejection power** — it must, or nothing downstream
  means anything. `broken.c` is rejected on parse issues alone, and every
  `test/negative/` file will be one the oracle rejects with an empty
  `unresolved` set. This is not a `validate()` that says everything is
  valid; the measured probe set contains a rejection.
- **Indeterminate must be visible, never folded into silence.** It is a
  measured quantity that says how much of the corpus we cannot adjudicate,
  and it is the number that decides whether C is worth sweeping at all. If
  it comes back dominant on the real corpus, that is a stop-and-report
  result, not something to average away.

## How the three values reach the sweep (decided)

`Lang::validate` returns `HashMap<String, bool>`, where `true` becomes a
"gap" cluster and `false` becomes "noise". Decision: **indeterminate
collapses to `false`.** No fix agent is ever dispatched against a file whose
validity we cannot vouch for, and the shared trait stays untouched, so the
other five languages are unaffected by C's ambiguity.

The cost of that collapse is that `sweep.rs`'s `noise_files` mixes "the
reference parser rejected this" with "we could not tell", and `gap_files` is
therefore a floor. That is paid for explicitly, not silently:

- `tools/c-oracle` emits all three counts, and `lang/c.rs` writes
  `corpus/c/oracle-verdicts.json` — per-file verdict, the diagnostic
  categories behind it, and the include paths in force.
- The sweep prints `oracle: N valid, N invalid, N indeterminate` so the
  unadjudicated count is never absent from the run it belongs to.
- Any reported C gap number is quoted with its indeterminate count beside
  it. A pass rate without that denominator is not a claim this crate makes.

## Measured result on the 20-package pilot

The section above says that if indeterminate came back dominant, that is a
stop-and-report result. **It came back dominant**, so here is the report.

Debian sid, top 20 C sources by popcon, 39,928 `.c`/`.h` files, tree-sitter-c
0.24.2 unpatched:

| | files |
|---|---|
| parsed clean | 21,991 (55.1%) |
| grammar rejected | 17,937 (44.9%) |
| ├─ oracle: **valid** → grammar gap | **5,502** |
| ├─ oracle: **invalid** → corpus noise | **452** |
| └─ oracle: **indeterminate** → cannot say | **11,983** |

Indeterminate outnumbers valid **2.2 : 1**. ~79% of those files carry an
unresolved `#include` — but that is not what makes them indeterminate, and
resolving those includes would not change a single verdict. See "Stubbing
`config.h` cannot work" below before reaching for the obvious fix.

An earlier revision of this table read 5,629 / 302 / 12,006. Those numbers
were produced while `c-oracle` still capped a request at 128 flags and
silently dropped the rest, which truncated the include list for glibc (498
header-bearing dirs), mesa (473) and samba (301). The cap is gone. Correcting
it, and adding `-idirafter`, moved 127 files out of the gap queue and
promoted 150 more from "cannot say" to positively invalid — a better-evidenced
picture rather than a rosier one.

**Assessment: this is worth sweeping, and the numbers do not lie — they
under-claim.** Three things have to be true for that, and all three are
measured rather than asserted:

1. **The oracle has real rejection power.** 452 corpus files were positively
   rejected on parse issues with no missing-context evidence, and all nine
   negative-corpus files are oracle-rejected and grammar-rejected. This is
   not a `validate()` that says everything is valid.
2. **The 5,502 gap files are not weakened by the 11,983.** Each of them is a
   file clang parsed with zero parse issues in a resolved-enough environment.
   The indeterminates do not cast doubt on that set; they only mean the true
   gap count is *larger* than 5,629.
3. **The error direction is one-way.** Everything unresolved lands in
   "not a gap". The sweep can under-report grammar bugs; it cannot invent
   them, and it cannot flatter the grammar's pass rate, because `passed` is
   measured against the whole corpus rather than against the adjudicable
   part.

What it costs: **`gap_files` is a floor, permanently, for this language.**
Any C number quoted without its indeterminate count beside it is a
misrepresentation, which is why `validate()` prints all three and writes
`oracle-verdicts.json`.

### Stubbing `config.h` cannot work, and the reason is structural

An earlier revision of this document proposed exactly that: most unresolved
includes are the `./configure`-generated `config.h`, so supply an empty stub,
resolve the include, and watch the indeterminate mass fall. It was wrong, and
it is written up here rather than deleted because it is the obvious idea and
the next person will have it too.

**A missing include never prevents a `valid` verdict.** The rule is `valid ⟺
parse == 0`; a file with no parse errors is already valid however many headers
are absent. So an include can only matter if resolving it *removes a parse
error* — and an empty stub supplies nothing. It cannot define the `HAVE_*`
macros or typedefs whose absence caused the parse error in the first place.

What a stub does do is delete missing-context evidence. Since `invalid ⟺ parse
> 0 and everything-else == 0`, its only possible effect is to move files from
indeterminate to **invalid** — the oracle asserting bad syntax about valid C.
All downside, no upside.

Every indeterminate file has `parse > 0` by definition. On the 20-package
pilot's 11,983 of them:

| files | |
|------:|---|
| 7,586 | a missing include **and** semantic errors — a stub changes nothing |
| 4,279 | no missing include at all — nothing to stub |
| **118** | a missing include only — **a stub would flip these to `invalid`** |

Measured to match: two runs over 500 indeterminate files, with and without a
stub tree, produced **0 flips out of 500**. Two shapes of stub were tried,
because the first attempt put `config.h` at the stub root when the commonest
spelling is `include/config.h`; correcting that changed nothing, which is when
the rule above made the experiment unnecessary.

**The real lever is fewer parse errors, not more headers.** Those parse errors
come overwhelmingly from unexpanded macros, so raising the floor means macro
expansion *in the oracle path* — and that is far more dangerous than the
diagnosis-only expansion in `treebank_preprocessing`, because it would let
expansion decide a verdict while the macro census cannot tell which header
actually reached which file. The sound route is per-file include resolution,
following real `#include` chains so the macro environment is genuine. That is
a large piece of work and it is not attempted here.

So the floor stays. That is a property of C, not a gap in the tooling.

## Reproducibility

libclang is pinned like `generate_cli` is, and for the same reason: clang's
recovery behaviour decides verdicts, so a version change moves the gap
numbers. `ledger.json` records `oracle: {tool: libclang, version: 20.1.2,
dialect: gnu17, flags: [...]}`. Note `-std=` is a real knob: C23 files may
need `gnu23`, and the choice belongs in the ledger, not in a comment.

## Preprocessor observations (out of scope, recorded)

The undefined-macro-as-declaration-prefix pattern (`MYLIB_EXPORT`,
`G_BEGIN_DECLS`, `PyAPI_FUNC(...)`) is the single mechanism behind every
indeterminate verdict seen so far. Recorded here, per the brief, and left
to the preprocessor session. No mechanism is built for it in this crate.
