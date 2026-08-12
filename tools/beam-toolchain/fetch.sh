#!/usr/bin/env bash
# Install a pinned BEAM toolchain — Erlang/OTP plus Elixir — without root and
# without building anything from source.
#
# This is deliberately NOT an Elixir detail. Two treebank grammars sit on the
# BEAM: elixir's oracle is `Code.string_to_quoted/2` and erlang's is
# `epp_dodger` (in OTP's own syntax_tools). An Erlang session needs only the
# OTP half of this script and can ignore the Elixir half entirely; nothing
# below is Elixir-specific until the last stanza.
#
# WHERE THE BINARIES COME FROM. builds.hex.pm is the Hex team's build host —
# the same one `erlef/setup-beam` (the official Erlang Ecosystem Foundation
# GitHub Action) pulls from. OTP is published there per distribution, so
# `ubuntu-24.04` is a build against this box's OpenSSL and ncurses rather
# than a generic tarball hoping for the best. Measured: 77 MB of OTP and
# 8.6 MB of Elixir, 14 s to fetch, ~90 s wall from nothing to a working
# `iex`. Building OTP from source (kerl/asdf, the usual path) is 10-20 min
# of compile for the same result.
#
# Two distribution alternatives, both rejected:
#   - `apt install elixir` gives Elixir 1.14.0 on OTP 25, both from 2022.
#     The oracle version IS the dialect (GRAMMARS.md, "Why the oracle is
#     pinned too"), and three years of parser changes is not a rounding
#     error.
#   - Erlang Solutions' apt repo (binaries2.erlang-solutions.com) is not
#     reachable from this box's proxy; builds.hex.pm is.
#
# WHICH VERSIONS, AND WHY THESE. Pinned exactly, because a different OTP or
# Elixir silently changes what "invalid" means:
#
#   OTP 28.5.0.5   newest patch of the mature OTP 28 line. Elixir 1.20
#                  requires OTP 27+; OTP 29 is four months old and its only
#                  benefit here would be novelty.
#   Elixir 1.20.3  newest stable (2026-08-04). The build is chosen to match
#                  the OTP major (`v1.20.3-otp-28.zip`) — Elixir ships one
#                  precompiled artifact per OTP major and mixing them is
#                  unsupported.
#
# An Erlang session that wants the same OTP does NOT need to re-run this:
# the install is shared at $PREFIX and `--otp-only` skips the Elixir half.
# If it needs a different OTP, add a second versioned directory rather than
# overwriting this one — two OTPs coexist fine, and the elixir grammar's
# ledger names the one its sweep numbers were produced with.
#
# Usage: tools/beam-toolchain/fetch.sh [--otp-only] [--print]
set -euo pipefail

OTP_VERSION=28.5.0.5
OTP_SHA256=e3476633cae6fef8e1bb53576832b15823f715e70e3d4d1e66a6be908804f967
ELIXIR_VERSION=1.20.3
ELIXIR_OTP_MAJOR=28
ELIXIR_SHA256=8100b91201ddf75f760954e570069b7d43a1c27a3099d65e27a1cd9d539ae51b
DISTRO=ubuntu-24.04

PREFIX="${TREEBANK_BEAM_PREFIX:-$HOME/.local/beam}"
OTP_DIR="$PREFIX/otp-$OTP_VERSION"
ELIXIR_DIR="$PREFIX/elixir-$ELIXIR_VERSION-otp-$ELIXIR_OTP_MAJOR"

otp_only=false; print_only=false
for arg in "$@"; do
  case "$arg" in
    --otp-only) otp_only=true ;;
    --print) print_only=true ;;
    *) echo "unknown argument: $arg" >&2; exit 2 ;;
  esac
done

if $print_only; then
  $otp_only && echo "$OTP_DIR/bin" || echo "$OTP_DIR/bin:$ELIXIR_DIR/bin"
  exit 0
fi

# Checksums are from builds.hex.pm's own builds.txt (4th field), fetched at
# pin time. They are verified here because this script installs an
# interpreter that decides every verdict the grammar is measured against.
verify() {
  local file=$1 want=$2 got
  got=$(sha256sum "$file" | cut -d' ' -f1)
  if [ "$got" != "$want" ]; then
    echo "checksum mismatch for $file" >&2
    echo "  want $want" >&2
    echo "  got  $got" >&2
    exit 1
  fi
}

tmp=$(mktemp -d); trap 'rm -rf "$tmp"' EXIT

if [ ! -x "$OTP_DIR/bin/erl" ]; then
  echo "beam-toolchain: fetching Erlang/OTP $OTP_VERSION ($DISTRO)" >&2
  curl -fsSL -o "$tmp/otp.tar.gz" \
    "https://builds.hex.pm/builds/otp/$DISTRO/OTP-$OTP_VERSION.tar.gz"
  verify "$tmp/otp.tar.gz" "$OTP_SHA256"
  rm -rf "$OTP_DIR"; mkdir -p "$OTP_DIR"
  tar xzf "$tmp/otp.tar.gz" --strip-components=1 -C "$OTP_DIR"
  # Install rewrites the absolute paths baked into erl/start scripts at build
  # time; without it `erl` looks for its root under the builder's directory.
  (cd "$OTP_DIR" && ./Install -minimal "$OTP_DIR" >/dev/null)
fi
"$OTP_DIR/bin/erl" -noshell -eval \
  'io:format("beam-toolchain: OTP ~s erts-~s~n",[erlang:system_info(otp_release),erlang:system_info(version)]),halt().' >&2

$otp_only && { echo "$OTP_DIR/bin"; exit 0; }

if [ ! -x "$ELIXIR_DIR/bin/elixir" ]; then
  echo "beam-toolchain: fetching Elixir $ELIXIR_VERSION (otp-$ELIXIR_OTP_MAJOR)" >&2
  curl -fsSL -o "$tmp/elixir.zip" \
    "https://builds.hex.pm/builds/elixir/v$ELIXIR_VERSION-otp-$ELIXIR_OTP_MAJOR.zip"
  verify "$tmp/elixir.zip" "$ELIXIR_SHA256"
  rm -rf "$ELIXIR_DIR"; mkdir -p "$ELIXIR_DIR"
  unzip -q "$tmp/elixir.zip" -d "$ELIXIR_DIR"
fi
PATH="$OTP_DIR/bin:$PATH" "$ELIXIR_DIR/bin/elixir" --version | tail -1 >&2

echo "$OTP_DIR/bin:$ELIXIR_DIR/bin"
