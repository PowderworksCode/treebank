#!/usr/bin/env bash
# Rehearse the whole wasm-pack release path without publishing anything.
#
# scripts/publish-wasm.sh can be checked up to the moment it uploads and no
# further. Everything that only happens AFTER a successful release — the tag,
# the skip on a re-run, the suffix increment, and a consumer actually fetching
# the assets over HTTP and parsing with them — would otherwise be untestable,
# which is exactly the part that breaks. This closes that gap, the way
# scripts/test-publish.sh does for crates.io.
#
# It:
#   1. releases every pack to a staging directory, tagged under rehearsal/;
#   2. serves that directory over localhost, so consumers fetch by URL rather
#      than reading the build tree — the same thing a real consumer does;
#   3. verifies SHA256SUMS, then runs BOTH example consumers (Python/wasmtime
#      and Node/WASI) against the fetched packs, asserting that each grammar's
#      patch-repro fixture parses clean and that the negative fixture is still
#      rejected. A pack that parses the repro is positive evidence the patch
#      series survived the wasm build;
#   4. asserts packs.json lists every pack built, with a sha256 that matches
#      the artifact and a URL naming the real tag rather than the rehearsal one;
#   5. asserts a re-run releases nothing, because everything is tagged;
#   6. asserts a forced second release lands on -treebank.2, so the suffix
#      really is derived rather than assumed.
#
# Tags go under rehearsal/ — its own namespace. Production tags at the same
# version would otherwise make this skip instead of release, quietly testing
# nothing. They are deleted on exit.
#
# Usage: scripts/test-publish-wasm.sh [grammar-dir ...]
set -euo pipefail

ROOT=$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)
cd "$ROOT"
# Distinct from scripts/test-publish.sh's "rehearsal/" namespace, and cleaned
# with a glob that cannot reach it. Git tags are shared by every worktree of
# this repo, so two sessions rehearsing at once share one tag namespace.
PREFIX="rehearsal-wasm/"
STAGE=$(mktemp -d)
PORT=0
SRV_PID=""

cleanup() {
  [ -n "$SRV_PID" ] && kill "$SRV_PID" 2>/dev/null || true
  while IFS= read -r t; do [ -n "$t" ] && git tag -d "$t" >/dev/null; done < <(git tag --list "rehearsal-wasm/*")
  rm -rf "$STAGE"
}
trap cleanup EXIT

TARGETS=("$@")
if [ "${#TARGETS[@]}" -eq 0 ]; then
  for l in crates/*/ledger.json; do TARGETS+=("$(dirname "$l")"); done
fi

# Which fixture proves which grammar comes from tools/consumer-test/grammars.json
# — the same file the crate rehearsal reads. Duplicating it here would mean
# adding a language in two places and finding out about the second one when a
# release went out untested.
#
# The mapping from a case to a PACK: a grammar directory that generates one
# parser owns all its cases; one that generates several (typescript ->
# typescript + tsx, php -> php + php_only) splits them by label, which is the
# generate_dir's basename. `expect` says which direction the case proves:
# "Clean" is a patch repro, "Error" is the negative corpus.
GRAMMARS_JSON=tools/consumer-test/grammars.json

cases_for() { # <grammar-dir-name> <pack-name> <n-generate-dirs> <expect>
  local grammar=$1 pack=$2 n=$3 expect=$4 label=${2#treebank-}
  if [ "$n" -le 1 ]; then
    jq -r --arg g "$grammar" --arg e "$expect" \
      '.[] | select(.grammar==$g) | .cases[] | select(.expect==$e) | .fixture' "$GRAMMARS_JSON"
  else
    jq -r --arg g "$grammar" --arg e "$expect" --arg l "$label" \
      '.[] | select(.grammar==$g) | .cases[]
       | select(.expect==$e)
       | select(.label==$l or (.label|startswith($l+" ")))
       | .fixture' "$GRAMMARS_JSON"
  fi
}

fail=0
say() { printf '%s\n' "$*"; }

say "=== 1. release every pack to a staging directory ==="
scripts/publish-wasm.sh --dry-run --tag-prefix "$PREFIX" --out "$STAGE/rel" "${TARGETS[@]}" \
  | grep -E '^(=|  (staged|version|change check|dry run|released|skipped))' || true
# --dry-run stages but does not tag; the rehearsal needs the tags to test the
# skip, so create them here from what was staged.
for d in "$STAGE/rel"/*/; do git tag -a "$PREFIX$(basename "$d")" -m rehearsal >/dev/null; done
# publish-wasm.sh builds the index from the tags that exist, and --dry-run
# creates none — so in a real dry run the index is legitimately empty. Rebuild
# it here now the rehearsal's tags are in place; wasm-index.sh is the thing
# under test either way.
scripts/wasm-index.sh --staged "$STAGE/rel" --offline --tag-prefix "$PREFIX" > "$STAGE/rel/packs.json"

say ""
say "=== 2. serve the staged assets over localhost ==="
PORT=$(python3 -c 'import socket;s=socket.socket();s.bind(("127.0.0.1",0));print(s.getsockname()[1]);s.close()')
(cd "$STAGE/rel" && exec python3 -m http.server "$PORT" --bind 127.0.0.1 >/dev/null 2>&1) &
SRV_PID=$!
for _ in $(seq 50); do curl -sf "http://127.0.0.1:$PORT/" >/dev/null && break; sleep 0.1; done
say "  serving $STAGE/rel on http://127.0.0.1:$PORT"

say ""
say "=== 3. fetch by URL, verify hashes, parse with both consumers ==="
mkdir -p "$STAGE/consume"
for d in "$STAGE/rel"/*/; do
  relname=$(basename "$d")
  ( cd "$STAGE/consume" && rm -rf "$relname" && mkdir "$relname" && cd "$relname"
    curl -sfO "http://127.0.0.1:$PORT/$relname/SHA256SUMS"
    while read -r _ f; do curl -sfO "http://127.0.0.1:$PORT/$relname/$f"; done < SHA256SUMS
    sha256sum -c SHA256SUMS >/dev/null ) || { say "  FAIL: $relname did not fetch/verify"; fail=1; continue; }
  say "  $relname: fetched, SHA256SUMS ok"

  # Which grammar directory produced this release, and how many parsers it
  # generates — needed to split cases across packs.
  grammar_dir=""
  for t in "${TARGETS[@]}"; do
    [ "$(basename "$t")" = "${relname%-v*}" ] && grammar_dir=$t
  done
  [ -n "$grammar_dir" ] || grammar_dir="crates/${relname%-v*}"
  ngen=$(jq -r '(.generate_dirs // ["."]) | length' "$grammar_dir/ledger.json" 2>/dev/null || echo 1)
  gname=$(basename "$grammar_dir")

  for w in "$STAGE/consume/$relname"/*.wasm; do
    pack=$(basename "$w" .wasm)

    # The pack must agree with the ledger about what it is. This is the check
    # that scales: it needs no per-language fixture, and it catches a
    # mis-detected entry point or a stale artifact — a pack built from the
    # wrong grammar still parses SOMETHING cleanly.
    prov="$STAGE/consume/$relname/$pack.json"
    want_sha=$(jq -r .upstream.sha "$grammar_dir/ledger.json")
    got_sha=$(jq -r .upstream.sha "$prov")
    got_lang=$("${PYTHON:-python3}" tools/wasm-pack/examples/parse.py "$w" 2>/dev/null | sed -n '1s/.*language=\([^ ]*\).*/\1/p')
    want_lang=$(jq -r .grammar "$grammar_dir/ledger.json")
    [ "$ngen" -gt 1 ] && want_lang=${pack#treebank-}
    # Compared with separators removed. The name a grammar gives itself and the
    # name treebank's directory gives it are allowed to differ in punctuation —
    # upstream's C# grammar is "c_sharp" where the directory is "csharp", the
    # same split the crate has between treebank-grammar-csharp and the
    # tree_sitter_c_sharp library. What this must still catch is a pack built
    # from the WRONG grammar, and for a multi-parser directory the wrong one of
    # its parsers, which upstream.sha alone cannot see.
    norm() { printf '%s' "${1//[^a-zA-Z0-9]/}" | tr 'A-Z' 'a-z'; }
    if [ "$got_sha" != "$want_sha" ]; then
      say "    FAIL: $pack provenance upstream.sha $got_sha != ledger $want_sha"; fail=1
    elif [ "$(norm "$got_lang")" != "$(norm "$want_lang")" ]; then
      say "    FAIL: $pack reports language '$got_lang', ledger says '$want_lang'"; fail=1
    else
      say "    $pack: provenance agrees with ledger (${got_lang}, ${want_sha:0:12})"
    fi

    mapfile -t positives < <(cases_for "$gname" "$pack" "$ngen" Clean)
    if [ "${#positives[@]}" -eq 0 ]; then
      say "    FAIL: $pack has no \"Clean\" case in $GRAMMARS_JSON"
      say "          add one, as PUBLISHING.md requires for the crate rehearsal"
      fail=1
    fi
    for fx in "${positives[@]}"; do
      for consumer in python node; do
        case "$consumer" in
          python) out=$("${PYTHON:-python3}" tools/wasm-pack/examples/parse.py "$w" "tools/consumer-test/fixtures/$fx" 2>&1) ;;
          node)   out=$(node --no-warnings tools/wasm-pack/examples/parse.mjs "$w" "tools/consumer-test/fixtures/$fx" 2>&1) ;;
        esac
        if grep -q "$fx: clean" <<<"$out"; then
          say "    $pack via $consumer: $fx clean"
        else
          say "    FAIL: $pack via $consumer did not parse $fx cleanly"; printf '%s\n' "$out" | sed 's/^/      /'; fail=1
        fi
      done
    done
    while IFS= read -r neg; do
      [ -n "$neg" ] || continue
      out=$("${PYTHON:-python3}" tools/wasm-pack/examples/parse.py "$w" "tools/consumer-test/fixtures/$neg" 2>&1)
      if grep -qE "$neg: [0-9]+ error" <<<"$out"; then
        say "    $pack: $neg still rejected"
      else
        say "    FAIL: $pack accepted $neg — the negative corpus did not survive the wasm build"; fail=1
      fi
    done < <(cases_for "$gname" "$pack" "$ngen" Error)
  done
done

say ""
say "=== 4. the index lists every pack, with hashes that match ==="
IDX="$STAGE/rel/packs.json"
if [ ! -f "$IDX" ]; then
  say "  FAIL: no packs.json was generated"; fail=1
else
  n_idx=$(jq '.packs|length' "$IDX")
  n_built=$(ls "$STAGE/rel"/*/*.wasm 2>/dev/null | wc -l)
  if [ "$n_idx" != "$n_built" ]; then
    say "  FAIL: index lists $n_idx packs, $n_built were built"; fail=1
  else
    say "  ok — $n_idx packs listed"
  fi
  # Every entry must carry the hash of the artifact it points at. An index whose
  # hashes are stale or absent is worse than no index: it looks verifiable.
  while IFS=$'\t' read -r pack sha url; do
    built=$(ls "$STAGE/rel"/*/"$pack.wasm" 2>/dev/null | head -1)
    if [ -z "$built" ]; then
      say "  FAIL: index names $pack, which was not built"; fail=1
    elif [ "$sha" = "null" ]; then
      say "  FAIL: index entry for $pack has no sha256"; fail=1
    elif [ "$sha" != "$(sha256sum "$built" | cut -d' ' -f1)" ]; then
      say "  FAIL: index sha256 for $pack does not match the artifact"; fail=1
    elif [[ "$url" != https://github.com/*/releases/download/* ]]; then
      say "  FAIL: index url for $pack looks wrong: $url"; fail=1
    fi
  done < <(jq -r '.packs[] | [.pack, (.sha256//"null"), .urls.wasm] | @tsv' "$IDX")
  # The index must name the REAL tag, never the rehearsal namespace, or a
  # rehearsal would publish URLs nobody can fetch.
  if grep -q 'rehearsal' "$IDX"; then
    say "  FAIL: index leaked the rehearsal tag prefix into its URLs"; fail=1
  else
    say "  ok — hashes match the built artifacts and URLs name real tags"
  fi
fi

say ""
say "=== 5. a re-run releases nothing ==="
again=$(scripts/publish-wasm.sh --dry-run --skip-materialize --tag-prefix "$PREFIX" --out "$STAGE/rel2" "${TARGETS[@]}" 2>&1 || true)
if grep -q "released:" <<<"$again"; then
  say "  FAIL: a second run wanted to release something"; grep "released:" <<<"$again" | sed 's/^/    /'; fail=1
else
  say "  ok — every pack skipped: $(grep -c 'skipped:' <<<"$again") of ${#TARGETS[@]}"
fi

say ""
say "=== 6. a forced re-release lands on -treebank.2 ==="
one=("${TARGETS[0]}")
forced=$(scripts/publish-wasm.sh --dry-run --skip-materialize --force --tag-prefix "$PREFIX" --out "$STAGE/rel3" "${one[@]}" 2>&1 || true)
if grep -qE 'version: .*-treebank\.2' <<<"$forced"; then
  say "  ok — $(grep -oE 'version: [^ ]+ ' <<<"$forced" | head -1)"
else
  say "  FAIL: forced re-release did not compute -treebank.2"; grep -E 'version:' <<<"$forced" | sed 's/^/    /'; fail=1
fi

say ""
if [ "$fail" = 0 ]; then say "test-publish-wasm: ok — nothing was published"; else say "test-publish-wasm: FAILED" >&2; fi
exit "$fail"
