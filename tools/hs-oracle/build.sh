#!/usr/bin/env bash
# Build the Haskell validity oracle against the GHC on PATH.
#
# The `ghc` LIBRARY (the compiler's own parser, shipped inside every GHC
# installation) is the reference parser; there is no package to install and
# no network access needed, but the GHC version is load-bearing exactly the
# way py-oracle's CPython version is: the toolchain's parser is the language
# definition, and Haskell grows syntax through LANGUAGE extensions, so
# syntax newer than this GHC is not valid Haskell *here*. ledger.json
# records the version the sweep numbers were produced with; keep the two in
# step, and check.hs refuses to run if the two disagree at run time.
#
# -O1, not -O2: measured 9 s to build at -O1 against 41 s at -O2, for a
# sweep-time difference inside the run-to-run noise (the work is inside the
# ghc library, which is already compiled).
set -euo pipefail
cd "$(dirname "$0")"

if ! command -v ghc >/dev/null 2>&1; then
  echo "hs-oracle: no ghc on PATH (https://www.haskell.org/ghcup/)" >&2
  exit 1
fi

# The building compiler's libdir is baked in with -cpp: the oracle is run by
# a sweep from an arbitrary working directory and must not depend on `ghc`
# being on PATH at that moment. TREEBANK_GHC_LIBDIR still overrides it.
LIBDIR=$(ghc --print-libdir)
GHCFLAGS=(-package ghc -package process -package directory -O1
          -cpp "-DTREEBANK_GHC_LIBDIR=\"$LIBDIR\"")
ghc "${GHCFLAGS[@]}" -o hs-oracle check.hs
ghc "${GHCFLAGS[@]}" -o explain explain.hs
ghc --version
echo "hs-oracle: built"
