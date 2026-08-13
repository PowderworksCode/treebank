#!/usr/bin/env bash
# Adversarial battery for the Haskell oracle.
#
# The lesson this exists for is treebank-php's: an oracle that agrees with a
# reference on every file of clean library code can still be silently wrong,
# because clean code exercises none of its failure modes. Only files that
# SHOULD be rejected test an oracle. So this battery is weighted at the two
# ways this particular oracle can lie:
#
#   1. extension-gated syntax. GHC's parser reports "Illegal \case (use
#      LambdaCase)" as a RECOVERABLE error and still returns POk, so an
#      oracle reading the result constructor alone calls it valid. Measured
#      before the fix: 9 of 12 such constructs were called valid that GHC
#      itself rejects. Each is here as a pair — without the flag it must be
#      invalid, with the flag it must be valid — because only the pair
#      proves the flag is what decided it.
#   2. the parse-only boundary. Three cases (RecordWildCards, DerivingVia,
#      TransformListComp) are rejected by `ghc` but accepted by GHC's
#      PARSER, because their extension is checked in the renamer. A
#      parse-only oracle must call them valid; asserting that here is what
#      stops someone "fixing" the oracle into a typechecker.
#
# expected.tsv is <file>\t<verdict>\t<flags, or - for none>\t<what the ghc
# DRIVER does>. The `-` is not decoration: bash's `read` treats tab as IFS
# whitespace, so an empty column between two tabs silently disappears and
# every later field shifts left by one.
# This script checks column 2, the oracle's verdict. vs-ghc.sh checks column
# 4, which is the measured behaviour of `ghc` itself on the same file — a
# coarser question (it compiles rather than parses), and the ten rows where
# the two columns disagree are the parse-only boundary written down.
set -uo pipefail
cd "$(dirname "$0")"
ORACLE=../hs-oracle
[ -x "$ORACLE" ] || { echo "battery: no oracle built — run tools/hs-oracle/build.sh" >&2; exit 2; }

fail=0
while IFS=$'\t' read -r file want flags _ghc; do
  [ -n "$file" ] || continue
  [ "$flags" = "-" ] && flags=""
  req="$PWD/$file"
  [ -n "$flags" ] && req="$req	${flags// /	}"
  got=$(printf '%s\n' "$req" | "$ORACLE" | cut -f2)
  if [ "$got" != "$want" ]; then
    echo "battery: FAIL $file [${flags:-no flags}] — want $want, got ${got:-<none>}"
    fail=1
  fi
done < expected.tsv

if [ "$fail" = 0 ]; then
  echo "battery: ok ($(grep -c . expected.tsv) cases)"
else
  echo "battery: FAILED" >&2
fi
exit "$fail"
