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
#   1. every grammar under test publishes, and gets tagged;
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
# Usage: scripts/test-publish.sh [publish.sh flags] [grammar-dir...]
#   scripts/test-publish.sh                              every grammar
#   scripts/test-publish.sh crates/treebank-rust         just one
#   scripts/test-publish.sh --skip-verify                what CI runs, since the
#                                                        verify job already gated
set -euo pipefail

ROOT=$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)
REG_NAME=local
CONSUMER="$ROOT/tools/consumer-test"

FLAGS=()
DIRS=()
while [ $# -gt 0 ]; do
  case "$1" in
    -*) FLAGS+=("$1") ;;
    *)  DIRS+=("$1") ;;
  esac
  shift
done
if [ "${#DIRS[@]}" -eq 0 ]; then
  for l in "$ROOT"/crates/*/ledger.json; do DIRS+=("crates/$(basename "$(dirname "$l")")"); done
fi

# Grammar dir names, e.g. treebank-rust — the key grammars.json is written in.
NAMES=()
for d in "${DIRS[@]}"; do NAMES+=("$(basename "$d")"); done
echo "test-publish: ${#NAMES[@]} grammar(s): ${NAMES[*]}"

# Every grammar under test must have an entry, or it would publish and then
# silently not be consumer-tested.
for n in "${NAMES[@]}"; do
  if ! jq -e --arg g "$n" 'any(.[]; .grammar == $g)' "$CONSUMER/grammars.json" >/dev/null; then
    echo "test-publish: FAIL — $n has no entry in tools/consumer-test/grammars.json;" >&2
    echo "              add one (dep, crate, fixtures) so publishing it is actually tested." >&2
    exit 1
  fi
done

if ! command -v cargo-http-registry >/dev/null; then
  echo "test-publish: cargo-http-registry not found; installing" >&2
  cargo install cargo-http-registry --quiet
fi

PORT=$(python3 -c 'import socket;s=socket.socket();s.bind(("127.0.0.1",0));print(s.getsockname()[1]);s.close()')
REG_ROOT=$(mktemp -d)
SERVER_PID=""
# The rehearsal publishes at the same version numbers as production (both count
# up from an empty registry), so its tags must live in their own namespace or
# they collide with the real ones the moment anything has actually shipped —
# and, worse, a real tag would make the rehearsal skip instead of publish, so it
# would quietly stop testing anything.
TAG_PREFIX="rehearsal/"

cleanup() {
  local rc=$?
  [ -n "$SERVER_PID" ] && kill "$SERVER_PID" 2>/dev/null || true
  # Only the rehearsal's own namespace, so a real publish's tags are untouchable
  # by construction rather than by careful bookkeeping.
  while IFS= read -r t; do
    [ -n "$t" ] && git -C "$ROOT" tag -d "$t" >/dev/null 2>&1 || true
  done < <(git -C "$ROOT" tag --list "$TAG_PREFIX*")
  rm -rf "$REG_ROOT" \
         "$CONSUMER/Cargo.toml" "$CONSUMER/Cargo.lock" "$CONSUMER/src/cases.rs"
  return $rc
}
trap cleanup EXIT

# A previous run killed before its trap ran would leave these behind, and they
# would make step 1 skip rather than publish.
git -C "$ROOT" tag --list "$TAG_PREFIX*" | while IFS= read -r t; do
  git -C "$ROOT" tag -d "$t" >/dev/null
done

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

# publish.sh creates annotated tags, which need a tagger identity. A fresh CI
# runner has none, and this is a rehearsal, so supply a throwaway one via the
# environment rather than writing to the checkout's git config.
export GIT_COMMITTER_NAME="${GIT_COMMITTER_NAME:-treebank rehearsal}"
export GIT_COMMITTER_EMAIL="${GIT_COMMITTER_EMAIL:-rehearsal@localhost}"
export GIT_AUTHOR_NAME="$GIT_COMMITTER_NAME"
export GIT_AUTHOR_EMAIL="$GIT_COMMITTER_EMAIL"

pub() { "$ROOT/scripts/publish.sh" --execute --registry "$REG_NAME" --index "$REG_ROOT" \
          --no-push --tag-prefix "$TAG_PREFIX" ${FLAGS+"${FLAGS[@]}"} "$@"; }

echo
echo "== 1/4 publish to it"
pub --force "${DIRS[@]}"

# Read back what actually landed, rather than trusting the log.
index_path() {
  local n=${1,,}
  case ${#n} in
    1) echo "1/$n" ;; 2) echo "2/$n" ;; 3) echo "3/${n:0:1}/$n" ;;
    *) echo "${n:0:2}/${n:2:2}/$n" ;;
  esac
}
declare -A VERSION
missing=0
for n in "${NAMES[@]}"; do
  crate="treebank-grammar-${n#treebank-}"
  f="$REG_ROOT/$(index_path "$crate")"
  if [ ! -f "$f" ]; then
    echo "test-publish: FAIL — $crate never reached the registry" >&2; missing=1; continue
  fi
  VERSION[$n]=$(jq -r 'select(.vers) | .vers' "$f" | sort -V | tail -1)
  echo "   $crate ${VERSION[$n]}"
done
[ "$missing" = 0 ] || exit 1

echo
echo "== 2/4 a consumer resolves them and parses code upstream cannot"
# Manifest and case list are generated from grammars.json for exactly the
# grammars under test, so a one-grammar run builds a one-grammar consumer.
cp "$CONSUMER/Cargo.toml.head" "$CONSUMER/Cargo.toml"
{
  echo "pub fn cases() -> Vec<(&'static str, Language, &'static str, Expect)> {"
  echo "    vec!["
} > "$CONSUMER/src/cases.rs"
for n in "${NAMES[@]}"; do
  jq -r --arg g "$n" --arg v "${VERSION[$n]}" '
    .[] | select(.grammar == $g)
    | "\(.dep) = { package = \"\(.crate)\", version = \"=\($v)\", registry = \"@REG@\" }"
  ' "$CONSUMER/grammars.json" | sed "s|@REG@|$REG_NAME|" >> "$CONSUMER/Cargo.toml"
  jq -r --arg g "$n" '
    .[] | select(.grammar == $g) | .cases[]
    | "        (\(.label | @json), \(.language).into(), include_str!(\"../fixtures/\(.fixture)\"), Expect::\(.expect)),"
  ' "$CONSUMER/grammars.json" >> "$CONSUMER/src/cases.rs"
done
{ echo "    ]"; echo "}"; } >> "$CONSUMER/src/cases.rs"
(cd "$CONSUMER" && cargo run --quiet)

echo
echo "== 3/4 re-running publishes nothing (everything is tagged)"
out=$(pub "${DIRS[@]}" 2>&1) || { echo "$out"; exit 1; }
if echo "$out" | grep -q 'published:'; then
  echo "$out" >&2
  echo "test-publish: FAIL — a second run published something; it should have skipped" >&2
  exit 1
fi
echo "$out" | grep 'skipped:' || { echo "test-publish: FAIL — nothing was skipped either" >&2; exit 1; }

echo
echo "== 4/4 a forced re-publish lands on the next suffix, not the same one"
one=${NAMES[0]}
pub --force "crates/$one" >/dev/null
crate="treebank-grammar-${one#treebank-}"
after=$(jq -r 'select(.vers) | .vers' "$REG_ROOT/$(index_path "$crate")" | sort -V | tail -1)
expected=${VERSION[$one]%.*}.2
if [ "$after" != "$expected" ]; then
  echo "test-publish: FAIL — expected $expected after a forced re-publish, got $after" >&2
  exit 1
fi
echo "   $crate ${VERSION[$one]} -> $after"

echo
echo "test-publish: ok — publish, consume, skip and increment all behave"
