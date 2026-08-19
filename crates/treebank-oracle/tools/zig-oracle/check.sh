#!/usr/bin/env bash
# Syntax-only Zig validity check for the treebank oracle.
#
# argv:   $1 is the zig binary to judge with (`zig`, `zig-0.11`, …), so one
#         script serves every version family the union oracle needs.
# stdin:  one file path per line
# stdout: "<path>\tvalid|invalid" per line
#
# `zig fmt --stdin` is the compiler's own tokenizer and parser: it builds a
# `std.zig.Ast`, renders it back out, and fails exactly when the source does
# not parse. It follows no `@import`, resolves no declaration and needs no
# build.zig, so a file is judged on its own text. Reading the source on
# stdin rather than naming the path also keeps it from touching the file.
#
# It is used INSTEAD OF `zig ast-check`, and the reason is worth keeping
# here as well as in the Rust module. An `invalid` verdict books a file our
# grammar failed as corpus noise. `ast-check` runs AstGen on top of the
# parser and rejects files that parse fine — an unused local, a discard of
# something already void — so every one of those would silently excuse a
# real grammar gap. The stricter tool is the one that flatters us.
#
# The formatted output is discarded: only the exit status is the verdict.
set -u
zig_bin=${1:?usage: check.sh <zig-binary>}
while IFS= read -r path; do
  [ -n "$path" ] || continue
  if "$zig_bin" fmt --stdin >/dev/null 2>&1 <"$path"; then
    printf '%s\tvalid\n' "$path"
  else
    printf '%s\tinvalid\n' "$path"
  fi
done
