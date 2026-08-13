# Publishing grammar crates to crates.io

Merging a change to a grammar publishes that grammar's crate. This file is the
whole contract: what gets published, under what name and version, what stops a
publish, and the one piece of setup a human has to do.

Mechanism: [`scripts/publish.sh`](scripts/publish.sh), driven by
[`.github/workflows/publish-grammars.yml`](.github/workflows/publish-grammars.yml).

## Setup (required once, by a human)

Publishing needs a crates.io API token as a repository secret. Nobody but a
crates.io account owner can create one, so this cannot be automated.

1. On <https://crates.io/settings/tokens>, **New Token**:

   | Field | Value |
   |---|---|
   | Name | `treebank CI` |
   | Endpoint scopes | `publish-new` **and** `publish-update` |
   | Crate scopes | `treebank-grammar-*` |
   | Expiry | your call; CI fails loudly when it lapses |

   `publish-new` is needed because the first version of each crate creates the
   crate; `publish-update` for every version after that. Nothing here needs
   `yank` or `change-owners`, and the crate scope means a leaked token cannot
   touch anything outside this namespace.

2. In the repo: **Settings → Secrets and variables → Actions → New repository
   secret**, name `CARGO_REGISTRY_TOKEN`, value the token.

3. Recommended, because publishing cannot be undone: **Settings → Environments
   → `crates-io` → Required reviewers**, add yourself. The publish job already
   declares that environment, so every real publish then waits for a click. Dry
   runs are unaffected.

The token is read only from the environment, and only by `cargo publish`. It is
never written to the repo, a workflow file, or a log.

Until the secret exists, dry runs work normally and a real publish fails with a
message pointing here. That failure is deliberate: a publish run that quietly
uploads nothing looks exactly like one that worked.

## What is actually published

Not the directory under `crates/` — that holds only the sources of truth: the
`upstream/` submodule pointer, `patches/`, and `ledger.json`. The crate is
[`scripts/materialize.sh`](scripts/materialize.sh)'s output, `<grammar>/build/`,
with the provenance files copied in beside it. Publishing therefore runs the
same materialization CI verifies and uploads exactly what it produced.

`ledger.json`, `LOCAL-PATCHES.md` and `patches/` ship *inside* the tarball, so
anyone who downloads it can see exactly how it differs from upstream without
leaving the crate.

## Names

Upstream owns the `tree-sitter-*` names on crates.io, so these crates cannot use
the names in upstream's manifests. They publish as:

<!-- BEGIN GENERATED: scripts/grammar-docs.sh -->
| directory | crate | library |
|---|---|---|
| `crates/treebank-bash` | `treebank-grammar-bash` | `tree_sitter_bash` |
| `crates/treebank-c` | `treebank-grammar-c` | `tree_sitter_c` |
| `crates/treebank-csharp` | `treebank-grammar-csharp` | `tree_sitter_c_sharp` |
| `crates/treebank-elixir` | `treebank-grammar-elixir` | `tree_sitter_elixir` |
| `crates/treebank-go` | `treebank-grammar-go` | `tree_sitter_go` |
| `crates/treebank-html` | `treebank-grammar-html` | `tree_sitter_html` |
| `crates/treebank-java` | `treebank-grammar-java` | `tree_sitter_java` |
| `crates/treebank-javascript` | `treebank-grammar-javascript` | `tree_sitter_javascript` |
| `crates/treebank-json` | `treebank-grammar-json` | `tree_sitter_json` |
| `crates/treebank-lua` | `treebank-grammar-lua` | `tree_sitter_lua` |
| `crates/treebank-php` | `treebank-grammar-php` | `tree_sitter_php` |
| `crates/treebank-python` | `treebank-grammar-python` | `tree_sitter_python` |
| `crates/treebank-rbs` | `treebank-grammar-rbs` | `tree_sitter_rbs` |
| `crates/treebank-ruby` | `treebank-grammar-ruby` | `tree_sitter_ruby` |
| `crates/treebank-rust` | `treebank-grammar-rust` | `tree_sitter_rust` |
| `crates/treebank-scala` | `treebank-grammar-scala` | `tree_sitter_scala` |
| `crates/treebank-toml` | `treebank-grammar-toml` | `tree_sitter_toml_ng` |
| `crates/treebank-typescript` | `treebank-grammar-typescript` | `tree_sitter_typescript` |
| `crates/treebank-yaml` | `treebank-grammar-yaml` | `tree_sitter_yaml` |
| `crates/treebank-zig` | `treebank-grammar-zig` | `tree_sitter_zig` |
<!-- END GENERATED -->

The crate name is **derived from the directory**, not read from the manifest.
That is safe to key on because `treebank ledger` (which `verify.sh` runs first)
already refuses a grammar whose directory and `grammar` field disagree, so
`crates/treebank-<lang>` and the ledger's language name are the same string by
the time publishing looks at either. `publish.sh` computes the name it expects
and refuses to publish a crate whose materialized manifest disagrees — so a grammar added without its identity patch
fails loudly at plan time instead of trying to upload under a name we do not
own.

The **library** name stays upstream's, deliberately, so these are drop-in.
For php that pin is load-bearing rather than cosmetic: upstream's manifest
declares no `[lib] name`, so Cargo derives one from the package name, and
renaming the package alone would have silently renamed the library to
`treebank_grammar_php` and broken every consumer's `tree_sitter_php::LANGUAGE_PHP`.
Patch 0002 sets it explicitly for that reason.

```toml
tree-sitter-rust = { package = "treebank-grammar-rust", version = "0.24.2-treebank.1" }
```

leaves `use tree_sitter_rust::LANGUAGE;` compiling unchanged. The consequence is
that a crate cannot depend on both ours and upstream's at once — they are the
same grammar, so that is not a real use case, but it is worth knowing.

Attribution is not decoration here. Each crate keeps upstream's MIT `LICENSE`
verbatim and upstream's `authors`; `repository` points at *this* repo, because
that is where this crate's source is and where its bugs belong; `homepage`
points at <https://treebank.dev>; `description` and the README (via the
`0001-treebank-redistribution-notice` patch every grammar carries) both lead
with the fact that this is a redistribution, not an upstream release.

## Versions

The published version is the **upstream** version plus an incrementing suffix:

```
upstream 0.24.2  ->  0.24.2-treebank.1, 0.24.2-treebank.2, ...
```

The tree never stores a published version. The materialized `Cargo.toml` carries
the upstream version — which `publish.sh` asserts equals `ledger.json`'s
`upstream.version` — and the suffix is derived at publish time from what
crates.io already has. There is no counter to drift out of sync, and a computed
version is by construction one that does not exist yet. Yanked versions still
count: the number is spent either way.

### The tradeoff, stated plainly

`0.24.2-treebank.1` is a semver **pre-release** of `0.24.2`, not a version after
it. That means:

- it sorts **below** plain `0.24.2`;
- `cargo add treebank-grammar-rust` will **not** pick it, and neither will a
  requirement of `"0.24"`, `"0.24.2"`, or `"^0.24"`. Cargo excludes pre-releases
  unless the requirement itself names one.

**Consumers must write the exact version**, as in the snippet above. Ordering
*within* the series behaves correctly — pre-release identifiers compare
numerically, so `treebank.1 < treebank.2 < treebank.10`.

This was chosen with the alternatives on the table (a reserved patch range like
`0.24.201`, or an independent `1.0.0` line, both of which resolve normally but
give up the at-a-glance correspondence to upstream). Exact pinning is defensible
for a redistribution whose content is defined by a patch set: you want to know
which patch set you are getting. It is recorded here because the choice is
permanent — crates.io versions can be yanked but never deleted, and a name and
version can never be reused.

## What stops a publish

The materialization invariant (see [GRAMMARS.md](GRAMMARS.md)) is the gate. A
crate that does not come out of `upstream submodule @ pinned sha + patches +
tree-sitter generate` with the pinned CLI, pass its corpus tests, and still
reject every file in the negative corpus **is not published**, full stop. That
invariant is the repo's entire claim to being trustworthy; shipping a tarball
that failed it would make the claim false.

It is enforced twice, on purpose:

1. the publish workflow's `verify` job *is* `verify-grammars.yml`, called rather
   than copied, so there is one definition of the check;
2. `publish.sh` runs `scripts/verify.sh` itself per crate before touching the
   network. The verify matrix is now derived rather than hand-written, so it can
   no longer omit a grammar — but the two jobs also scope themselves separately
   (one by what changed, one by publish tags), and this keeps the gate attached
   to the thing being uploaded rather than to a job that ran beside it. It also
   makes the script safe to run by hand.

A crate also will not publish if the materialized package name is not the
`treebank-grammar-<lang>` its directory implies, if its version disagrees with
the ledger's `upstream.version`, or if `cargo package` cannot build the tarball.

## What gets published, and when

On merge to `main`, each grammar crate is published **only if something under
its directory changed since the tag of its own last publish**
(`<crate>-v<version>`, pushed by the workflow). That includes the `upstream/`
submodule pointer, so an upstream version bump counts as a change even though no
file in this repo does.

- A docs typo, or a change to another crate, publishes nothing.
- Changes under a grammar's `test/` do not publish either: the negative corpus
  gates the release but never ships, so a corpus-only change does not alter the
  artifact.
- Re-running the workflow after a successful run is a no-op.
- If publishing several crates and one fails, the successful ones are tagged and
  the failed one is not, so a re-run retries exactly what is left. Nothing is
  ever republished.

`treebank-cli` is not published by this workflow. A grammar crate is recognised
by having a `ledger.json`, which the CLI does not.

## Testing it without publishing anything

`publish.sh` can be checked up to the moment it uploads and no further: against
crates.io the next step is irreversible. Everything that only happens *after* a
successful upload — the tag, the skip on a re-run, the suffix increment, and a
consumer actually resolving the crate — used to be untestable.

[`scripts/test-publish.sh`](scripts/test-publish.sh) closes that gap. It stands
up a real registry ([`cargo-http-registry`](https://crates.io/crates/cargo-http-registry))
on localhost, publishes every grammar to it for real, and asserts:

1. every grammar publishes and gets tagged;
2. a consumer crate resolves those crates from the registry, links them under
   *upstream's* names via cargo's `package = ` rename, and parses code
   upstream's grammars reject — so what was published really is the patched
   grammar;
3. re-running publishes nothing, because everything is tagged;
4. forcing a second publish lands on `-treebank.2`, not `.1` — the suffix really
   is derived from the registry rather than assumed.

```sh
scripts/test-publish.sh                 # full: materializes and verifies first
scripts/test-publish.sh --skip-verify   # what CI runs, since `verify` already gated
```

It never touches crates.io, never pushes, and deletes the tags it created on
exit. CI runs it on every change as the `rehearse` job, which `publish` depends
on.

The consumer fixtures live in [`tools/consumer-test/fixtures/`](tools/consumer-test/fixtures/)
and are drawn from the ledgers: each one is a construct that upstream's grammar
gets wrong, so a fixture parsing correctly is positive evidence that the patch
series survived packaging. `must-reject.rs` is the other direction — invalid
code upstream wrongly accepts, which our grammar must still reject.
[`grammars.json`](tools/consumer-test/grammars.json) maps each grammar to its
dependency line and its fixtures; the consumer's `Cargo.toml` and case list are
generated from it for exactly the grammars under test, so a one-grammar run
builds a one-grammar consumer.

The rehearsal tags under `rehearsal/`, its own namespace. It publishes at the
same version numbers production does — both count up from an empty registry —
so without that separation its tags would collide with the real ones, and a
real tag would make it skip instead of publish, quietly testing nothing.

## Which grammars a change is tested against

Verifying five grammars for a one-line patch to one of them is four wasted jobs,
so CI asks first. [`scripts/changed-grammars.sh`](scripts/changed-grammars.sh)
answers it:

- a change under `crates/treebank-<lang>/` concerns that grammar;
- a change to a **core** path — `scripts/`, `crates/treebank-cli/`, `tools/`,
  `.github/workflows/`, `.gitmodules`, the root `Cargo.toml`/`Cargo.lock` —
  concerns all of them, because that is what builds, verifies and packages every
  grammar;
- anything else (docs, corpus reports) concerns none, and CI does nothing.

`verify-grammars.yml` runs it in a `changes` job, builds its verify matrix from
the answer, and exposes it as a `workflow_call` output that `publish-grammars.yml`
reuses for `plan` and `rehearse` — so there is one definition of the rule, and it
has a self-test:

```sh
scripts/changed-grammars.sh --self-test
scripts/changed-grammars.sh origin/main      # what would this branch test?
```

### Publishing asks a narrower question

Testing scope and publishing scope are not the same question, and conflating
them costs real versions. What gets *published* is still decided per crate by
`publish.sh`, by diffing the crate against the tag of its own last publish —
plus `ARTIFACT_INPUTS`, the files outside a grammar directory that can still
change what its crate contains:

```sh
ARTIFACT_INPUTS=(scripts/materialize.sh scripts/publish.sh)
```

`materialize.sh` builds the tree that ships and `publish.sh` stamps and packages
it, so a change to either does put every crate up for a republish. But a change
to `treebank-cli` or `verify.sh` does not: those decide whether a crate is
*allowed* to ship, not what is in it, and cutting five releases for a change that
cannot alter a byte a consumer downloads spends five version numbers that can
never be reused.

Note that this is a *change* check and not a force: once a run tags, a re-run
skips. Widen the list if you disagree with where that line sits — it is one
array in `publish.sh`.

## Running it by hand

```sh
scripts/publish.sh --dry-run                      # all grammars; packages, uploads nothing
scripts/publish.sh --dry-run crates/treebank-rust # one grammar
scripts/publish.sh --execute                      # the real thing; needs the token
```

Flags: `--force` (publish even if unchanged), `--skip-verify` (materialize but
skip the corpus, only when the caller has already run it), `--no-tag`.

From the Actions tab, **publish-grammars → Run workflow** does the same. Its
`execute` box defaults to off, so dispatching to look at the plan cannot upload
by accident. A push to `main` publishes for real — that is the point of it.

## Adding a new grammar

Nothing here is per-grammar except one patch. For a new `crates/treebank-<lang>/`:

1. Add a **`patches/NNNN-treebank-crate-identity.patch`** as the last patch in
   the series, following the existing ones. It touches `Cargo.toml` and
   `Cargo.lock` only:
   - `name = "treebank-grammar-<lang>"` — must match the directory suffix, or
     `publish.sh` refuses to publish it;
   - `[lib] name` pinned to upstream's default (upstream's package name with
     dashes as underscores), so the crate stays drop-in;
   - `repository` at this repo, `homepage` at treebank.dev, and a `description`
     saying it is a redistribution;
   - `include` extended with `LICENSE`, `ledger.json`, `LOCAL-PATCHES.md` and
     `patches/*`;
   - the `Cargo.lock` self-entry renamed to match.

   Check two things upstream often gets wrong: that `LICENSE` is in `include`
   at all, and that the `tree-sitter` dev-dependency is `0.25` or later — the
   0.24 runtime cannot load the ABI-15 parsers the pinned CLI generates, so the
   crate's own tests fail against its own parser.

   Record it in `ledger.json` with `"kind": "packaging"` so it is not counted as
   a parser fix.

2. Nothing to do — the verify matrix is derived from what changed, over the
   grammars that exist, so a new `crates/treebank-<lang>/` with a `ledger.json`
   is picked up automatically.

3. Add it to the consumer test: an entry in
   [`tools/consumer-test/grammars.json`](tools/consumer-test/grammars.json)
   naming its dependency alias, crate name and cases, plus the fixtures those
   cases reference, built from the grammar's patch repros. `test-publish.sh`
   refuses to rehearse a grammar with no entry, so this cannot be silently
   skipped — but the fixtures themselves are on you.

   Nothing needs adding to the verify matrix or to `changed-grammars.sh`: both
   enumerate `crates/*/ledger.json`.

`publish.sh` picks it up with no changes: it enumerates `crates/*/ledger.json`.
