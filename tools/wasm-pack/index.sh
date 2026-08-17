#!/usr/bin/env bash
# Build packs.json: one document listing every published wasm pack.
#
# Published to a MOVING `packs-index` tag, so a consumer has one stable URL
# instead of N releases to discover:
#
#   https://github.com/<owner>/<repo>/releases/download/packs-index/packs.json
#
# The index is mutable by design. Nothing in it is trusted, though: every
# entry carries the sha256 of an IMMUTABLE artifact, so a consumer that
# checks the hash cannot be harmed by the index moving under it.
#
# Usage: tools/wasm-pack/index.sh [--staged DIR] [--offline] [--tag-prefix P]
set -euo pipefail

ROOT=$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)
cd "$ROOT"

STAGED=""
OFFLINE=0
TAG_PREFIX=""
while [ $# -gt 0 ]; do
  case $1 in
    --staged)     STAGED=${2:?--staged needs a value}; shift 2 ;;
    --offline)    OFFLINE=1; shift ;;
    --tag-prefix) TAG_PREFIX=${2:?--tag-prefix needs a value}; shift 2 ;;
    *) echo "index.sh: unknown argument $1" >&2; exit 2 ;;
  esac
done

SLUG=$(git remote get-url origin 2>/dev/null \
  | sed -E 's#(git@|https://)github.com[:/]##; s/\.git$//') || SLUG="PowderworksCode/treebank"
BASE="https://github.com/$SLUG/releases/download"

entries=()
for crate in crates/treebank-*/; do
  lang=$(basename "$crate" | sed 's/^treebank-//')
  [ -f "$crate/ledger.json" ] || continue          # only grammar crates
  pack="treebank-$lang"

  # The newest release tag for this pack is what "current" means.
  tag=$(git tag --list "$TAG_PREFIX$pack-v*" --sort=-creatordate | head -1 || true)
  [ -n "$tag" ] || continue
  version=${tag#"$TAG_PREFIX$pack-v"}
  tag=${tag#"$TAG_PREFIX"}                          # URLs always name the real tag

  # SHA256SUMS for this release: the staged copy first, then the published one.
  sums=""
  if [ -n "$STAGED" ] && [ -f "$STAGED/$pack-v$version/SHA256SUMS" ]; then
    sums=$(cat "$STAGED/$pack-v$version/SHA256SUMS")
  elif [ "$OFFLINE" = 0 ] && command -v gh >/dev/null; then
    sums=$(gh release download "$tag" --repo "$SLUG" --pattern SHA256SUMS --output - 2>/dev/null) || sums=""
  fi
  sha=$(printf '%s\n' "$sums" | awk -v f="$pack.wasm" '$2==f || $2=="*"f {print $1}' | head -1)

  entries+=("$(jq -n \
    --arg pack "$pack" --arg grammar "$lang" --arg version "$version" \
    --arg tag "$tag" --arg base "$BASE" --arg sha "$sha" \
    --arg vocabulary "$(jq -r '.vocabulary // ""' "$crate/ledger.json")" \
    --arg cli "$(jq -r '.generate_cli // ""' "$crate/ledger.json")" \
    --arg versions "$(jq -r '.versions // ""' "$crate/ledger.json")" \
    '{
       pack: $pack,
       grammar: $grammar,
       version: $version,
       language_versions: $versions,
       vocabulary: $vocabulary,
       generate_cli: $cli,
       sha256: (if $sha == "" then null else $sha end),
       note: (if $sha == "" then "sha256 unavailable when the index was built; verify against SHA256SUMS" else null end),
       urls: {
         wasm:       "\($base)/\($tag)/\($pack).wasm",
         provenance: "\($base)/\($tag)/\($pack).json",
         roles:      "\($base)/\($tag)/\($pack).roles.json",
         sha256sums: "\($base)/\($tag)/SHA256SUMS"
       }
     }')")
done

[ ${#entries[@]} -gt 0 ] || { echo '{"schema":"treebank-packs-index/1","pack_abi":1,"format":"standalone","packs":[]}'; exit 0; }

# pack_abi is a property of the ABI every pack in this index implements, not
# of any one pack: a consumer reads it once to know whether its binding fits.
printf '%s\n' "${entries[@]}" | jq -s '{
  schema: "treebank-packs-index/1",
  pack_abi: 1,
  format: "standalone",
  packs: (. | sort_by(.pack))
}'
