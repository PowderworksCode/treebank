#!/usr/bin/env bash
# Exercise the production sweep path without downloading an ecosystem corpus.
# Every language fixture has one valid file and one deliberately invalid file,
# so this covers parser loading, reference-oracle adjudication, reports and the
# generated ledger. Rust additionally runs twice to guard the shared pass cache.
set -euo pipefail

root=$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)
treebank_bin=${TREEBANK_BIN:-"$root/target/debug/treebank"}
lang=${1:?usage: tools/sweep-smoke.sh LANGUAGE}
fixture="$root/test/sweep-smoke/$lang"
scratch=$(mktemp -d "${TMPDIR:-/tmp}/treebank-sweep-smoke.XXXXXX")
trap 'rm -rf "$scratch"' EXIT

if [[ ! -d "$fixture/src/sweep-smoke" ]]; then
  echo "sweep smoke: no fixture for $lang" >&2
  exit 1
fi

grammar_lang=$lang
if [[ "$lang" == javascript ]]; then
  grammar_lang=typescript
fi

grammar="$scratch/grammar"
corpus="$scratch/corpus"
report="$corpus/reports/sweep.json"
cp -R "$root/crates/treebank-$grammar_lang" "$grammar"
cp -R "$fixture" "$corpus"

# Isolate the ledger check from production measurements. The sweep must
# replace this block from its own report; no test command touches the checkout.
printf "language = '%s'\n\n[corpus.sweep]\nfiles = 0\npassed = 0\nfailed = 0\ngap_files = 0\nnoise_files = 0\npass_rate = '0.00%%'\n" \
  "$lang" > "$grammar/ledger.toml"

if command -v sha256sum >/dev/null; then
  sha256() { sha256sum "$1" | cut -d ' ' -f 1; }
else
  sha256() { shasum -a 256 "$1" | cut -d ' ' -f 1; }
fi

files_json=$(
  for file in "$corpus"/src/sweep-smoke/*; do
    jq -n \
      --arg path "${file##*/}" \
      --argjson bytes "$(wc -c < "$file" | tr -d ' ')" \
      --arg sha256 "$(sha256 "$file")" \
      '{path: $path, bytes: $bytes, sha256: $sha256}'
  done | jq -s .
)
jq -n --argjson files "$files_json" \
  '{packages: [{package: "sweep", version: "smoke", downloads: 0, files: $files}]}' \
  > "$corpus/manifest.json"

run_sweep() {
  "$treebank_bin" sweep \
    --lang "$lang" \
    --grammar "$grammar" \
    --manifest "$corpus/manifest.json" \
    --out "$report"
}

run_sweep

jq -e --arg lang "$lang" '
  .lang == $lang and
  .files == 2 and
  .passed == 1 and
  .failed == 1 and
  .gap_files == 0 and
  .noise_files == 1 and
  (.provenance.corpus_lock_sha256 | test("^[0-9a-f]{64}$")) and
  (.provenance.grammar_sha256 | test("^[0-9a-f]{64}$")) and
  (.provenance | has("grammar_revision") | not) and
  (.clusters | length) == 1 and
  .clusters[0].verdict == "noise"
' "$report" >/dev/null
grep -Fq 'files = 2' "$grammar/ledger.toml"
grep -Fq 'passed = 1' "$grammar/ledger.toml"
grep -Fq 'failed = 1' "$grammar/ledger.toml"
grep -Fq 'gap_files = 0' "$grammar/ledger.toml"
grep -Fq 'noise_files = 1' "$grammar/ledger.toml"
grep -Fq "pass_rate = '50.00%'" "$grammar/ledger.toml"
grep -Eq "^corpus_lock_sha256 = '[0-9a-f]{64}'$" "$grammar/ledger.toml"
grep -Eq "^grammar_sha256 = '[0-9a-f]{64}'$" "$grammar/ledger.toml"
# Not `! grep`: errexit ignores a `!`-prefixed command, so that form could
# never fail the smoke test. This one can.
if grep -Fq 'grammar_revision' "$grammar/ledger.toml"; then
  echo "sweep smoke: ledger.toml must not carry grammar_revision" >&2
  exit 1
fi
jq -e '.passed_sha256 | length == 1' "$corpus/sweep-cache.json" >/dev/null

if [[ "$lang" == rust ]]; then
  cp "$report" "$scratch/first-report.json"
  cp "$grammar/ledger.toml" "$scratch/first-ledger.toml"
  run_sweep 2>"$scratch/second-run.stderr"

  # A passing file is cached; a failure is deliberately re-parsed so its
  # diagnostic and oracle verdict can never go stale.
  grep -Fq '(1 unchanged-and-passing, 1 to parse)' "$scratch/second-run.stderr"
  cmp "$report" "$scratch/first-report.json"
  cmp "$grammar/ledger.toml" "$scratch/first-ledger.toml"

  # Hosted canaries may sample a locked corpus whose full adjudication needs
  # more memory than the runner. Sampling must be deterministic and must not
  # replace the authoritative full-corpus ledger block.
  "$treebank_bin" sweep \
    --lang "$lang" \
    --grammar "$grammar" \
    --manifest "$corpus/manifest.json" \
    --out "$report" \
    --limit 1 \
    --no-write-ledger
  jq -e '.files == 1' "$report" >/dev/null
  cmp "$grammar/ledger.toml" "$scratch/first-ledger.toml"
fi

echo "sweep smoke ($lang): OK"
