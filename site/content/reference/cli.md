---
title: CLI
description: All twenty-one treebank subcommands, grouped by what they are for.
order: 10
---

`treebank <command>`. Every command is offline unless it says otherwise.

## Building a corpus

| command | what it does |
| --- | --- |
| `rank` | Build the top-K package list for a language's ecosystem |
| `fetch` | Download package tarballs, extract source files, write the manifest |
| `hydrate` | Recreate and verify the exact corpus pinned by a committed lock |
| `oracle` | Run a language's reference parser over paths on stdin, writing `<path>\tvalid\|invalid`. This is `Lang::validate` and nothing else — the same call `sweep` adjudicates failures with |

## Measuring a grammar against it

| command | what it does |
| --- | --- |
| `sweep` | Parse the corpus, adjudicate failures with the reference parser, write `sweep.json` and an agent-ready `REPORT.md` |
| `shape` | Compare our node boundaries against the reference parser's. Catches silent mis-parses: files that parse cleanly and build the wrong tree |
| `errors` | When the grammar rejects a file, does it reject in the right place? Compares our first `ERROR` node against the reference's first error |
| `fuzz` | Derive programs *from* the grammar and ask the oracle whether they are in the language. Failures arrive shrunk |
| `mutate` | Mutate corpus files and ask whether the grammar accepts things the language does not |
| `reformat` | Reformat every corpus file with the language's own formatter and assert our tree is unchanged |
| `roundtrip` | Re-render every file through the language's own printer and reparse it. Finds constructs handled in the spelling people write and not the one the toolchain emits |
| `incremental` | Parse, edit, reparse incrementally, and compare against a fresh parse of the edited text |
| `recovery` | Delete one token from a clean file and measure how much of it lands inside an `ERROR` |
| `kinds` | Count node kinds over the corpus and report which ones real code never produces |

## Checking a grammar structurally

| command | what it does |
| --- | --- |
| `terms` | Vocabulary conformance: declared supertypes from the closed structural list, total node coverage, required containments, a valid `terms.json` |
| `lint` | The structural smells the field guide names — declared-conflict growth, early commits between parallel tiers, same-text token splits, unreserved keywords, scanner drift, parse-table growth — against the grammar's `lint_policy.toml` where one exists, advisory where none does |
| `negative` | Assert that every file in a directory *fails* to parse |
| `rosetta` | The same program in every owned language must yield the same role counts |
| `verify` | Run every gate a grammar must pass, in one command |

## Reporting

| command | what it does |
| --- | --- |
| `status` | Every language's configuration, evidence and test coverage in one inventory. `--check` fails on missing or contradictory required configuration; `--github` adds live issues, pull requests, workflows and branch protection |

## The one you will run most

```sh
treebank status --check
```

It is the inventory: which grammars exist, what their corpus pass rates are,
how many gaps each has, what test coverage is checked in, whether the evidence
is current or stale, and which optional coverage is still missing. Warnings
stay visible without pretending optional coverage is broken.
