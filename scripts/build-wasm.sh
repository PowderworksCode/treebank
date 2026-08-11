#!/usr/bin/env bash
# Build a treebank wasm pack: one grammar, self-contained, byte-reproducible.
#
#   crates/treebank-<lang>/build/   (scripts/materialize.sh's output)
#   + vendor/tree-sitter @ the runtime sha the pinned CLI was cut from
#   + tools/wasm-pack/shim.c        (the pack ABI)
#   + provenance generated from ledger.json, linked INTO the module
#   -> dist/wasm/treebank-<lang>.wasm
#
# The pack is a standalone wasm module: it imports only WASI, so any runtime
# can load it with no emscripten glue and no native tree-sitter. See
# tools/wasm-pack/shim.c for the ABI and WASM-PACKS.md for why this format.
#
# THREE PINS, and all three are load-bearing:
#
#   generate_cli   the ledger's existing pin, which produced src/parser.c
#   runtime        vendor/tree-sitter, at the commit that CLI was cut from —
#                  the runtime must understand the language ABI the CLI emits
#   wasi-sdk       version AND sha256 per platform, in
#                  tools/wasm-pack/toolchain.sh. Measured: two compiler
#                  versions turn identical sources into different bytes, so
#                  this has exactly the exposure generate_cli has.
#
# The compiler is downloaded, hash-verified and cached outside the repo; it is
# never taken from PATH. A contributor's own clang or emcc cannot influence the
# output, which is the whole point — `tree-sitter build --wasm` gets this wrong
# by preferring a local emcc, so a machine with emscripten installed silently
# produces different bytes from CI.
#
# Usage: scripts/build-wasm.sh <grammar-dir> [generate-dir]
#   scripts/build-wasm.sh crates/treebank-python
#   scripts/build-wasm.sh crates/treebank-typescript tsx
#
#   --out DIR     where to write (default dist/wasm)
#   --skip-materialize   use build/ as it stands (caller asserts it is fresh)
set -euo pipefail
shopt -s nullglob

ROOT=$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)

# The toolchain pin and its rationale live in their own file: changing it
# changes every pack's bytes and should be reviewable on its own.
# shellcheck source=../tools/wasm-pack/toolchain.sh
. "$ROOT/tools/wasm-pack/toolchain.sh"

OUT="$ROOT/dist/wasm"
SKIP_MATERIALIZE=0
ARGS=()
while [ $# -gt 0 ]; do
  case "$1" in
    --out)              OUT=$2; shift ;;
    --skip-materialize) SKIP_MATERIALIZE=1 ;;
    -h|--help)          sed -n '2,40p' "${BASH_SOURCE[0]}"; exit 0 ;;
    -*)                 echo "build-wasm: unknown flag $1" >&2; exit 2 ;;
    *)                  ARGS+=("$1") ;;
  esac
  shift
done

GRAMMAR_DIR=$(cd "${ARGS[0]:?usage: build-wasm.sh <grammar-dir> [generate-dir]}" && pwd)
GEN_DIR=${ARGS[1]:-.}
[ -f "$GRAMMAR_DIR/ledger.json" ] || { echo "build-wasm: $GRAMMAR_DIR has no ledger.json" >&2; exit 2; }

LANG_NAME=$(jq -r .grammar "$GRAMMAR_DIR/ledger.json")
# One pack per generated grammar, not per directory: typescript ships two
# (typescript, tsx) and they are different parsers with different tables.
if [ "$GEN_DIR" = "." ]; then PACK="treebank-$LANG_NAME"; else PACK="treebank-$(basename "$GEN_DIR")"; fi

SRC="$GRAMMAR_DIR/build/$GEN_DIR/src"
RUNTIME="$ROOT/vendor/tree-sitter"

# Set after materialization, from the grammar's own declaration; see ENTRY below.
LANGUAGE_NAME=""

# ---- preconditions -------------------------------------------------------
# Initialized on demand, as materialize.sh does for a grammar's upstream: CI
# checks out without submodules so a job fetches only what it needs.
if [ ! -e "$RUNTIME/lib/src/lib.c" ]; then
  echo "build-wasm: initializing vendor/tree-sitter submodule"
  git -C "$ROOT" submodule update --init --depth 1 -- vendor/tree-sitter
fi
RUNTIME_SHA=$(git -C "$RUNTIME" rev-parse HEAD)
if [ -n "$(git -C "$RUNTIME" status --porcelain)" ]; then
  echo "build-wasm: FAIL — vendor/tree-sitter is dirty; the runtime pin must be pristine" >&2
  exit 1
fi
# The runtime and the CLI that generated parser.c must agree: the runtime has
# to understand the language ABI the CLI emits. They ship from one repo at one
# version, so this is checkable rather than assumed.
CLI_WANT=$(jq -r .generate_cli "$GRAMMAR_DIR/ledger.json")
RUNTIME_VERSION=$(sed -n 's/^version = "\(.*\)"$/\1/p' "$RUNTIME/Cargo.toml" | head -1)
if [ "$RUNTIME_VERSION" != "$CLI_WANT" ]; then
  echo "build-wasm: FAIL — runtime pin is tree-sitter $RUNTIME_VERSION but the ledger's generate_cli is $CLI_WANT" >&2
  echo "  these must match: the runtime linked into the pack has to load the ABI that CLI generates" >&2
  exit 1
fi

if [ "$SKIP_MATERIALIZE" = 0 ]; then
  "$ROOT/scripts/materialize.sh" "$GRAMMAR_DIR" >/dev/null
fi
[ -f "$SRC/parser.c" ] || { echo "build-wasm: no $SRC/parser.c — did materialize run?" >&2; exit 1; }
[ -f "$SRC/grammar.json" ] || { echo "build-wasm: no $SRC/grammar.json — did materialize run?" >&2; exit 1; }
LANGUAGE_NAME=$(jq -r .name "$SRC/grammar.json")
[ -n "$LANGUAGE_NAME" ] && [ "$LANGUAGE_NAME" != null ] \
  || { echo "build-wasm: $SRC/grammar.json declares no name" >&2; exit 1; }

WASI_SDK=$(wasi_sdk_ensure) || exit 1
BINARYEN=$(binaryen_ensure) || exit 1

# ---- provenance ----------------------------------------------------------
# Linked into the module, so a .wasm that gets vendored into someone's repo and
# rediscovered later still answers for itself. Deliberately contains NOTHING
# that varies without changing the artifact: no timestamp, no build host, no
# treebank commit. Anything ambient in here would break byte-reproducibility,
# which is the property the provenance exists to make checkable.
STAGE=$(mktemp -d)
trap 'rm -rf "$STAGE"' EXIT

# Every patch, with the hash of the patch FILE. That is what makes the series
# checkable: a consumer can fetch patches/ from this repo, hash them, and know
# they are looking at the divergence this pack was built from.
PATCH_MANIFEST=$(
  for p in "$GRAMMAR_DIR"/patches/*.patch; do
    jq -n --arg file "$(basename "$p")" --arg sha "$(sha256sum "$p" | cut -d' ' -f1)" \
      '{($file): $sha}'
  done | jq -sc 'add // {}'
)

# `grammar` and `language_name` are two different facts and are recorded
# separately because they are allowed to differ: `grammar` is the directory-derived
# name treebank uses (csharp, matching the crate), `language_name` is what the
# grammar calls itself and what ts_language_name returns (c_sharp). Anything
# comparing a pack against a ledger has to compare the right one, exactly —
# normalising them together would hide a pack built from the wrong grammar.
jq -S \
  --arg pack "$PACK" \
  --arg format standalone \
  --arg language_name "$LANGUAGE_NAME" \
  --arg cli "$CLI_WANT" \
  --arg rt_sha "$RUNTIME_SHA" \
  --arg rt_ver "$RUNTIME_VERSION" \
  --arg sdk_ver "$WASI_SDK_VERSION" \
  --arg sdk_plat "$(wasi_sdk_platform)" \
  --arg sdk_sha "$(wasi_sdk_sha256 "$(wasi_sdk_platform)")" \
  --arg bin_ver "$BINARYEN_VERSION" \
  --arg bin_sha "$(binaryen_sha256 "$(wasi_sdk_platform)")" \
  --argjson patch_files "$PATCH_MANIFEST" \
  '{
     pack: $pack,
     pack_abi: 1,
     format: $format,
     grammar: .grammar,
     language_name: $language_name,
     upstream: .upstream,
     toolchain: {
       generate_cli: {tool: "tree-sitter-cli", version: $cli},
       runtime:      {tool: "tree-sitter", version: $rt_ver, sha: $rt_sha},
       wasi_sdk:     {tool: "wasi-sdk", version: $sdk_ver, platform: $sdk_plat, sha256: $sdk_sha},
       binaryen:     {tool: "binaryen", version: $bin_ver, platform: $sdk_plat, sha256: $bin_sha}
     },
     patches: [.patches[] | {id, title, kind: (.kind // "grammar"),
                             file: (.file | sub("^patches/"; "")),
                             sha256: $patch_files[.file | sub("^patches/"; "")]}],
     sweep: (.corpus | {upstream: (.sweep_upstream // .sweep_upstream_committed), patched: .sweep_patched}),
     license: "See LICENSE in the upstream grammar repository; patches are treebank'"'"'s and carry the same terms.",
     note: "Patched redistribution. This pack gives you treebank'"'"'s patched grammar, NOT the corpus sweeps or reference-compiler oracle those patches were derived from."
   }' "$GRAMMAR_DIR/ledger.json" > "$STAGE/provenance.json"

# As a C string literal, byte for byte, with an explicit length so a host can
# read it without scanning for a NUL.
python3 - "$STAGE/provenance.json" "$STAGE/provenance.c" <<'PY'
import json, sys
raw = open(sys.argv[1], 'rb').read().rstrip(b'\n')
esc = ''.join('\\%03o' % b if (b < 32 or b > 126 or chr(b) in '"\\?') else chr(b) for b in raw)
with open(sys.argv[2], 'w') as f:
    f.write('/* generated by scripts/build-wasm.sh; do not edit */\n')
    f.write('const char treebank_provenance[] = "%s";\n' % esc)
    f.write('const unsigned treebank_provenance_len = %d;\n' % len(raw))
PY

# ---- compile -------------------------------------------------------------
# The entry point is DECLARED, not detected. src/grammar.json is generate's own
# record of what this grammar is called; ts_language_name() returns that exact
# string and the entry symbol is exactly "tree_sitter_<name>". Deriving it from
# the directory would be wrong wherever the two differ — the C# grammar calls
# itself c_sharp while its directory is csharp — and pattern-matching parser.c
# for something that looks like an entry point is a guess that happens to work.
ENTRY="tree_sitter_$LANGUAGE_NAME"
if ! grep -q "TSLanguage \*$ENTRY(void)" "$SRC/parser.c"; then
  echo "build-wasm: FAIL — $SRC/grammar.json declares \"$LANGUAGE_NAME\" but" >&2
  echo "  $SRC/parser.c defines no $ENTRY(). The generated sources disagree with" >&2
  echo "  each other; re-run scripts/materialize.sh." >&2
  exit 1
fi
echo "build-wasm: $PACK  (entry $ENTRY, runtime $RUNTIME_VERSION, wasi-sdk $WASI_SDK_VERSION, binaryen $BINARYEN_VERSION)"

mkdir -p "$STAGE/g" "$OUT"
# The WHOLE materialized tree, structure preserved — not just src/. typescript's
# scanner.c includes "../../common/scanner.h", a path that only resolves if the
# grammar keeps its shape, and flattening src/ into one directory breaks it.
tar -C "$GRAMMAR_DIR/build" --exclude=.git --exclude=node_modules --exclude=target \
    -cf - . | tar -C "$STAGE/g" -xf -
if [ "$GEN_DIR" = "." ]; then REL_SRC="g/src"; else REL_SRC="g/$GEN_DIR/src"; fi
cp "$ROOT/tools/wasm-pack/shim.c" "$STAGE/shim.c"
cp -R "$RUNTIME/lib/src" "$STAGE/rt-src"
cp -R "$RUNTIME/lib/include" "$STAGE/rt-include"

# -mexec-model=reactor: the module is a library the host calls into, not a
# command with a main(). Losing this (LTO does) breaks every WASI host.
# --strip-all: drops the ~155 KB name section, which also carries the output
# FILENAME — the one part of a wasi-sdk build that is not a pure function of
# the inputs. See tools/wasm-pack/toolchain.sh for why -O3 and why no LTO.
#
# Sources are passed by relative path, from inside the staging directory, so
# nothing about this machine's layout can reach the output.
SCANNER=""
[ -f "$STAGE/$REL_SRC/scanner.c" ] && SCANNER="$REL_SRC/scanner.c"
( cd "$STAGE" && "$WASI_SDK/bin/clang" \
    --target=wasm32-wasip1 -mexec-model=reactor -O3 \
    --sysroot="$WASI_SDK/share/wasi-sysroot" \
    -DTREEBANK_LANGUAGE_FN="$ENTRY" \
    -I rt-include -I rt-src -I "$REL_SRC" \
    rt-src/lib.c "$REL_SRC/parser.c" $SCANNER shim.c provenance.c \
    -Wl,--export-memory -Wl,--strip-all \
    -o linked.wasm )

# lld leaves one data segment covering the whole static image. wasm-opt's
# memory packing splits it and drops the zero runs parse tables are full of,
# which is most of the artifact for a large grammar — see toolchain.sh. `-all`
# because wasi-sdk emits bulk-memory and non-trapping float ops that wasm-opt
# will not validate under its default feature set.
( cd "$STAGE" && "$BINARYEN/bin/wasm-opt" -all -O3 \
    --strip-producers --strip-debug linked.wasm -o out.wasm )

install -m 0644 "$STAGE/out.wasm" "$OUT/$PACK.wasm"
cp "$STAGE/provenance.json" "$OUT/$PACK.json"

SIZE=$(stat -c%s "$OUT/$PACK.wasm")
SHA=$(sha256sum "$OUT/$PACK.wasm" | cut -d' ' -f1)
echo "build-wasm: ok — $OUT/$PACK.wasm  ${SIZE} bytes  sha256:${SHA:0:16}…"
