#!/usr/bin/env bash
# Fetch every pack over HTTP and parse with it, from both example consumers.
#
# What this protects is the boundary, not the transport: a pack that loads
# under wasmtime in check.sh can still be wrong at the ABI, and the two
# example bindings are the only things that exercise it the way a consumer
# does. They are also the reference the browser binding was ported from, so
# if they break, the site breaks with them.
#
# Served over localhost and fetched by URL rather than read out of the build
# tree, because that is what a real consumer does -- the site fetches packs
# from R2 exactly this way -- and because reading the build tree would prove
# nothing about the bytes that travel.
#
# The fixtures are the sweep-smoke pair, which also drive the native smoke, so
# this proves crossing the wasm boundary preserves BOTH acceptance and
# rejection for every shipped grammar. A pack that accepts everything would
# pass a valid-only check.
set -euo pipefail

ROOT=$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)
cd "$ROOT"
PY=${TREEBANK_WASM_PYTHON:-python3}
OUT=${TREEBANK_WASM_OUT:-dist/wasm}
STAGE=$(mktemp -d)
SRV_PID=""

cleanup() {
  [ -n "$SRV_PID" ] && kill "$SRV_PID" 2>/dev/null || true
  rm -rf "$STAGE"
}
trap cleanup EXIT

fail() { echo "test-consumers: FAIL — $*" >&2; exit 1; }

GRAMMARS=("$@")
if [ ${#GRAMMARS[@]} -eq 0 ]; then
  while IFS= read -r grammar; do
    GRAMMARS+=("$grammar")
  done < <(./tools/wasm-pack/list-grammars.sh)
fi
[ ${#GRAMMARS[@]} -gt 0 ] || fail "no grammars discovered"

fixture() {
  local lang=$1 kind=$2
  local matches
  case $kind in
    valid) matches=("test/sweep-smoke/$lang/src/sweep-smoke/"[Vv]alid.*) ;;
    invalid) matches=("test/sweep-smoke/$lang/src/sweep-smoke/"[Ii]nvalid.*) ;;
    *) fail "unknown fixture kind: $kind" ;;
  esac
  if [ ${#matches[@]} -ne 1 ] || [ ! -f "${matches[0]}" ]; then
    fail "$lang: expected exactly one $kind sweep-smoke fixture"
  fi
  printf '%s\n' "${matches[0]}"
}

for lang in "${GRAMMARS[@]}"; do
  [ -f "$OUT/treebank-$lang.wasm" ] || ./tools/wasm-pack/build.sh "$lang" --out "$OUT" >/dev/null
  cp "$OUT/treebank-$lang.wasm" "$STAGE/"
done
( cd "$STAGE" && sha256sum ./*.wasm > SHA256SUMS )

PORT=$("$PY" -c 'import socket;s=socket.socket();s.bind(("127.0.0.1",0));print(s.getsockname()[1]);s.close()')
( cd "$STAGE" && "$PY" -m http.server "$PORT" --bind 127.0.0.1 >/dev/null 2>&1 ) &
SRV_PID=$!
for _ in $(seq 50); do curl -sf "http://127.0.0.1:$PORT/" >/dev/null && break; sleep 0.1; done

FETCHED=$(mktemp -d "$STAGE/fetched.XXXX")
curl -sf "http://127.0.0.1:$PORT/SHA256SUMS" -o "$FETCHED/SHA256SUMS" \
  || fail "could not fetch SHA256SUMS over HTTP"

for lang in "${GRAMMARS[@]}"; do
  curl -sf "http://127.0.0.1:$PORT/treebank-$lang.wasm" -o "$FETCHED/treebank-$lang.wasm" \
    || fail "$lang: could not fetch the pack over HTTP"

  # Against the FETCHED bytes, not the built ones: the point is what travels.
  ( cd "$FETCHED" && grep "treebank-$lang.wasm\$" SHA256SUMS | sha256sum -c - >/dev/null ) \
    || fail "$lang: sha256 mismatch after fetch"

  neg=$(fixture "$lang" invalid)
  valid=$(fixture "$lang" valid)
  out_py=$("$PY" tools/wasm-pack/examples/parse.py "$FETCHED/treebank-$lang.wasm" "$neg")
  out_js=$(node tools/wasm-pack/examples/parse.mjs "$FETCHED/treebank-$lang.wasm" "$neg")
  echo "$out_py" | grep -q "error(s)" || fail "$lang: python consumer did not reject the negative fixture"
  echo "$out_js" | grep -q "error(s)" || fail "$lang: node consumer did not reject the negative fixture"

  ok_py=$("$PY" tools/wasm-pack/examples/parse.py "$FETCHED/treebank-$lang.wasm" "$valid")
  ok_js=$(node tools/wasm-pack/examples/parse.mjs "$FETCHED/treebank-$lang.wasm" "$valid")
  echo "$ok_py" | grep -q "clean" || fail "$lang: python consumer did not accept the valid fixture"
  echo "$ok_js" | grep -q "clean" || fail "$lang: node consumer did not accept the valid fixture"
done

echo "test-consumers: OK — ${#GRAMMARS[@]} packs fetched over HTTP, checksummed, parsed by both consumers"
