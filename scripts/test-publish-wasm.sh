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
#   4. asserts a re-run releases nothing, because everything is tagged;
#   5. asserts a forced second release lands on -treebank.2, so the suffix
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

# Which fixture proves which grammar. Drawn from tools/consumer-test, so the
# wasm path is checked against the same repros the crate path is.
fixture_for() {
  case "$1" in
    treebank-c)          echo patched.c ;;
    treebank-csharp)     echo patched.cs ;;
    treebank-java)       echo patched.java ;;
    treebank-javascript) echo patched.js ;;
    treebank-python)     echo patched.py ;;
    treebank-rust)       echo patched.rs ;;
    treebank-typescript) echo patched.ts ;;
    treebank-tsx)        echo patched.tsx ;;
    *)                   echo "" ;;
  esac
}
negative_for() {
  case "$1" in
    treebank-python) echo must-reject.py ;;
    treebank-rust)   echo must-reject.rs ;;
    *)               echo "" ;;
  esac
}

fail=0
say() { printf '%s\n' "$*"; }

say "=== 1. release every pack to a staging directory ==="
scripts/publish-wasm.sh --dry-run --tag-prefix "$PREFIX" --out "$STAGE/rel" "${TARGETS[@]}" \
  | grep -E '^(=|  (staged|version|change check|dry run|released|skipped))' || true
# --dry-run stages but does not tag; the rehearsal needs the tags to test the
# skip, so create them here from what was staged.
for d in "$STAGE/rel"/*/; do git tag -a "$PREFIX$(basename "$d")" -m rehearsal >/dev/null; done

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

  for w in "$STAGE/consume/$relname"/*.wasm; do
    pack=$(basename "$w" .wasm)
    fx=$(fixture_for "$pack")
    [ -n "$fx" ] || { say "    FAIL: $pack has no fixture in test-publish-wasm.sh"; fail=1; continue; }
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
    neg=$(negative_for "$pack")
    if [ -n "$neg" ]; then
      out=$("${PYTHON:-python3}" tools/wasm-pack/examples/parse.py "$w" "tools/consumer-test/fixtures/$neg" 2>&1)
      if grep -qE "$neg: [0-9]+ error" <<<"$out"; then
        say "    $pack: $neg still rejected"
      else
        say "    FAIL: $pack accepted $neg — the negative corpus did not survive the wasm build"; fail=1
      fi
    fi
  done
done

say ""
say "=== 4. a re-run releases nothing ==="
again=$(scripts/publish-wasm.sh --dry-run --skip-materialize --tag-prefix "$PREFIX" --out "$STAGE/rel2" "${TARGETS[@]}" 2>&1 || true)
if grep -q "released:" <<<"$again"; then
  say "  FAIL: a second run wanted to release something"; grep "released:" <<<"$again" | sed 's/^/    /'; fail=1
else
  say "  ok — every pack skipped: $(grep -c 'skipped:' <<<"$again") of ${#TARGETS[@]}"
fi

say ""
say "=== 5. a forced re-release lands on -treebank.2 ==="
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
