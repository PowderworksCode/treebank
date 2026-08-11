#!/usr/bin/env bash
# Build packs.json: one document listing every published wasm pack.
#
# Twenty-two GitHub Releases are not a distribution. A consumer — and every
# treebank binding — needs one stable URL that answers "what packs exist, at
# what version, and what should each one hash to". That is this file, published
# to the moving `packs-index` tag:
#
#   https://github.com/<owner>/<repo>/releases/download/packs-index/packs.json
#
# The index is deliberately mutable; that is what an index is for. Nothing in
# it is trusted, though: every entry carries the sha256 of an immutable
# artifact, so a tampered index can misdirect a consumer but cannot make them
# accept bytes the project did not build.
#
# Hashes come from the staged release when a pack was built in this run, and
# from that pack's published SHA256SUMS otherwise. When neither is available —
# a dry run with no network — the entry records sha256: null rather than
# guessing, and `note` says why.
#
# Usage: scripts/wasm-index.sh [--staged DIR] [--offline] [--tag-prefix P] > packs.json
set -euo pipefail
shopt -s nullglob

ROOT=$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)
STAGED=""
OFFLINE=0
TAG_PREFIX=""
while [ $# -gt 0 ]; do
  case "$1" in
    --staged)  STAGED=$2; shift ;;
    --offline) OFFLINE=1 ;;
    --tag-prefix) TAG_PREFIX=${2:?--tag-prefix needs a value}; shift ;;
    -h|--help) sed -n '2,20p' "${BASH_SOURCE[0]}"; exit 0 ;;
    *)         echo "wasm-index: unknown flag $1" >&2; exit 2 ;;
  esac
  shift
done

# The repo the releases live in, taken from the remote rather than hard-coded.
SLUG=$(git -C "$ROOT" remote get-url origin 2>/dev/null \
  | sed -E 's#^git@github.com:#https://github.com/#; s#\.git$##; s#^https://github.com/##') || SLUG=""
[ -n "$SLUG" ] || SLUG="PowderworksCode/treebank"
BASE="https://github.com/$SLUG/releases/download"

entries=()
for ledger in "$ROOT"/crates/*/ledger.json; do
  dir=$(dirname "$ledger")
  lang=$(basename "$dir"); lang=${lang#treebank-}
  pack_main="treebank-$lang"

  # The newest release tag for this grammar, which is what "current" means.
  IFS= read -r tag < <(git -C "$ROOT" tag --list "$TAG_PREFIX$pack_main-v*" --sort=-creatordate; printf '\n') || true
  [ -n "$tag" ] || continue                   # never released: not in the index
  version=${tag#"$TAG_PREFIX$pack_main-v"}
  tag=${tag#"$TAG_PREFIX"}                    # URLs always name the real tag

  # SHA256SUMS for this release: staged copy first, then the published one.
  sums=""
  if [ -n "$STAGED" ] && [ -f "$STAGED/$pack_main-v$version/SHA256SUMS" ]; then
    sums=$(cat "$STAGED/$pack_main-v$version/SHA256SUMS")
  elif [ "$OFFLINE" = 0 ] && command -v gh >/dev/null; then
    sums=$(gh release view "$tag" --repo "$SLUG" 2>/dev/null >/dev/null \
      && gh release download "$tag" --repo "$SLUG" --pattern SHA256SUMS --output - 2>/dev/null) || sums=""
  fi

  while IFS= read -r gd; do
    if [ "$gd" = "." ]; then pack="treebank-$lang"; else pack="treebank-$(basename "$gd")"; fi
    sha=$(printf '%s\n' "$sums" | awk -v f="$pack.wasm" '$2==f || $2=="*"f {print $1}' | head -1)
    entries+=("$(jq -n \
      --arg pack "$pack" --arg version "$version" --arg tag "$tag" \
      --arg grammar "$lang" --arg base "$BASE" \
      --arg sha "$sha" \
      --argjson upstream "$(jq -c '.upstream | {git_url, sha, version}' "$ledger")" \
      '{
         pack: $pack,
         grammar: $grammar,
         version: $version,
         upstream: $upstream,
         sha256: (if $sha == "" then null else $sha end),
         note: (if $sha == "" then "sha256 unavailable when the index was built; verify against SHA256SUMS" else null end),
         urls: {
           wasm:       "\($base)/\($tag)/\($pack).wasm",
           provenance: "\($base)/\($tag)/\($pack).json",
           sha256sums: "\($base)/\($tag)/SHA256SUMS",
           queries:    "\($base)/\($tag)/queries.tar.gz"
         }
       }')")
  done < <(jq -r '(.generate_dirs // ["."])[]' "$ledger")
done

# pack_abi is a property of the ABI every pack in this index implements, not of
# any one pack; a consumer reads it once to know whether its binding still fits.
printf '%s\n' "${entries[@]}" | jq -s --arg abi 1 '{
  schema: "treebank-packs-index/1",
  pack_abi: ($abi | tonumber),
  format: "standalone",
  packs: (. | sort_by(.pack))
}'
