#!/usr/bin/env bash
# The wasm toolchain pin, and how to get it.
#
# Sourced by scripts/build-wasm.sh. Kept separate because it is the piece with
# the same weight as ledger.json's generate_cli: changing anything here changes
# every pack's bytes, and it should be reviewable on its own.
#
# WHY wasi-sdk AND NOT EMSCRIPTEN. Both produce byte-reproducible output and
# both work — this was measured, not assumed. The trade:
#
#   emscripten 4.0.4          2.81 GB Docker image
#   wasi-sdk 33 + binaryen    193 MB + 103 MB, plain tarballs, no Docker
#
# 14x less toolchain, no Docker dependency, and output within 0.2% of
# emscripten on size and within 1% on speed. At one grammar that is a wash; at
# twenty, where every CI job would otherwise pull 2.81 GB from a rate-limited
# registry, it is the right trade. It is also where upstream tree-sitter went:
# 0.26.1 replaced emscripten with wasi-sdk for `tree-sitter build --wasm`.
#
# WHY BINARYEN IS NOT OPTIONAL. lld emits one data segment spanning the whole
# static image; emscripten runs wasm-opt, whose memory-packing pass splits it
# and drops the long zero runs that parse tables are full of. The effect scales
# with table size and is invisible on small grammars, which is exactly how it
# gets missed:
#
#                    lld alone    + wasm-opt -O3    emscripten
#   python             628,003         564,805        560,956
#   rust             1,348,851         894,378        887,233
#   csharp           5,647,046       3,073,926      3,069,052
#
# Without wasm-opt, csharp is 84% larger. wasm-opt needs `-all`: wasi-sdk 33
# emits bulk-memory and non-trapping float ops that it refuses to validate
# under its default feature set.
#
# WHY -O3 AND NOT -Oz. Measured on 497 files / 10.1 MB of CPython's stdlib:
#
#   -O3   624,787 bytes   7,343 bytes/ms
#   -O2   611,245 bytes   6,608 bytes/ms
#   -Oz   588,207 bytes   5,348 bytes/ms
#
# -Oz buys 6% size for 28% throughput. Packs are meant to parse corpora, so
# that is the wrong direction.
#
# WHY NO LTO. `-flto` produces a module that exports _start instead of
# _initialize — the reactor exec model is silently lost and every WASI host
# refuses to instantiate it. It also made the module BIGGER (967 KB). Do not
# re-add it without checking the exports.
#
# WHY --strip-all. The name section is ~155 KB of the unstripped module, and it
# carries the output FILENAME, which is the one thing in a wasi-sdk build that
# is not a pure function of the inputs. Stripping it removes both problems.
# Nothing else about the build varies: identical inputs give identical bytes,
# from any directory.
#
# No binaryen. wasm-opt closes about 5% more and is another tool to pin.

WASI_SDK_VERSION="33.0"
WASI_SDK_TAG="wasi-sdk-33"
BINARYEN_VERSION="131"

# The tree-sitter RUNTIME source, linked into every pack. Pinned to the same
# version as the generate CLI (ledger generate_cli): the runtime must
# understand the language ABI the CLI emits, so the two move together.
#
# Fetched and hash-verified rather than vendored as a submodule. Treebank
# owns its grammars and carries no vendored trees; the runtime is a
# toolchain input like wasi-sdk and binaryen, and is cached the same way.
RUNTIME_VERSION="0.26.12"
RUNTIME_SHA256="428e2b182fe38eddc100d8bd851e47c96921a69281b66abafc25ba4b0aaeeeab"

# sha256 per platform, computed by downloading each release asset. Upstream
# publishes no checksum file, so these are ours. A contributor on a platform
# not listed here gets a clear failure rather than an unverified download.
wasi_sdk_sha256() {
  case "$1" in
    x86_64-linux) echo 0ba8b5bfaeb2adf3f29bab5841d76cf5318ab8e1642ea195f88baba1abd47bce ;;
    arm64-linux)  echo 4f98ee738c7abb45c81a94d1461fc53cc569d1cd01498951c8184d841a027844 ;;
    x86_64-macos) echo 18f3f201ba9734e6a4455b0b6410690395a55e9ffa9f6f5066f66083a94b93b3 ;;
    arm64-macos)  echo 85c997a2665ead91673b5bb88b7d0df3fc8900df3bfa244f720d478187bbdc78 ;;
    *)            echo "" ;;
  esac
}

wasi_sdk_platform() {
  local os arch
  case "$(uname -s)" in
    Linux)  os=linux ;;
    Darwin) os=macos ;;
    *)      echo ""; return ;;
  esac
  case "$(uname -m)" in
    x86_64|amd64)  arch=x86_64 ;;
    arm64|aarch64) arch=arm64 ;;
    *)             echo ""; return ;;
  esac
  echo "$arch-$os"
}

# Prints the sysroot-bearing directory, downloading and verifying it once.
# Cached outside the repo: it is a toolchain, not a source of truth, and it is
# 646 MB unpacked.
wasi_sdk_ensure() {
  local cache="${TREEBANK_WASM_CACHE:-${XDG_CACHE_HOME:-$HOME/.cache}/treebank}"
  local plat want dir url tgz got
  plat=$(wasi_sdk_platform)
  if [ -z "$plat" ]; then
    echo "toolchain: unsupported platform $(uname -s)/$(uname -m)" >&2
    echo "  add its sha256 to tools/wasm-pack/toolchain.sh" >&2
    return 1
  fi
  want=$(wasi_sdk_sha256 "$plat")
  if [ -z "$want" ]; then
    echo "toolchain: no pinned sha256 for platform $plat" >&2
    echo "  add it to wasi_sdk_sha256() in tools/wasm-pack/toolchain.sh" >&2
    return 1
  fi
  dir="$cache/wasi-sdk-$WASI_SDK_VERSION-$plat"
  if [ -x "$dir/bin/clang" ]; then printf '%s\n' "$dir"; return 0; fi

  url="https://github.com/WebAssembly/wasi-sdk/releases/download/$WASI_SDK_TAG/wasi-sdk-$WASI_SDK_VERSION-$plat.tar.gz"
  tgz="$cache/.wasi-sdk-$plat.tar.gz"
  mkdir -p "$cache"
  echo "toolchain: fetching wasi-sdk $WASI_SDK_VERSION for $plat" >&2
  curl -fsSL --retry 3 "$url" -o "$tgz" || { echo "toolchain: download failed: $url" >&2; return 1; }
  got=$(sha256sum "$tgz" | cut -d' ' -f1)
  if [ "$got" != "$want" ]; then
    rm -f "$tgz"
    echo "toolchain: FAIL — sha256 mismatch for wasi-sdk $WASI_SDK_VERSION $plat" >&2
    echo "  want $want" >&2
    echo "  got  $got" >&2
    return 1
  fi
  rm -rf "$dir.tmp" && mkdir -p "$dir.tmp"
  tar xzf "$tgz" -C "$dir.tmp" --strip-components=1
  rm -f "$tgz"
  mv "$dir.tmp" "$dir"
  printf '%s\n' "$dir"
}

# Binaryen ships a checksum file beside each asset, but these are ours: a
# checksum served from the same place as the artifact proves only that the
# download completed.
binaryen_sha256() {
  case "$1" in
    x86_64-linux) echo b5bf1f0eaf17c63ee588ff7a5954dc8f6ce2c26989051c66f24dfe9ece3e46db ;;
    arm64-linux)  echo ba991f677edd9a21d2bc96c0144bc8ac5b112d4d98a3eb266e075e22e557df2a ;;
    x86_64-macos) echo d209fadd8a894bdaf3bd3612a23c32a0af184d2f4a979b8c789e6e4f6a4de883 ;;
    arm64-macos)  echo e441b48dc22163d209b4f05e44dc7210909b01237642b6c9ae48fd710a3ef83e ;;
    *)            echo "" ;;
  esac
}

# Binaryen calls 64-bit arm on Linux "aarch64" and on macOS "arm64".
binaryen_asset_platform() {
  case "$1" in
    arm64-linux) echo aarch64-linux ;;
    *)           echo "$1" ;;
  esac
}

# Prints the directory holding bin/wasm-opt, downloading and verifying once.
binaryen_ensure() {
  local cache="${TREEBANK_WASM_CACHE:-${XDG_CACHE_HOME:-$HOME/.cache}/treebank}"
  local plat want dir url tgz got asset
  plat=$(wasi_sdk_platform)
  [ -n "$plat" ] || { echo "toolchain: unsupported platform $(uname -s)/$(uname -m)" >&2; return 1; }
  want=$(binaryen_sha256 "$plat")
  if [ -z "$want" ]; then
    echo "toolchain: no pinned binaryen sha256 for platform $plat" >&2
    echo "  add it to binaryen_sha256() in tools/wasm-pack/toolchain.sh" >&2
    return 1
  fi
  dir="$cache/binaryen-$BINARYEN_VERSION-$plat"
  if [ -x "$dir/bin/wasm-opt" ]; then printf '%s\n' "$dir"; return 0; fi

  asset=$(binaryen_asset_platform "$plat")
  url="https://github.com/WebAssembly/binaryen/releases/download/version_$BINARYEN_VERSION/binaryen-version_$BINARYEN_VERSION-$asset.tar.gz"
  tgz="$cache/.binaryen-$plat.tar.gz"
  mkdir -p "$cache"
  echo "toolchain: fetching binaryen $BINARYEN_VERSION for $plat" >&2
  curl -fsSL --retry 3 "$url" -o "$tgz" || { echo "toolchain: download failed: $url" >&2; return 1; }
  got=$(sha256sum "$tgz" | cut -d' ' -f1)
  if [ "$got" != "$want" ]; then
    rm -f "$tgz"
    echo "toolchain: FAIL — sha256 mismatch for binaryen $BINARYEN_VERSION $plat" >&2
    echo "  want $want" >&2
    echo "  got  $got" >&2
    return 1
  fi
  rm -rf "$dir.tmp" && mkdir -p "$dir.tmp"
  tar xzf "$tgz" -C "$dir.tmp" --strip-components=1
  rm -f "$tgz"
  mv "$dir.tmp" "$dir"
  printf '%s\n' "$dir"
}

# Prints the directory holding lib/src/lib.c, downloading and verifying once.
runtime_ensure() {
  local cache="${TREEBANK_WASM_CACHE:-${XDG_CACHE_HOME:-$HOME/.cache}/treebank}"
  local dir tgz got
  dir="$cache/tree-sitter-$RUNTIME_VERSION"
  if [ -e "$dir/lib/src/lib.c" ]; then printf '%s\n' "$dir"; return 0; fi

  mkdir -p "$cache"
  tgz="$cache/tree-sitter-$RUNTIME_VERSION.tar.gz"
  echo "toolchain: fetching tree-sitter runtime $RUNTIME_VERSION" >&2
  curl -sSL -o "$tgz" \
    "https://github.com/tree-sitter/tree-sitter/archive/refs/tags/v$RUNTIME_VERSION.tar.gz" || return 1

  got=$(sha256sum "$tgz" | cut -d' ' -f1)
  if [ "$got" != "$RUNTIME_SHA256" ]; then
    echo "toolchain: FAIL - sha256 mismatch for tree-sitter runtime $RUNTIME_VERSION" >&2
    echo "  want $RUNTIME_SHA256" >&2
    echo "  got  $got" >&2
    rm -f "$tgz"
    return 1
  fi

  rm -rf "$dir"
  tar xzf "$tgz" -C "$cache" || return 1
  rm -f "$tgz"
  [ -e "$dir/lib/src/lib.c" ] || { echo "toolchain: runtime tarball has an unexpected layout" >&2; return 1; }
  printf '%s\n' "$dir"
}
