#!/usr/bin/env python3
"""Compare a swept ledger against its committed form, field by field.

The corpus canary normally enforces evidence with `git diff --exit-code`:
the sweep rewrites `[corpus.*sweep*]`, and any change at all is a failure.
That is the right gate for a grammar whose oracle answers the same way
everywhere, and it stays the default.

It is the wrong gate for C. `treebank-oracle`'s C oracle says so in its own
header — "validity in isolation is not a question C has an answer to. The
verdict is relative to an include environment." Only files the oracle
vouches for become `gap_files`; the rest become `noise_files`. Change the
available system headers and files move between those two buckets without
the grammar or the corpus changing at all.

That is measured, not supposed. On run 33218406274 the C leg reproduced
every quantity the grammar and corpus determine — 84,473 files, 62,539
passed, 21,934 failed, 74.03%, both SHAs identical to the committed ledger
— while `gap_files` read 4,012 against a committed 6,392. A rerun on a
fresh runner reproduced 4,012 again, bit for bit, so the split is perfectly
deterministic per environment and simply is not the environment the
committed evidence came from.

Re-baselining would be the wrong repair: the evidence host adjudicated MORE
files successfully (11,320 noise against the runner's 14,754), so adopting
the runner's numbers would shrink the C gap queue by losing verdicts rather
than by fixing grammar.

So this checks what the inputs actually determine, and reports the rest
instead of either failing on it or hiding it.
"""

import sys
import tomllib

# Everything here is fixed by the corpus bytes and the grammar bytes. If one
# of these moves while the SHAs hold, something is genuinely wrong.
REPRODUCIBLE = (
    "files",
    "passed",
    "failed",
    "pass_rate",
    "corpus_lock_sha256",
    "grammar_sha256",
    "grammar_revision",
)


def sweep_tables(doc):
    """Every `[corpus.sweep]` / `[corpus.<lang>_sweep]` block, by name."""
    corpus = doc.get("corpus", {})
    return {
        f"corpus.{name}": table
        for name, table in corpus.items()
        if isinstance(table, dict) and (name == "sweep" or name.endswith("_sweep"))
    }


def main():
    if len(sys.argv) != 3:
        sys.exit(f"usage: {sys.argv[0]} <committed-ledger> <swept-ledger>")
    committed_path, swept_path = sys.argv[1], sys.argv[2]

    with open(committed_path, "rb") as f:
        committed = sweep_tables(tomllib.load(f))
    with open(swept_path, "rb") as f:
        swept = sweep_tables(tomllib.load(f))

    if not committed:
        sys.exit(f"{committed_path}: no [corpus.*sweep*] block to compare")
    if committed.keys() != swept.keys():
        sys.exit(
            f"sweep blocks changed: committed {sorted(committed)}, "
            f"swept {sorted(swept)}"
        )

    failures, drift = [], []
    for block, before in committed.items():
        after = swept[block]
        for key in REPRODUCIBLE:
            if key in before and before[key] != after.get(key):
                failures.append(
                    f"{block}.{key}: committed {before[key]!r}, swept "
                    f"{after.get(key)!r}"
                )
        for key in sorted(set(before) | set(after)):
            if key not in REPRODUCIBLE and before.get(key) != after.get(key):
                drift.append(
                    f"{block}.{key}: committed {before.get(key)!r}, swept "
                    f"{after.get(key)!r}"
                )

    # Reported every run, pass or fail. The whole point of narrowing the gate
    # is to stop pretending these numbers are portable; hiding them would
    # instead pretend they do not exist.
    if drift:
        print("canary: adjudication-derived fields differ from the committed")
        print("canary: ledger. These depend on the oracle's include environment,")
        print("canary: so they are reported and not enforced:")
        for line in drift:
            print(f"canary:   {line}")
    else:
        print("canary: adjudication-derived fields match the committed ledger too")

    if failures:
        print("::error::corpus evidence the inputs determine does not match")
        for line in failures:
            print(f"  {line}")
        print(
            "The corpus lock and grammar SHAs fix these values. A difference "
            "here is a real divergence, not an environment difference."
        )
        return 1

    print("canary: every input-determined field matches the committed ledger")
    return 0


if __name__ == "__main__":
    sys.exit(main())
