# Corpus locks

This directory holds the small, committable identity of Treebank's large,
gitignored corpora. There is one `<language>.json` lock per corpus once that
language has had a fresh full fetch.

Generate or deliberately update a lock with:

```sh
cargo run -p treebank-cli -- fetch --lang <language> \
  --lock-out corpus-locks/<language>.json
```

Review lock changes as evidence changes: package versions, archive digests,
and admitted file digests all affect the sweep population. Do not hand-edit a
lock or copy an old `corpus/<language>/manifest.json`; manifests from before
archive provenance was recorded cannot recreate their inputs.

Recreate an exact corpus on a clean machine with:

```sh
cargo run -p treebank-cli -- hydrate --lang <language>
```

`hydrate` does not resolve “latest.” It downloads each recorded URL, verifies
the archive byte count and SHA-256, extracts through the language's normal
admission rules, and verifies that the resulting file set has no missing,
extra, or changed entries. Only then does it publish `corpus/<language>/src`
and its `manifest.json`.

`rust.json` is the first release canary: 1,000 pinned crates and 27,307 source
files. The weekly and manually dispatchable `corpus-canary.yml` workflow
hydrates it from scratch, performs the production sweep, and requires the
generated `[corpus.sweep]` block to remain byte-for-byte equal to the committed
Rust ledger. A grammar or oracle change that improves or regresses the result
therefore needs an explicit, reviewed evidence update.
