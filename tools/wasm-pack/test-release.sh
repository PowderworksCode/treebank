#!/usr/bin/env bash
# Rehearse the whole release path without publishing anything.
#
# release.sh can be checked up to the moment it uploads and no further.
# Everything that only happens AFTER a successful release — the tag, the skip
# on a re-run, and a consumer actually FETCHING the assets over HTTP and
# parsing with them — would otherwise be untestable, which is exactly the
# part that breaks.
#
# It:
#   1. stages every pack, tagged under rehearsal-wasm/;
#   2. serves the staging directory over localhost, so consumers fetch by URL
#      rather than reading the build tree — what a real consumer does;
#   3. verifies SHA256SUMS against the FETCHED bytes, then runs BOTH example
#      consumers against them, asserting each grammar's sweep-smoke valid
#      fixture parses clean and its invalid fixture is still rejected;
#   4. asserts packs.json lists every pack, with a sha256 matching the
#      artifact and a URL naming the real tag rather than the rehearsal one;
#   5. asserts a re-run releases nothing, because everything is tagged.
#
# Tags go under rehearsal-wasm/ — its own namespace. Production tags at the
# same version would otherwise make step 5 pass by skipping, quietly testing
# nothing. They are deleted on exit.
set -euo pipefail

ROOT=$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)
cd "$ROOT"
PY=${TREEBANK_WASM_PYTHON:-python3}
PREFIX="rehearsal-wasm/"
STAGE=$(mktemp -d)
SRV_PID=""

cleanup() {
  [ -n "$SRV_PID" ] && kill "$SRV_PID" 2>/dev/null || true
  git tag --list "$PREFIX*" | while read -r t; do git tag -d "$t" >/dev/null; done
  rm -rf "$STAGE"
}
trap cleanup EXIT

fail() { echo "test-release: FAIL — $*" >&2; exit 1; }

GRAMMARS=()
while IFS= read -r grammar; do
  GRAMMARS+=("$grammar")
done < <(./tools/wasm-pack/list-grammars.sh)
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

# ---- 1. stage, and tag as a real release would -------------------------
# Deliberately pass no grammar arguments: this rehearses release.sh's
# discovery-driven default, not merely the same list copied into its caller.
TREEBANK_WASM_PYTHON="$PY" ./tools/wasm-pack/release.sh --stage "$STAGE" --tag-prefix "$PREFIX" >/dev/null
for lang in "${GRAMMARS[@]}"; do
  v=$(grep -m1 '^version = ' "crates/treebank-$lang/Cargo.toml" | cut -d'"' -f2)
  [ -d "$STAGE/treebank-$lang-v$v" ] || fail "$lang: nothing staged"
  git tag "$PREFIX""treebank-$lang-v$v"
done

# ---- 2. serve it, so consumers fetch by URL ----------------------------
PORT=$("$PY" -c 'import socket;s=socket.socket();s.bind(("127.0.0.1",0));print(s.getsockname()[1]);s.close()')
( cd "$STAGE" && "$PY" -m http.server "$PORT" --bind 127.0.0.1 >/dev/null 2>&1 ) &
SRV_PID=$!
for _ in $(seq 50); do curl -sf "http://127.0.0.1:$PORT/" >/dev/null && break; sleep 0.1; done

FETCHED=$(mktemp -d "$STAGE/fetched.XXXX")
for lang in "${GRAMMARS[@]}"; do
  v=$(grep -m1 '^version = ' "crates/treebank-$lang/Cargo.toml" | cut -d'"' -f2)
  rel="treebank-$lang-v$v"
  mkdir -p "$FETCHED/$rel"
  for asset in "treebank-$lang.wasm" "treebank-$lang.json" "treebank-$lang.roles.json" SHA256SUMS; do
    curl -sf "http://127.0.0.1:$PORT/$rel/$asset" -o "$FETCHED/$rel/$asset" \
      || fail "$lang: could not fetch $asset over HTTP"
  done

  # ---- 3. checksums, against the FETCHED bytes ------------------------
  ( cd "$FETCHED/$rel" && sha256sum -c SHA256SUMS >/dev/null ) || fail "$lang: SHA256SUMS mismatch after fetch"

  # Both consumers, on the fetched pack. The fixtures also drive the native
  # sweep smoke, so this proves that crossing the wasm boundary preserves
  # both acceptance and rejection for every shipped grammar.
  neg=$(fixture "$lang" invalid)
  valid=$(fixture "$lang" valid)
  out_py=$("$PY" tools/wasm-pack/examples/parse.py "$FETCHED/$rel/treebank-$lang.wasm" "$neg")
  out_js=$(node tools/wasm-pack/examples/parse.mjs "$FETCHED/$rel/treebank-$lang.wasm" "$neg")
  echo "$out_py" | grep -q "error(s)" || fail "$lang: python consumer did not reject the negative fixture"
  echo "$out_js" | grep -q "error(s)" || fail "$lang: node consumer did not reject the negative fixture"

  ok_py=$("$PY" tools/wasm-pack/examples/parse.py "$FETCHED/$rel/treebank-$lang.wasm" "$valid")
  ok_js=$(node tools/wasm-pack/examples/parse.mjs "$FETCHED/$rel/treebank-$lang.wasm" "$valid")
  echo "$ok_py" | grep -q "clean" || fail "$lang: python consumer did not accept the valid fixture"
  echo "$ok_js" | grep -q "clean" || fail "$lang: node consumer did not accept the valid fixture"
done

# ---- 4. the index describes what was actually staged -------------------
idx=$(./tools/wasm-pack/index.sh --staged "$STAGE" --tag-prefix "$PREFIX" --offline)
for lang in "${GRAMMARS[@]}"; do
  v=$(grep -m1 '^version = ' "crates/treebank-$lang/Cargo.toml" | cut -d'"' -f2)
  want=$(sha256sum "$STAGE/treebank-$lang-v$v/treebank-$lang.wasm" | cut -d' ' -f1)
  got=$(printf '%s' "$idx" | jq -r --arg p "treebank-$lang" '.packs[] | select(.pack==$p) | .sha256')
  [ "$got" = "$want" ] || fail "$lang: index sha256 $got != artifact $want"
  url=$(printf '%s' "$idx" | jq -r --arg p "treebank-$lang" '.packs[] | select(.pack==$p) | .urls.wasm')
  case "$url" in
    *"$PREFIX"*) fail "$lang: index URL names the rehearsal tag: $url" ;;
    *"treebank-$lang-v$v/treebank-$lang.wasm") ;;
    *) fail "$lang: unexpected index URL: $url" ;;
  esac
done

# ---- 5. a re-run must release nothing ----------------------------------
again=$(TREEBANK_WASM_PYTHON="$PY" ./tools/wasm-pack/release.sh --stage "$STAGE" --tag-prefix "$PREFIX")
echo "$again" | grep -q "0 pack(s) staged" || fail "a re-run released something; releases must be immutable"

echo "test-release: OK — ${#GRAMMARS[@]} packs staged, fetched over HTTP, checksummed, parsed by both consumers, indexed"
