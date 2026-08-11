#!/usr/bin/env sh
# Build the oracle against one or more Zig toolchains.
#
#   ./build.sh                       # the pinned version named in ledger.json
#   ./build.sh /path/to/zig ...      # one binary per toolchain given
#
# Producing one binary per version is the whole point: a version bump is
# measured by running two of them over the same corpus and diffing the
# verdicts, not by swapping the toolchain and hoping.
set -e
cd "$(dirname "$0")"
for zig in "${@:-zig}"; do
    v=$("$zig" version)
    "$zig" build-exe check.zig   -O ReleaseFast --name "check-$v"
    "$zig" build-exe explain.zig -O ReleaseFast --name "explain-$v"
    echo "built check-$v, explain-$v"
done
