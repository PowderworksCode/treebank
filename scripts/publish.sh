#!/usr/bin/env bash
# Publish a grammar crate to crates.io.
#
# What gets published. Not the directory under crates/ — that holds only the
# sources of truth (upstream submodule pointer, patches/, ledger.json). The
# crate is scripts/materialize.sh's output, `<grammar-dir>/build/`, plus the
# provenance files copied in beside it. So publishing runs the same
# materialization CI verifies, and uploads exactly what it produced.
#
# Naming. Upstream owns tree-sitter-<lang> on crates.io, so every grammar
# publishes as treebank-grammar-<lang>, derived from the directory name. Each
# grammar's patches/ carries a "treebank crate identity" patch that applies that
# name (and our repository/homepage/description) to upstream's Cargo.toml. This
# script derives the name it expects and refuses to publish a crate whose
# manifest disagrees — a grammar added without that patch fails here, loudly,
# rather than trying to upload under upstream's name.
#
# Versioning. No published version is ever stored in the tree. The manifest
# carries the *upstream* version (which is the ledger's upstream.version — this
# script asserts they agree), and the published version is that base plus an
# incrementing treebank suffix:
#
#     upstream 0.24.2  ->  0.24.2-treebank.1, 0.24.2-treebank.2, ...
#
# The suffix is derived from what crates.io already has, never from a counter in
# the repo, so it cannot drift. Note that this is a semver *pre-release*: it
# sorts below plain 0.24.2 and a `"0.24"` requirement will not select it.
# Consumers must name the exact version. See PUBLISHING.md.
#
# Idempotence. A crate is published only when something under its directory
# changed since the tag of its own last publish (`<crate>-v<version>`) — which
# includes the upstream submodule pointer, so a version bump counts as a change
# even though no file in the repo does. Re-running after everything is tagged is
# a no-op; re-running after a partial failure publishes only what never got
# tagged. Because the suffix comes from crates.io, a version computed here is by
# construction one that does not exist yet.
#
# Verification. Materialization is a precondition, not a formality: a crate that
# does not come out of `submodule @ pinned sha + patches + generate`, pass its
# corpus tests and still reject the negative corpus must never reach crates.io.
# By default this script runs scripts/verify.sh per crate and publishes its
# build/ only if that passed.
#
# Usage:
#   scripts/publish.sh --dry-run                 # every grammar, package only
#   scripts/publish.sh --dry-run crates/treebank-rust
#   scripts/publish.sh --execute                 # real publish; needs a token
#   scripts/publish.sh --execute --force crates/treebank-rust
#
#   --dry-run       package and compile the tarball; never uploads (default)
#   --execute       actually publish, and tag
#   --force         publish even if nothing changed since the last publish tag
#   --skip-verify   materialize but skip the tests (only when CI already gated)
#   --no-tag        do not create a git tag
#   --no-push       create tags but do not push them
#   --tag-prefix P  prefix the publish tags, so a rehearsal against a throwaway
#                   registry cannot collide with (or be misled by) the real
#                   publish tags. Affects both the tags created and the ones the
#                   change check reads, so the two stay consistent.
#   --registry NAME publish to a cargo registry other than crates.io
#   --index URL     where to enumerate existing versions; a https:// base or a
#                   local sparse-index directory. Defaults to index.crates.io.
#
# The last two exist so the whole path — upload, tag, and a consumer resolving
# the result — can be rehearsed against a throwaway local registry without
# touching crates.io. scripts/test-publish.sh does exactly that, and CI runs it
# on every change. See PUBLISHING.md.
#
# Environment:
#   CARGO_REGISTRY_TOKEN            required for --execute against crates.io.
#   CARGO_REGISTRIES_<NAME>_TOKEN   ... or for --execute --registry <name>.
#   Neither is ever logged.
set -euo pipefail
shopt -s nullglob

ROOT=$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)
MODE=dry-run
FORCE=0
SKIP_VERIFY=0
DO_TAG=1
DO_PUSH=1
TAG_PREFIX=""
REGISTRY=""
INDEX_BASE="https://index.crates.io"
TARGETS=()

while [ $# -gt 0 ]; do
  case "$1" in
    --dry-run)     MODE=dry-run ;;
    --execute)     MODE=execute ;;
    --force)       FORCE=1 ;;
    --skip-verify) SKIP_VERIFY=1 ;;
    --no-tag)      DO_TAG=0 ;;
    --no-push)     DO_PUSH=0 ;;
    --tag-prefix)  TAG_PREFIX=${2:?--tag-prefix needs a value}; shift ;;
    --registry)    REGISTRY=${2:?--registry needs a name}; shift ;;
    --index)       INDEX_BASE=${2:?--index needs a url or directory}; shift ;;
    -h|--help)     sed -n '2,66p' "${BASH_SOURCE[0]}"; exit 0 ;;
    -*)            echo "publish: unknown flag $1" >&2; exit 2 ;;
    *)             TARGETS+=("$1") ;;
  esac
  shift
done

# Files outside a grammar directory that still change what its crate contains:
# materialize.sh builds the tree that ships, and this script stamps and packages
# it. A change to either makes every crate "changed since its own last publish",
# which is what makes a core change publish everything — see PUBLISHING.md.
#
# Deliberately narrow. Widening this to all of scripts/ or crates/treebank-cli/
# would cut five releases for a change that cannot alter a single byte a
# consumer downloads, and a crates.io version spent is spent. Note this is a
# *change* check, not a force: once the run tags, a re-run skips.
ARTIFACT_INPUTS=(
  scripts/materialize.sh
  scripts/publish.sh
)

# A grammar crate is a crates/ subdir with a ledger.json. That is what separates
# the grammars from treebank-cli, which this script does not publish.
if [ "${#TARGETS[@]}" -eq 0 ]; then
  for l in "$ROOT"/crates/*/ledger.json; do TARGETS+=("$(dirname "$l")"); done
fi

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
publish: --execute needs $TOKEN_VAR and it is not set.

  In CI:    Settings > Secrets and variables > Actions > New repository secret
            Name: CARGO_REGISTRY_TOKEN
            Value: a crates.io API token with publish-new + publish-update,
                   scoped to treebank-grammar-*
  Locally:  $TOKEN_VAR=... scripts/publish.sh --execute

This is a hard failure rather than a fallback to --dry-run: a publish run that
quietly uploads nothing looks exactly like a publish run that worked.
See PUBLISHING.md.
MSG
  exit 2
fi

# crates.io sparse index path: 1/x, 2/xx, 3/x/xxx, else xx/xx/name.
index_path() {
  local n=${1,,}
  case ${#n} in
    1) echo "1/$n" ;;
    2) echo "2/$n" ;;
    3) echo "3/${n:0:1}/$n" ;;
    *) echo "${n:0:2}/${n:2:2}/$n" ;;
  esac
}

# Highest N among published <base>-treebank.N, or 0. Yanked versions count:
# the number is spent either way, and crates.io will not let us reuse it.
published_suffix_max() {
  local name=$1 base=$2 body http max=0 v n path
  path=$(index_path "$name")
  case "$INDEX_BASE" in
    http://*|https://*)
      body=$(curl -sS --retry 3 --retry-delay 2 -w '\n%{http_code}' \
        "$INDEX_BASE/$path") || {
        echo "publish: could not reach the registry index at $INDEX_BASE" >&2; exit 1; }
      http=${body##*$'\n'}
      body=${body%$'\n'*}
      case "$http" in
        404) echo 0; return ;;   # crate does not exist yet -> first publish
        200) ;;
        *)   echo "publish: registry index returned HTTP $http for $name" >&2; exit 1 ;;
      esac
      ;;
    *)
      # A local sparse-index directory, as a throwaway registry serves. Same
      # xx/xx/name layout and same JSON-lines format as crates.io.
      local f="${INDEX_BASE#file://}/$path"
      [ -f "$f" ] || { echo 0; return; }
      body=$(cat "$f")
      ;;
  esac
  while IFS= read -r v; do
    [ -n "$v" ] || continue
    n=${v#"$base-treebank."}
    [ "$n" != "$v" ] && [[ $n =~ ^[0-9]+$ ]] && [ "$n" -gt "$max" ] && max=$n
  done < <(printf '%s\n' "$body" | jq -r 'select(.vers) | .vers')
  echo "$max"
}

STAGE_ROOT=$(mktemp -d)
# Kept on failure: the verify/materialize logs live in here, and deleting them
# is exactly what you do not want when a publish has just refused to run.
cleanup_stage() {
  if [ "${overall:-0}" = 0 ]; then
    rm -rf "$STAGE_ROOT"
  else
    echo "publish: logs kept in $STAGE_ROOT" >&2
  fi
}
trap cleanup_stage EXIT

overall=0
published=()
skipped=()

for dir in "${TARGETS[@]}"; do
  dir=$(cd "$dir" && pwd)
  rel=${dir#"$ROOT"/}
  [ -f "$dir/ledger.json" ] || { echo "publish: $rel has no ledger.json — not a grammar crate" >&2; exit 2; }

  # The crate name is derived from the directory, not read from the manifest:
  # it is the thing being asserted. crates/treebank-rust -> treebank-grammar-rust.
  lang=${rel#crates/treebank-}
  name="treebank-grammar-$lang"

  base=$(jq -r .upstream.version "$dir/ledger.json")
  sha=$(jq -r .upstream.sha "$dir/ledger.json")
  upstream_url=$(jq -r .upstream.git_url "$dir/ledger.json")
  grammar_patches=$(jq '[.patches[] | select((.kind // "grammar") == "grammar")] | length' "$dir/ledger.json")

  echo "=============================================================="
  echo "$rel  ->  $name"

  # ---- has anything shipped-worthy changed since our last publish? ----------
  # Checked before materializing, which is the expensive part: an unchanged
  # crate should cost nothing. Newest by creation date, which is what "our last
  # publish" actually means — refname sorting would have to reason about
  # pre-release suffixes to agree. Read rather than `head`: `head` closing the
  # pipe early surfaces as a SIGPIPE failure under `pipefail`.
  IFS= read -r last_tag < <(git -C "$ROOT" tag --list "$TAG_PREFIX$name-v*" --sort=-creatordate; printf '\n') || true
  if [ "$FORCE" = 1 ]; then
    echo "  change check: forced"
  elif [ -z "$last_tag" ]; then
    echo "  change check: no $TAG_PREFIX$name-v* tag yet — first publish"
  else
    # test/ is our negative corpus: it gates the release but never ships, so
    # corpus-only work does not cut one. Everything else under the crate dir is
    # either the submodule pointer, a patch, or the provenance that ships beside
    # the crate — all of which change what a consumer would get. ARTIFACT_INPUTS
    # covers the same question for files outside the directory.
    if git -C "$ROOT" diff --quiet "$last_tag" HEAD \
         -- "$rel" ":(exclude)$rel/test" "${ARTIFACT_INPUTS[@]}"; then
      echo "  change check: unchanged since $last_tag — skipping"
      skipped+=("$name (unchanged since $last_tag)")
      continue
    fi
    echo "  change check: changed since $last_tag"
  fi

  # ---- materialize + gate --------------------------------------------------
  # Either path leaves build/ freshly materialized; that tree is what ships.
  # The log goes to the staging root rather than anywhere under the crate dir,
  # which materialize.sh treats as a source of truth.
  vlog="$STAGE_ROOT/verify-$name.log"
  if [ "$SKIP_VERIFY" = 1 ]; then
    echo "  materialize: scripts/materialize.sh $rel (tests skipped; caller asserts CI ran them)"
    if ! "$ROOT/scripts/materialize.sh" "$dir" > "$vlog" 2>&1; then
      echo "publish: FAIL — $rel does not materialize; refusing to publish" >&2
      tail -40 "$vlog" >&2
      overall=1; continue
    fi
  else
    echo "  verify: scripts/verify.sh $rel"
    if ! "$ROOT/scripts/verify.sh" "$dir" > "$vlog" 2>&1; then
      echo "publish: FAIL — $rel does not verify; refusing to publish" >&2
      tail -40 "$vlog" >&2
      overall=1; continue
    fi
  fi
  echo "  materialize: ok"

  build="$dir/build"
  # Read name/version from [package] only: [lib] and the dependency tables have
  # their own keys, and matching one of those would be silently wrong. `sed -n
  # 0,/re/` stops at the first match rather than piping into `head`.
  pkg=$(sed -n '/^\[package\]/,/^\[lib\]/p' "$build/Cargo.toml")
  manifest_name=$(printf '%s\n' "$pkg" | sed -n '0,/^name *= *"/s/^name *= *"\(.*\)"/\1/p')
  manifest_version=$(printf '%s\n' "$pkg" | sed -n '0,/^version *= *"/s/^version *= *"\(.*\)"/\1/p')

  if [ "$manifest_name" != "$name" ]; then
    echo "publish: FAIL — $rel materializes as \"$manifest_name\", expected \"$name\"" >&2
    if [ "${manifest_name#tree-sitter}" != "$manifest_name" ]; then
      echo "         that is upstream's crate name, which we do not own. This grammar is" >&2
      echo "         missing its \"treebank crate identity\" patch — see PUBLISHING.md," >&2
      echo "         \"Adding a new grammar\"." >&2
    fi
    overall=1; continue
  fi
  # The manifest version IS the publish base. If they ever disagree, the suffix
  # would be computed against a version nobody declared.
  if [ "$manifest_version" != "$base" ]; then
    echo "publish: FAIL — Cargo.toml version ($manifest_version) != ledger upstream.version ($base)" >&2
    echo "         these must agree; the published version is the ledger version plus a suffix" >&2
    overall=1; continue
  fi

  next=$(( $(published_suffix_max "$name" "$base") + 1 ))
  version="$base-treebank.$next"
  echo "  version: $version  (upstream $base + treebank suffix $next)"

  # ---- stage ---------------------------------------------------------------
  # A copy, so the version and sha-bearing description are stamped into nothing
  # that survives the run. build/ is a throwaway git repo (materialize.sh commits
  # it so grammar edits show up as a diff); dropping .git means cargo sees a
  # plain directory and needs no --allow-dirty.
  stage=$(mktemp -d "$STAGE_ROOT/$name.XXXXXX")
  cp -R "$build/." "$stage/crate"
  rm -rf "$stage/crate/.git" "$stage/crate/target" "$stage/crate/node_modules"
  # Provenance lives in the grammar dir, not in build/, but the crate's include
  # list names it: the ledger, the patch series and their prose ship inside the
  # tarball so a consumer can reconstruct what they were given.
  cp "$dir/ledger.json" "$dir/LOCAL-PATCHES.md" "$stage/crate/"
  cp -R "$dir/patches" "$stage/crate/patches"

  desc="Patched redistribution of $upstream_url at $base (${sha:0:7}) with treebank's $grammar_patches parser-fix patches applied; materialized from that pinned commit and verified against its corpus. Not an upstream release."

  VERSION="$version" DESC="$desc" NAME="$name" python3 - "$stage/crate" <<'PY'
import os, re, sys
d = sys.argv[1]
version, desc, name = os.environ['VERSION'], os.environ['DESC'], os.environ['NAME']

def toml_str(v):
    return '"%s"' % v.replace('\\', '\\\\').replace('"', '\\"')

p = os.path.join(d, 'Cargo.toml')
s = open(p).read()
# Rewrite only within [package]: [lib] and the dependency tables carry their own
# version keys, and clobbering one of those would be silent and wrong.
head, sep, tail = s.partition('\n[lib]')
assert sep, 'no [lib] section; refusing to guess where [package] ends'
head, n = re.subn(r'(?m)^version = "[^"]*"$', 'version = %s' % toml_str(version), head, count=1)
assert n == 1, 'expected exactly one version key in [package], found %d' % n
head, n = re.subn(r'(?m)^description = ".*"$', 'description = %s' % toml_str(desc), head, count=1)
assert n == 1, 'expected exactly one description key in [package], found %d' % n
open(p, 'w').write(head + sep + tail)

# Not every upstream ships a lockfile -- tree-sitter-elixir is the first here
# that does not -- and a library does not need one to publish: cargo resolves
# and writes its own. When there IS one it must be kept in step with the
# version rewritten above, so the entry is still asserted rather than
# best-effort; it is only the FILE that is optional.
p = os.path.join(d, 'Cargo.lock')
if os.path.exists(p):
    s = open(p).read()
    s, n = re.subn(r'(?m)^(name = "%s"\nversion = )"[^"]*"' % re.escape(name),
                   r'\1"%s"' % version, s, count=1)
    assert n == 1, 'package %s not found in Cargo.lock' % name
    open(p, 'w').write(s)
PY

  # ---- package / publish ---------------------------------------------------
  if [ "$MODE" = dry-run ]; then
    echo "  dry run: cargo package (builds the tarball and compiles it; uploads nothing)"
    if (cd "$stage/crate" && cargo package --quiet); then
      echo "  dry run: ok — $name $version would publish"
      published+=("$name $version (dry run)")
    else
      echo "publish: FAIL — $rel does not package" >&2
      overall=1
    fi
  else
    echo "  publishing $name $version${REGISTRY:+ to registry $REGISTRY}"
    # No --no-verify: cargo compiles the packaged tarball before uploading, which
    # is the last chance to catch a crate that packages but does not build.
    # `if`, not `[ ... ] && ...`: as a bare statement a false test returns 1 and
    # `set -e` would abort the run for the crates.io case.
    pub_args=(publish)
    if [ -n "$REGISTRY" ]; then pub_args+=(--registry "$REGISTRY"); fi
    if (cd "$stage/crate" && cargo "${pub_args[@]}"); then
      published+=("$name $version")
      if [ "$DO_TAG" = 1 ]; then
        tag="$TAG_PREFIX$name-v$version"
        git -C "$ROOT" tag -a "$tag" -m "$name $version (upstream $base @ ${sha:0:7})"
        echo "  tagged $tag"
        if [ "$DO_PUSH" = 1 ]; then
          git -C "$ROOT" push origin "$tag"
          echo "  pushed $tag"
        fi
      fi
    else
      echo "publish: FAIL — $name $version did not publish" >&2
      overall=1
    fi
  fi

  rm -rf "$stage"
done

echo "=============================================================="
for p in "${published[@]}"; do echo "  published: $p"; done
for s in "${skipped[@]}"; do echo "  skipped:   $s"; done
[ "$overall" = 0 ] || echo "  one or more crates failed" >&2
exit "$overall"
