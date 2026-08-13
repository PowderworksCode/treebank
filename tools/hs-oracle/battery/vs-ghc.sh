#!/usr/bin/env bash
# Pins expected.tsv's fourth column: what the `ghc` DRIVER does with each
# battery file, measured rather than predicted.
#
# It is deliberately NOT used to derive column 2. `ghc` compiles where this
# oracle parses, and the driver cannot be talked into answering the narrower
# question: -ddump-parsed prints its banner even for a file whose parse
# errors are fatal (GHC accumulates recoverable errors, dumps the tree it
# built, and only then fails), and no dump combination separates a parse
# error from a renamer one. So the two columns answer two questions, and
# what this script protects is the RELATIONSHIP between them:
#
#   col2=invalid, col4=rejects  — both agree the file is bad
#   col2=valid,   col4=accepts  — both agree it is fine
#   col2=valid,   col4=rejects  — the parse-only boundary: GHC's parser
#                                 accepts it and a later phase does not.
#                                 Ten rows, each one a documented case
#                                 rather than an unexplained divergence.
#
# A change on either side shows up here as a diff. Not run in CI: it costs a
# `ghc` process per case and only moves when the toolchain does.
set -uo pipefail
cd "$(dirname "$0")"
command -v ghc >/dev/null || { echo "vs-ghc: no ghc on PATH" >&2; exit 2; }
tmp=$(mktemp -d); trap 'rm -rf "$tmp"' EXIT
agree=0; disagree=0
while IFS=$'\t' read -r file want flags wantghc; do
  [ -n "$file" ] || continue
  [ "$flags" = "-" ] && flags=""
  out=$(cd "$tmp" && ghc -fno-code -v0 $flags "$OLDPWD/$file" 2>&1)
  if [ -z "$out" ]; then got=accepts
  elif grep -qE "Could not find module|could not execute" <<<"$out"; then got=unjudged
  else got="rejects:$(grep -oE 'GHC-[0-9]+' <<<"$out" | head -1)"; fi
  if [ "$got" = "$wantghc" ]; then agree=$((agree+1)); else
    disagree=$((disagree+1))
    echo "MOVED $file [${flags:-no flags}] — expected.tsv records ghc as $wantghc, got $got"
    sed -n '1,3p' <<<"$out" | sed 's/^/    /'
  fi
done < expected.tsv
echo "vs-ghc: $agree unchanged, $disagree moved"
echo "        (parse-only boundary: $(awk -F'\t' '$2=="valid" && $4!="accepts"' expected.tsv | wc -l) rows where the oracle accepts and ghc does not)"
exit $((disagree > 0))
