# Corpus locks

This directory holds the small, committable identity of Treebank's large,
gitignored corpora. There is one `<language>.json` lock per corpus once that
language has had a fresh full fetch.

Generate or deliberately update a lock without retaining the full corpus with:

```sh
cargo run -p treebank-cli -- fetch --lang <language> \
  --lock-out corpus-locks/<language>.json --lock-only
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

All ten registered languages have a lock. A lock pins inputs; it does not claim
that CI sweeps the full population.

| language | pinned packages | admitted source files |
|---|---:|---:|
| Bash | 588 | 77,205 |
| C | 100 | 84,473 |
| C++ | 97 | 121,957 |
| Java | 889 | 243,576 |
| JavaScript | 1,000 | 25,300 |
| Python | 1,915 | 298,354 |
| Ruby | 120 | 6,487 |
| Rust | 1,000 | 27,307 |
| TypeScript | 1,000 | 11,991 |
| Zig | 6 | 17,168 |

Package counts are what the fetch actually admitted, not the ranking limit:
packages with no matching source distribution or no admitted files are absent,
as are Debian source archives over the ecosystem's safety cap.

`rust.json` remains the release canary: 1,000 pinned crates and 27,307 source
files. The weekly and manually
dispatchable `corpus-canary.yml` workflow hydrates it from scratch, performs
the production sweep, and requires the generated `[corpus.sweep]` block to
remain byte-for-byte equal to the committed Rust ledger. A grammar or oracle
change that improves or regresses the result therefore needs an explicit,
reviewed evidence update.
