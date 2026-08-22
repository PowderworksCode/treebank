#!/usr/bin/env bash
# Stage (and optionally publish) one release per wasm pack.
#
# A release is: the .wasm, its provenance and roles as sibling JSON for
# consumers that would rather read a file than instantiate a module, and
# SHA256SUMS over all three. The module carries provenance and roles inside
# itself as well — the siblings are a convenience, not the source of truth.
#
# --publish is the ONLY thing here that touches the network, and it is off by
# default: staging is what CI exercises, so the path is tested without a
# release ever happening by accident.
#
# Versioning is plain semver from the crate's Cargo.toml. Treebank owns these
# grammars, so there is no upstream version to track and no build counter to
# derive — the scheme the vendoring era needed does not apply.
#
# Usage: tools/wasm-pack/release.sh [--stage DIR] [--tag-prefix P] [--publish] [grammar ...]
set -euo pipefail

ROOT=$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)
cd "$ROOT"

STAGE="dist/release"
TAG_PREFIX=""
PUBLISH=0
GRAMMARS=()
while [ $# -gt 0 ]; do
  case $1 in
    --stage)      STAGE=${2:?--stage needs a value}; shift 2 ;;
    --tag-prefix) TAG_PREFIX=${2:?--tag-prefix needs a value}; shift 2 ;;
    --publish)    PUBLISH=1; shift ;;
    -*) echo "release.sh: unknown argument $1" >&2; exit 2 ;;
    *)  GRAMMARS+=("$1"); shift ;;
  esac
done
if [ ${#GRAMMARS[@]} -eq 0 ]; then
  while IFS= read -r grammar; do
    GRAMMARS+=("$grammar")
  done < <(./tools/wasm-pack/list-grammars.sh)
fi
[ ${#GRAMMARS[@]} -gt 0 ] || { echo "release: no grammars discovered" >&2; exit 1; }

released=0
for lang in "${GRAMMARS[@]}"; do
  crate="crates/treebank-$lang"
  pack="treebank-$lang"
  version=$(grep -m1 '^version = ' "$crate/Cargo.toml" | cut -d'"' -f2)
  tag="$TAG_PREFIX$pack-v$version"

  # Already released at this version: do nothing. A release is immutable, so
  # re-running must be a no-op rather than a second upload.
  if git rev-parse -q --verify "refs/tags/$tag" >/dev/null; then
    echo "release: $tag already exists, skipping"
    continue
  fi

  dir="$STAGE/$pack-v$version"
  mkdir -p "$dir"
  ./tools/wasm-pack/build.sh "$lang" --out "$dir" >/dev/null

  # Provenance and roles beside the binary, extracted from the binary — so
  # they cannot disagree with what the module says about itself.
  "${TREEBANK_WASM_PYTHON:-python3}" - "$dir/$pack.wasm" "$dir/$pack.json" "$dir/$pack.roles.json" <<'PY'
import json, sys
from wasmtime import Engine, Linker, Module, Store, WasiConfig
wasm, prov_out, roles_out = sys.argv[1:4]
eng = Engine(); store = Store(eng); store.set_wasi(WasiConfig())
lk = Linker(eng); lk.define_wasi()
e = lk.instantiate(store, Module.from_file(eng, wasm)).exports(store)
mem = e["memory"]; e["_initialize"](store)
def blob(p, n): return json.loads(mem.read(store, p, p + n))
json.dump(blob(e["tb_provenance"](store), e["tb_provenance_len"](store)), open(prov_out, "w"), indent=2)
json.dump(blob(e["tb_roles"](store), e["tb_roles_len"](store)), open(roles_out, "w"), indent=2)
PY

  (cd "$dir" && sha256sum "$pack.wasm" "$pack.json" "$pack.roles.json" > SHA256SUMS)
  echo "release: staged $tag -> $dir"
  released=$((released + 1))

  if [ "$PUBLISH" = 1 ]; then
    git tag "$tag"
    gh release create "$tag" --repo "$(git remote get-url origin | sed -E 's#(git@|https://)github.com[:/]##; s/\.git$//')" \
      --title "$pack $version" --notes "treebank wasm pack. Verify against SHA256SUMS." \
      "$dir/$pack.wasm" "$dir/$pack.json" "$dir/$pack.roles.json" "$dir/SHA256SUMS"
  fi
done

echo "release: $released pack(s) staged$([ "$PUBLISH" = 1 ] && echo ", published" || echo "; nothing published")"
