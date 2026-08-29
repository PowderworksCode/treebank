#!/usr/bin/env bash
# Publish the `treebank` crate to crates.io.
#
# Publishing is irreversible: a version can be yanked but never deleted, and a
# name/version pair can never be reused. The shape of this script follows from
# that.
#
#   - Dry run is the default. Uploading takes --execute.
#   - A version the registry already has is skipped rather than attempted, so
#     re-running a release for an existing tag is a no-op instead of a failure.
#   - Existing versions come from the sparse index rather than the web API,
#     because the index is what cargo itself resolves against, and because
#     --index lets the whole path be rehearsed against a local directory.
#   - The version is whatever crates/treebank/Cargo.toml says. Nothing here
#     computes or bumps it; the release workflow checks the tag against it.
#
# Only `treebank` is published. The grammars ship as wasm packs from R2 and
# the CLI is not a library, so both carry `publish = false` -- there is no
# ordering problem here and no dependency to publish first.
#
# Usage:
#   tools/publish-crate.sh                 # dry run: package and verify
#   tools/publish-crate.sh --execute       # real publish; needs a token
#   tools/publish-crate.sh --execute --registry local --index ./idx
#
#   --dry-run       package and compile the tarball; never uploads (default)
#   --execute       actually publish
#   --registry NAME publish to a cargo registry other than crates.io
#   --index URL     where to enumerate existing versions; an https:// base or a
#                   local sparse-index directory. Defaults to index.crates.io.
#   --allow-dirty   package with uncommitted changes present
#
# Environment:
#   CARGO_REGISTRY_TOKEN            required for --execute against crates.io.
#   CARGO_REGISTRIES_<NAME>_TOKEN   ... or for --execute --registry <name>.
#   Neither is ever logged.
set -euo pipefail

ROOT=$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)
CRATE=treebank
MANIFEST=crates/treebank/Cargo.toml
MODE=dry-run
REGISTRY=""
INDEX_BASE="https://index.crates.io"
ALLOW_DIRTY=0

while [ $# -gt 0 ]; do
  case "$1" in
    --dry-run)     MODE=dry-run ;;
    --execute)     MODE=execute ;;
    --registry)    REGISTRY=${2:?--registry needs a name}; shift ;;
    --index)       INDEX_BASE=${2:?--index needs a url or directory}; shift ;;
    --allow-dirty) ALLOW_DIRTY=1 ;;
    -h|--help)     sed -n '2,36p' "${BASH_SOURCE[0]}"; exit 0 ;;
    -*)            echo "publish-crate: unknown flag $1" >&2; exit 2 ;;
    *)             echo "publish-crate: unexpected argument $1" >&2; exit 2 ;;
  esac
  shift
done

cd "$ROOT"

VERSION=$(sed -n 's/^version = "\(.*\)"$/\1/p' "$MANIFEST" | head -n 1)
[ -n "$VERSION" ] || { echo "publish-crate: no version in $MANIFEST" >&2; exit 1; }

# cargo reads the token for a named registry from CARGO_REGISTRIES_<NAME>_TOKEN,
# and only the default registry from CARGO_REGISTRY_TOKEN.
if [ -n "$REGISTRY" ]; then
  reg_upper=${REGISTRY^^}; reg_upper=${reg_upper//-/_}
  TOKEN_VAR="CARGO_REGISTRIES_${reg_upper}_TOKEN"
else
  TOKEN_VAR=CARGO_REGISTRY_TOKEN
fi
if [ "$MODE" = execute ] && [ -z "${!TOKEN_VAR:-}" ]; then
  cat >&2 <<MSG
publish-crate: --execute needs $TOKEN_VAR and it is not set.

  In CI:   Settings > Secrets and variables > Actions > New repository secret
           Name: CARGO_REGISTRY_TOKEN
           Value: a crates.io API token with publish-new, scoped to $CRATE

  Once the crate exists on crates.io, configure Trusted Publishing instead
  and delete that secret: the release workflow then authenticates over OIDC
  with a token that lives under an hour.
MSG
  exit 1
fi

# The sparse index lays a name out as <first two>/<next two>/<name>, and each
# line is one published version.
index_path() {
  local name=$1
  case ${#name} in
    1) printf '1/%s\n' "$name" ;;
    2) printf '2/%s\n' "$name" ;;
    3) printf '3/%s/%s\n' "${name:0:1}" "$name" ;;
    *) printf '%s/%s/%s\n' "${name:0:2}" "${name:2:2}" "$name" ;;
  esac
}

# Whether the registry already carries this version.
#
# A missing entry is the normal case for a first publish and is not an error;
# anything else that goes wrong reads as "not published", and cargo refuses the
# upload on its own if that guess was wrong.
already_published() {
  local path body
  path=$(index_path "$CRATE")
  if [ -d "$INDEX_BASE" ]; then
    [ -f "${INDEX_BASE}/${path}" ] || return 1
    body=$(cat "${INDEX_BASE}/${path}")
  else
    body=$(curl -sSf "${INDEX_BASE}/${path}" 2>/dev/null) || return 1
  fi
  # A herestring rather than a pipe: `grep -q` exits at the first match, and
  # under `set -o pipefail` the SIGPIPE that gives the writer would become the
  # status of the whole pipeline, turning a hit into a miss.
  grep -q "\"vers\"[[:space:]]*:[[:space:]]*\"${VERSION}\"" <<<"$body"
}

if already_published; then
  echo "publish-crate: ${CRATE} ${VERSION} is already on the registry; nothing to do"
  exit 0
fi

# Written as `if` rather than `test && ...`: under `set -e` a false test as a
# bare statement aborts the script.
args=(publish --package "$CRATE" --locked)
if [ "$MODE" = dry-run ]; then args+=(--dry-run); fi
if [ -n "$REGISTRY" ]; then args+=(--registry "$REGISTRY"); fi
if [ "$ALLOW_DIRTY" = 1 ]; then args+=(--allow-dirty); fi

echo "publish-crate: ${MODE} ${CRATE} ${VERSION}"
cargo "${args[@]}"
