#!/usr/bin/env bash
# Syntax-only Zig validity check for the treebank oracle.
#
# stdin:  one file path per line
# stdout: "<path>\tvalid|invalid" per line
#
# `zig fmt --stdin` is the compiler's own tokenizer and parser: it builds a
# `std.zig.Ast`, renders it back out, and fails exactly when the source does
# not parse. It follows no `@import`, resolves no declaration and needs no
# build.zig, so a file is judged on its own text.
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
while IFS= read -r path; do
  [ -n "$path" ] || continue
  if zig fmt --stdin >/dev/null 2>&1 <"$path"; then
    printf '%s\tvalid\n' "$path"
  else
    printf '%s\tinvalid\n' "$path"
  fi
done
