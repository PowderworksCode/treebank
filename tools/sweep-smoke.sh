#!/usr/bin/env bash
# Exercise the real sweep path without downloading an ecosystem corpus.
# The invalid file is intentional: it makes the Rust oracle adjudicate a
# rejection as corpus noise, while the valid file proves and then exercises
# the incremental passing-file cache on the second run.
set -euo pipefail

root=$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)
treebank_bin=${TREEBANK_BIN:-"$root/target/debug/treebank"}
fixture="$root/test/sweep-smoke/rust"
scratch=$(mktemp -d "${TMPDIR:-/tmp}/treebank-sweep-smoke.XXXXXX")
trap 'rm -rf "$scratch"' EXIT

grammar="$scratch/grammar"
corpus="$scratch/corpus"
report="$corpus/reports/sweep.json"
cp -R "$root/crates/treebank-rust" "$grammar"
cp -R "$fixture" "$corpus"
cp "$fixture/ledger.before.toml" "$grammar/ledger.toml"

run_sweep() {
  "$treebank_bin" sweep \
    --lang rust \
    --grammar "$grammar" \
    --manifest "$corpus/manifest.json" \
    --out "$report"
}

run_sweep

jq -e '
  .lang == "rust" and
  .files == 2 and
  .passed == 1 and
  .failed == 1 and
  .gap_files == 0 and
  .noise_files == 1 and
  (.clusters | length) == 1 and
  .clusters[0].verdict == "noise"
' "$report" >/dev/null
cmp "$grammar/ledger.toml" "$fixture/ledger.expected.toml"
jq -e '.passed_sha256 | length == 1' "$corpus/sweep-cache.json" >/dev/null

cp "$report" "$scratch/first-report.json"
cp "$grammar/ledger.toml" "$scratch/first-ledger.toml"
run_sweep 2>"$scratch/second-run.stderr"

# A passing file is cached; a failure is deliberately re-parsed so its
# diagnostic and oracle verdict can never go stale.
grep -Fq '(1 unchanged-and-passing, 1 to parse)' "$scratch/second-run.stderr"
cmp "$report" "$scratch/first-report.json"
cmp "$grammar/ledger.toml" "$scratch/first-ledger.toml"

echo "sweep smoke: OK"
