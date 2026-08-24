#!/usr/bin/env python3
"""Regenerate corpus/rust/top-k.json from the crates.io db dump CSVs.

Ranks every crate by total downloads, resolves its default version, and
writes the same {rank, name, version, downloads} shape the fetch step
expects. Run it whenever the db dump is refreshed:

    python3 tools/make-rust-topk.py [--limit 5000]
"""

import argparse
import csv
import json
import os
import sys

csv.field_size_limit(sys.maxsize)

CORPUS = os.path.join(os.path.dirname(__file__), "..", "corpus", "rust", "db")


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--limit", type=int, default=5000)
    args = ap.parse_args()

    downloads = {}  # crate_id -> total downloads
    with open(os.path.join(CORPUS, "crate_downloads.csv")) as fh:
        for row in csv.DictReader(fh):
            downloads[row["crate_id"]] = int(row["downloads"])

    names = {}  # crate_id -> name
    with open(os.path.join(CORPUS, "crates.csv")) as fh:
        for row in csv.DictReader(fh):
            names[row["id"]] = row["name"]

    default_version = {}  # crate_id -> version num
    with open(os.path.join(CORPUS, "default_versions.csv")) as fh:
        for row in csv.DictReader(fh):
            default_version[row["crate_id"]] = row["version_id"]

    version_num = {}  # version id -> num
    with open(os.path.join(CORPUS, "versions.csv")) as fh:
        for row in csv.DictReader(fh):
            version_num[row["id"]] = row["num"]

    ranked = sorted(
        ((dl, cid) for cid, dl in downloads.items() if dl > 0 and cid in names),
        reverse=True,
    )[: args.limit]

    out = []
    for rank, (dl, cid) in enumerate(ranked, 1):
        vid = default_version.get(cid, "")
        out.append({
            "rank": rank,
            "name": names[cid],
            "version": version_num.get(vid, ""),
            "downloads": dl,
        })

    dest = os.path.join(CORPUS, "..", "top-k.json")
    with open(dest, "w") as fh:
        json.dump(out, fh, indent=1)
    print(f"wrote {dest}: {len(out)} crates (top: {out[0]['name']})")


if __name__ == "__main__":
    sys.exit(main())
