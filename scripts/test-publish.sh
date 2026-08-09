#!/usr/bin/env bash
# Rehearse the whole publish path against a throwaway local registry.
#
# scripts/publish.sh can be checked up to the moment it uploads, and no further:
# against crates.io the next step is irreversible, so the parts that only happen
# after a successful upload — the tag, the version increment, the skip on a
# re-run, and a consumer actually resolving the crate — were untestable. This
# script closes that gap by standing up a real registry (cargo-http-registry) on
# localhost and publishing to it for real.
#
# What it asserts, in order:
#
#   1. every grammar publishes, and gets tagged;
#   2. a consumer resolves those crates from the registry, links them under
#      upstream's names, and parses code upstream's grammar cannot — so what
#      was published really is the patched grammar (tools/consumer-test);
#   3. re-running publishes nothing, because everything is tagged (idempotence);
#   4. forcing a second publish lands on -treebank.2, not .1 (the suffix really
#      is derived from the registry rather than assumed).
#
# Nothing here touches crates.io. Tags it creates are deleted on exit, and it
# never pushes.
#
# Usage: scripts/test-publish.sh [extra publish.sh flags]
#   e.g. scripts/test-publish.sh --skip-verify     (when CI already verified)
set -euo pipefail

ROOT=$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)
EXTRA=("$@")
REG_NAME=local
CONSUMER="$ROOT/tools/consumer-test"

if ! command -v cargo-http-registry >/dev/null; then
  echo "test-publish: cargo-http-registry not found; installing" >&2
  cargo install cargo-http-registry --quiet
fi

PORT=$(python3 -c 'import socket;s=socket.socket();s.bind(("127.0.0.1",0));print(s.getsockname()[1]);s.close()')
REG_ROOT=$(mktemp -d)
TAGS_BEFORE=$(mktemp)
git -C "$ROOT" tag --list 'treebank-grammar-*' | sort > "$TAGS_BEFORE"
SERVER_PID=""

cleanup() {
  local rc=$?
  [ -n "$SERVER_PID" ] && kill "$SERVER_PID" 2>/dev/null || true
  # Delete only tags this run created; a real publish's tags must survive.
  if [ -s "$TAGS_BEFORE" ] || true; then
    while IFS= read -r t; do
      [ -n "$t" ] && git -C "$ROOT" tag -d "$t" >/dev/null 2>&1 || true
    done < <(comm -13 "$TAGS_BEFORE" <(git -C "$ROOT" tag --list 'treebank-grammar-*' | sort))
  fi
  rm -rf "$REG_ROOT" "$TAGS_BEFORE" "$CONSUMER/Cargo.toml" "$CONSUMER/Cargo.lock"
  return $rc
}
trap cleanup EXIT

echo "== starting a throwaway registry on 127.0.0.1:$PORT"
cargo-http-registry --addr "127.0.0.1:$PORT" "$REG_ROOT" >"$REG_ROOT.log" 2>&1 &
SERVER_PID=$!
for _ in $(seq 1 50); do
  [ -s "$REG_ROOT/config.json" ] && break
  sleep 0.2
done
if [ ! -s "$REG_ROOT/config.json" ]; then
  echo "test-publish: registry did not come up" >&2; cat "$REG_ROOT.log" >&2; exit 1
fi

# Configured by environment rather than a config file so nothing is left behind
# in the work tree. cargo accepts any token for this registry.
export CARGO_REGISTRIES_LOCAL_INDEX="file://$REG_ROOT"
export CARGO_REGISTRIES_LOCAL_TOKEN=rehearsal-token-not-a-credential

echo
echo "== 1/4 publish every grammar to it"
"$ROOT/scripts/publish.sh" --execute --registry "$REG_NAME" --index "$REG_ROOT" \
  --no-push --force "${EXTRA[@]}"

# Read back what actually landed, rather than trusting the log.
declare -A VERSION
index_path() {
  local n=${1,,}
  case ${#n} in
    1) echo "1/$n" ;; 2) echo "2/$n" ;; 3) echo "3/${n:0:1}/$n" ;;
    *) echo "${n:0:2}/${n:2:2}/$n" ;;
  esac
}
missing=0
for l in "$ROOT"/crates/*/ledger.json; do
  lang=$(basename "$(dirname "$l")"); lang=${lang#treebank-}
  crate="treebank-grammar-$lang"
  f="$REG_ROOT/$(index_path "$crate")"
  if [ ! -f "$f" ]; then
    echo "test-publish: FAIL — $crate never reached the registry" >&2; missing=1; continue
  fi
  VERSION[$lang]=$(jq -r 'select(.vers) | .vers' "$f" | sort -V | tail -1)
  echo "   $crate ${VERSION[$lang]}"
done
[ "$missing" = 0 ] || exit 1

echo
echo "== 2/4 a consumer resolves them and parses code upstream cannot"
sed -e "s|@REGISTRY@|$REG_NAME|g" \
    -e "s|@VERSION_RUST@|${VERSION[rust]}|g" \
    -e "s|@VERSION_TYPESCRIPT@|${VERSION[typescript]}|g" \
    -e "s|@VERSION_JAVASCRIPT@|${VERSION[javascript]}|g" \
    -e "s|@VERSION_JAVA@|${VERSION[java]}|g" \
    -e "s|@VERSION_CSHARP@|${VERSION[csharp]}|g" \
    "$CONSUMER/Cargo.toml.in" > "$CONSUMER/Cargo.toml"
# A grammar added without a line in Cargo.toml.in would otherwise be published
# and then silently not consumer-tested.
if grep -vE '^\s*#' "$CONSUMER/Cargo.toml" | grep -q '@'; then
  echo "test-publish: FAIL — an unsubstituted placeholder is left in Cargo.toml;" >&2
  echo "              tools/consumer-test/Cargo.toml.in is missing a grammar." >&2
  grep -nvE '^\s*#' "$CONSUMER/Cargo.toml" | grep '@' >&2
  exit 1
fi
(cd "$CONSUMER" && cargo run --quiet)

echo
echo "== 3/4 re-running publishes nothing (everything is tagged)"
out=$("$ROOT/scripts/publish.sh" --execute --registry "$REG_NAME" --index "$REG_ROOT" \
  --no-push "${EXTRA[@]}" 2>&1) || { echo "$out"; exit 1; }
if echo "$out" | grep -q 'published:'; then
  echo "$out" >&2
  echo "test-publish: FAIL — a second run published something; it should have skipped" >&2
  exit 1
fi
echo "$out" | grep 'skipped:' || { echo "test-publish: FAIL — nothing was skipped either" >&2; exit 1; }

echo
echo "== 4/4 a forced re-publish lands on the next suffix, not the same one"
one_lang=rust
"$ROOT/scripts/publish.sh" --execute --registry "$REG_NAME" --index "$REG_ROOT" \
  --no-push --force "${EXTRA[@]}" "$ROOT/crates/treebank-$one_lang" >/dev/null
after=$(jq -r 'select(.vers) | .vers' "$REG_ROOT/$(index_path "treebank-grammar-$one_lang")" | sort -V | tail -1)
expected=${VERSION[$one_lang]%.*}.2
if [ "$after" != "$expected" ]; then
  echo "test-publish: FAIL — expected $expected after a forced re-publish, got $after" >&2
  exit 1
fi
echo "   treebank-grammar-$one_lang ${VERSION[$one_lang]} -> $after"

echo
echo "test-publish: ok — publish, consume, skip and increment all behave"
