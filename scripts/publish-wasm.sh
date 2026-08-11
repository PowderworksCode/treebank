#!/usr/bin/env bash
# Publish treebank wasm packs as GitHub Release assets.
#
# WHERE, and why. Upstream tree-sitter publishes every grammar's wasm as a
# GitHub Release asset (tree-sitter-python.wasm next to the source tarball);
# its npm packages carry native .node prebuilds and no wasm at all. Releases
# are also the only option that costs a consumer in ANY language nothing but an
# HTTPS GET — which matters because treebank packs are meant to be consumed
# from bindings in several languages, not only from JavaScript. npm and OCI
# stay open as later mirrors precisely because the artifact is the same file.
#
# NAMES AND VERSIONS mirror scripts/publish.sh exactly, because a second
# registry with a second scheme is a support burden:
#
#   crates.io   treebank-grammar-python   0.25.0-treebank.N   tag treebank-grammar-python-v...
#   releases    treebank-python           0.25.0-treebank.N   tag treebank-python-v...
#
# The version is the ledger's upstream.version plus an incrementing suffix, and
# the suffix is derived from the tags that already exist rather than stored, so
# it cannot drift. Same tradeoff as the crates: it is a semver PRE-RELEASE, so
# a consumer must name the exact version. See PUBLISHING.md.
#
# WHAT CHANGED. Only grammars whose pack could actually differ are rebuilt and
# released, decided per pack by diffing against the tag of its own last release
# — plus ARTIFACT_INPUTS below, the files outside a grammar directory that
# change what a pack contains.
#
# Usage:
#   scripts/publish-wasm.sh --dry-run                    # build + stage, upload nothing
#   scripts/publish-wasm.sh --dry-run crates/treebank-python
#   scripts/publish-wasm.sh --execute                    # real; needs gh auth
#
#   --dry-run          build and stage the release; never uploads (default)
#   --execute          create the release and upload assets
#   --force            release even if unchanged since the last release tag
#   --skip-materialize build/ is already fresh (only when CI has just built it)
#   --out DIR          staging directory (default dist/release)
#   --tag-prefix P     prefix release tags, so a rehearsal cannot collide with
#                      or be misled by the real ones
set -euo pipefail
shopt -s nullglob

ROOT=$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)
MODE=dry-run
FORCE=0
SKIP_MATERIALIZE=0
TAG_PREFIX=""
OUT="$ROOT/dist/release"
TARGETS=()

while [ $# -gt 0 ]; do
  case "$1" in
    --dry-run)          MODE=dry-run ;;
    --execute)          MODE=execute ;;
    --force)            FORCE=1 ;;
    --skip-materialize) SKIP_MATERIALIZE=1 ;;
    --out)              OUT=$2; shift ;;
    --tag-prefix)       TAG_PREFIX=${2:?--tag-prefix needs a value}; shift ;;
    -h|--help)          sed -n '2,40p' "${BASH_SOURCE[0]}"; exit 0 ;;
    -*)                 echo "publish-wasm: unknown flag $1" >&2; exit 2 ;;
    *)                  TARGETS+=("$1") ;;
  esac
  shift
done

# Files outside a grammar directory that change what a pack CONTAINS. Narrower
# than "anything in scripts/", for the same reason publish.sh's list is narrow:
# a release that cannot alter a byte a consumer downloads spends a version
# number that can never be reused. vendor/tree-sitter is here because the
# runtime is linked into every pack.
ARTIFACT_INPUTS=(
  scripts/build-wasm.sh
  tools/wasm-pack/shim.c
  vendor/tree-sitter
)

if [ "${#TARGETS[@]}" -eq 0 ]; then
  for l in "$ROOT"/crates/*/ledger.json; do TARGETS+=("$(dirname "$l")"); done
fi

# Highest N among existing <pack>-v<base>-treebank.N tags, or 0. Reads tags
# rather than the GitHub API so a dry run needs no network and no auth, and so
# the rehearsal and the real run answer the question the same way.
released_suffix_max() {
  local pack=$1 base=$2 max=0 t n
  while IFS= read -r t; do
    [ -n "$t" ] || continue
    n=${t#"$TAG_PREFIX$pack-v$base-treebank."}
    [ "$n" != "$t" ] && [[ $n =~ ^[0-9]+$ ]] && [ "$n" -gt "$max" ] && max=$n
  done < <(git -C "$ROOT" tag --list "$TAG_PREFIX$pack-v$base-treebank.*")
  echo "$max"
}

overall=0
released=()
skipped=()

for dir in "${TARGETS[@]}"; do
  dir=$(cd "$dir" && pwd)
  rel=${dir#"$ROOT"/}
  [ -f "$dir/ledger.json" ] || { echo "publish-wasm: $rel has no ledger.json" >&2; exit 2; }

  lang=${rel#crates/treebank-}
  base=$(jq -r .upstream.version "$dir/ledger.json")
  sha=$(jq -r .upstream.sha "$dir/ledger.json")
  mapfile -t gen_dirs < <(jq -r '(.generate_dirs // ["."])[]' "$dir/ledger.json")

  # One RELEASE per grammar directory, carrying every pack that directory
  # generates: typescript builds two parsers (typescript, tsx) from one pinned
  # upstream at one version, and splitting them across two releases would let
  # them drift apart for no reason.
  packs=()
  for gd in "${gen_dirs[@]}"; do
    if [ "$gd" = "." ]; then packs+=("treebank-$lang"); else packs+=("treebank-$(basename "$gd")"); fi
  done
  pack_main="treebank-$lang"

  echo "=============================================================="
  echo "$rel  ->  ${packs[*]}"

  # ---- has anything that changes the artifact changed? ---------------------
  IFS= read -r last_tag < <(git -C "$ROOT" tag --list "$TAG_PREFIX$pack_main-v*" --sort=-creatordate; printf '\n') || true
  if [ "$FORCE" = 1 ]; then
    echo "  change check: forced"
  elif [ -z "$last_tag" ]; then
    echo "  change check: no $TAG_PREFIX$pack_main-v* tag yet — first release"
  else
    # test/ is the negative corpus: it gates the release but never ships.
    if git -C "$ROOT" diff --quiet "$last_tag" HEAD \
         -- "$rel" ":(exclude)$rel/test" "${ARTIFACT_INPUTS[@]}"; then
      echo "  change check: unchanged since $last_tag — skipping"
      skipped+=("$pack_main (unchanged since $last_tag)")
      continue
    fi
    echo "  change check: changed since $last_tag"
  fi

  next=$(( $(released_suffix_max "$pack_main" "$base") + 1 ))
  version="$base-treebank.$next"
  tag="$TAG_PREFIX$pack_main-v$version"
  # The staging path is the tag WITHOUT the prefix: a prefix is a git tag
  # namespace ("rehearsal/") and may contain a slash, which would silently
  # turn one staging directory into two nested ones.
  stage="$OUT/$pack_main-v$version"
  echo "  version: $version  (upstream $base + treebank suffix $next)"

  rm -rf "$stage"; mkdir -p "$stage"
  ok=1
  for i in "${!gen_dirs[@]}"; do
    mflag=(--skip-materialize)
    # Materialize once per grammar directory, not once per generated parser.
    [ "$i" = 0 ] && [ "$SKIP_MATERIALIZE" = 0 ] && mflag=()
    if ! "$ROOT/scripts/build-wasm.sh" "${mflag[@]}" --out "$stage" "$dir" "${gen_dirs[$i]}"; then
      echo "publish-wasm: FAIL — ${packs[$i]} did not build" >&2
      ok=0; break
    fi
  done
  # A half-staged directory looks exactly like a finished release to anything
  # that globs the output, so a failure removes it rather than leaving it.
  [ "$ok" = 1 ] || { rm -rf "$stage"; overall=1; continue; }

  # Upstream's LICENSE travels with the binary. The grammars are MIT and the
  # licence requires the notice to accompany redistributions; a .wasm is a
  # redistribution of that source in object form. patches/ and LOCAL-PATCHES.md
  # go with it for the same reason they ship inside the crate tarball: our
  # entire divergence, readable without leaving the release.
  cp "$dir/build/LICENSE" "$stage/LICENSE" 2>/dev/null \
    || cp "$dir/build/LICENSE."* "$stage/" 2>/dev/null \
    || { echo "publish-wasm: FAIL — no LICENSE in $rel/build; upstream attribution must travel" >&2
         rm -rf "$stage"; overall=1; continue; }
  cp "$dir/LOCAL-PATCHES.md" "$stage/LOCAL-PATCHES.md"
  mkdir -p "$stage/patches" && cp "$dir"/patches/*.patch "$stage/patches/"

  # Highlight/injection/tags queries travel with the grammar. Every upstream
  # grammar ships them, editors need them, and nvim-treesitter's release-hygiene
  # tier counts them; they are already in the materialized tree and cost a few
  # KB. Packed as one archive so a release has a fixed asset list however many
  # query files a grammar has.
  # Reproducible: sorted entries, zeroed mtimes and ownership, and `gzip -n` so
  # the gzip header carries no timestamp. Without -n the archive — and so
  # SHA256SUMS — differs on every build, which would make a release look
  # changed when nothing in it had.
  if [ -d "$dir/build/queries" ]; then
    tar -C "$dir/build" --sort=name --mtime=@0 --owner=0 --group=0 \
        --numeric-owner -cf - queries | gzip -n > "$stage/queries.tar.gz"
  fi

  ( cd "$stage" && sha256sum *.wasm *.json $([ -f queries.tar.gz ] && echo queries.tar.gz) > SHA256SUMS )

  cat > "$stage/RELEASE.md" <<EOF
# ${packs[*]} $version

Patched redistribution of $(jq -r .upstream.git_url "$dir/ledger.json") \
$base (\`${sha:0:12}\`), built to standalone WebAssembly.

Each \`.wasm\` is self-contained: it carries the tree-sitter runtime, the
patched grammar and the pack ABI, and imports only WASI. No emscripten glue,
no native tree-sitter, no Rust toolchain. The matching \`.json\` is the same
provenance that is linked *inside* the module — read it from the binary with
\`tb_provenance()\` if the file ever gets separated from it.

**What this gives you:** treebank's patched parsing, with the provenance to
prove which upstream, which patches and which toolchain produced it.

**What it does not give you:** the corpus sweeps and reference-compiler oracle
those patches were derived from. Validating against \`javac\`, \`clang\` or
\`python\` needs real processes on a real machine; a wasm module cannot do it.
The sweep numbers in the provenance are evidence recorded at build time, not
something the pack can re-derive.

Verify: \`sha256sum -c SHA256SUMS\`
EOF

  echo "  staged: $(cd "$stage" && ls | tr '\n' ' ')"
  if [ "$MODE" = dry-run ]; then
    echo "  dry run: would create release $tag with $(ls "$stage" | wc -l) assets"
    released+=("$tag (dry run)")
  else
    command -v gh >/dev/null || { echo "publish-wasm: gh is required for --execute" >&2; exit 2; }
    if gh release create "$tag" --title "${packs[*]} $version" --notes-file "$stage/RELEASE.md" \
         "$stage"/*; then
      released+=("$tag")
      git -C "$ROOT" tag -a "$tag" -m "${packs[*]} $version (upstream $base @ ${sha:0:7})" 2>/dev/null || true
    else
      echo "publish-wasm: FAIL — $tag did not publish" >&2
      overall=1
    fi
  fi
done

# The index: one stable URL listing every pack, so consumers do not have to
# discover twenty-two releases. Regenerated from the tags that exist plus
# whatever this run staged, and published to a moving tag of its own.
INDEX="$OUT/packs.json"
mkdir -p "$OUT"
index_args=(--staged "$OUT")
[ -n "$TAG_PREFIX" ] && index_args+=(--tag-prefix "$TAG_PREFIX")
[ "$MODE" = dry-run ] && index_args+=(--offline)
"$ROOT/scripts/wasm-index.sh" "${index_args[@]}" > "$INDEX"
echo "  index: $(jq '.packs|length' "$INDEX") packs -> $INDEX"
if [ "$MODE" = execute ] && [ "$overall" = 0 ]; then
  # Deleted and recreated rather than edited: a release asset cannot be
  # replaced atomically, and a half-updated index is worse than a stale one.
  gh release delete "${TAG_PREFIX}packs-index" --yes --cleanup-tag 2>/dev/null || true
  gh release create "${TAG_PREFIX}packs-index" --title "Pack index" \
     --notes "Every published treebank wasm pack, with the sha256 of each artifact. Regenerated on every release run." \
     "$INDEX"
  echo "  index: published to ${TAG_PREFIX}packs-index"
fi

echo "=============================================================="
for r in "${released[@]}"; do echo "  released: $r"; done
for s in "${skipped[@]}"; do echo "  skipped:  $s"; done
[ "$overall" = 0 ] || echo "  one or more packs failed" >&2
exit "$overall"
