# treebank-csharp

Upstream [tree-sitter/tree-sitter-c-sharp](https://github.com/tree-sitter/tree-sitter-c-sharp)
pinned at **0.23.5** (`cac6d5fb595f5811a076336682d5d595ac1c9e85`, the commit
tagged v0.23.5) as the `upstream/` git submodule; `scripts/materialize.sh`
applies the patch series below and generates the parser into `build/`
(gitignored). One grammar, no
npm deps for generation (`generate_deps` is null). Contract, reconstruction
invariant, CLI pin rationale, and workflow: see
[GRAMMARS.md](../../GRAMMARS.md) at the repo root.

## The corpus is not the package

**NuGet ships compiled assemblies. There is not one `.cs` file in any of the
top twenty packages.** So unlike rust, typescript, javascript and java, the
C# corpus cannot be "what the registry serves".

What the registry does serve is SourceLink metadata: nearly every package's
`.nuspec` carries `<repository url="…" commit="…">` naming the exact git
commit it was built from. `resolve()` downloads the `.nupkg`, reads that,
and returns a GitHub source archive at that commit. Reproducible, and
pinned to a commit rather than a branch — but it is **repository** source:
tests, samples and build tooling included, and plenty of code that never
shipped in the package.

Two consequences worth knowing before reading any number in the ledger:

- **The top of NuGet collapses into a few monorepos.** Eleven of the top
  twenty packages resolve to `dotnet/dotnet`, the .NET Virtual Mono Repo.
  `resolve()` therefore refuses a repo+commit already claimed by a
  higher-ranked package — 13 of the top 100 are skipped that way — and even
  so the corpus is dominated by `dotnet/dotnet` and
  `Azure/azure-sdk-for-net`. **File counts here reflect repository size, not
  package popularity.**
- **8 packages have no usable repository metadata** (several AWS and legacy
  Microsoft packages) and are skipped rather than guessed at.

100 ranked packages → 50 fetched → 860,590 `.cs` files.

## Reference parser

`tools/cs-oracle` is Roslyn, via `CSharpSyntaxTree.ParseText`, at
`LanguageVersion.Latest` — the newest *stable* C#, deliberately not
`Preview`. A file needing an unreleased feature is not yet valid C#, and
recording it as corpus noise is more honest than reporting the grammar's
rejection of unreleased syntax as a gap. Parse-only, so unresolved types are
not errors and each file is judged on its own.

### The preprocessor asymmetry — read this before trusting the gap count

Roslyn parses the **active configuration**. With no preprocessor symbols
defined, `#if FOO` is false and only the `#else` branch is parsed; the other
branch is disabled text, and whatever it contains cannot make the file
invalid. tree-sitter parses **all** branches as part of one tree.

For a great deal of real C# these disagree, and neither is wrong:

```csharp
#if BUILD_ENGINE
namespace Microsoft.Build.BackEnd.Components.Caching
#else
namespace Microsoft.Build.Shared
#endif
{
```

Roslyn sees one namespace declaration. The grammar sees two, then a `{`.
There is no single well-formed tree for that file, so it is not a grammar
bug that a fix agent could close.

**5,040 of the 7,148 remaining oracle-valid failures contain conditional
compilation** — and that is correlation, so it was tested directly. Reducing
each of those 5,040 to the configuration Roslyn actually parsed (every `#if`
false, only `#else` kept, directive and inactive lines blanked so line
numbers still line up) and re-parsing gives:

| | files |
|---|---:|
| parse cleanly once reduced — failure caused *only* by conditional compilation | **4,617** |
| still fail after reduction — real candidates | 423 |

So the honest split of the 7,148 is 4,617 inherent and **2,531 actionable**
(2,108 with no conditional compilation, plus those 423). The reducer is in
the session notes, not the repo; re-deriving it is a few dozen lines of
`#if`-evaluation with every symbol undefined.

## Patches

1. **Treebank redistribution notice** (`0001`) — prepends a warning to
   upstream's `README.md` stating that this tree is an automatically
   generated, patched redistribution maintained by
   [treebank](https://treebank.dev), so the notice travels with every
   materialized/published copy. Applied first; touches no grammar code.

2. **`async` is a contextual keyword** (`0002`) — `Run(async)`,
   `Run(async: true)`, `var resultCode = async`. `async` modifies a method,
   lambda or anonymous method but is a legal identifier everywhere else, and
   .NET's own test suites lean on that heavily (a `bool async` parameter
   threaded through parameterised tests). It joins the grammar's existing
   `_reserved_identifier` list next to `var`, `when`, `where` and `yield`.

   It needs `prec(-2)` rather than a conflict declaration. A bare `async`
   can begin a method modifier, a lambda/anonymous-method modifier or an
   identifier; declaring the four-way conflict resolved the top-level case
   and the same ambiguity immediately reappeared inside the generated
   declaration-list repeat. The precedence keeps the modifier and lambda
   readings winning wherever they apply and lets `async` fall back to an
   identifier only where they cannot.

   **+3,799 files** (849,118 → 852,917). All 168 upstream corpus tests still
   pass, and the added test keeps identifier uses and `async` lambdas
   (`async () =>`, `async x =>`, `static async (int x) =>`,
   `async delegate`) in one file.

## Negative corpus

`test/negative/` holds 8 files Roslyn rejects and this grammar must keep
rejecting — a Java varargs signature, a Rust `fn … -> i32`, a TypeScript
type annotation and a Visual Basic module among them, since "accepts another
language's syntax" is the cheapest way for a C# grammar to drift.

One candidate was tried and left out: `public private void M()`, which
Roslyn's *parser* accepts (duplicate accessibility is a later error), so it
is not a syntactic case at all.

3. **Treebank crate identity** (`0003`) — packaging, not a grammar
   change. Upstream owns `tree-sitter-c-sharp` on crates.io, so this publishes as
   `treebank-grammar-csharp` with our `repository`, `homepage` and
   `description`. `[lib] name` stays `tree_sitter_c_sharp` so the crate is a drop-in.
   `include` gains `LICENSE`, `ledger.json`, `LOCAL-PATCHES.md` and
   `patches/*` so provenance travels inside the published tarball. The
   published version string is deliberately *not* here — it is derived from
   crates.io at publish time. See [PUBLISHING.md](../../PUBLISHING.md).
