#!/usr/bin/env bash
# Syntax-only bash validity check for the treebank oracle.
#
# stdin:  one file path per line
# stdout: "<path>\tvalid|invalid" per line
#
# `bash -n` is bash's own parser and nothing else: it reads the script,
# builds the command list, and stops before executing a single word. That
# makes it the right shape of oracle — a file is judged on its own text,
# with no side effects and no dependence on what is installed.
#
# Two things it does NOT catch, worth stating because they set the ceiling
# on what a bash sweep can mean. `bash -n` does not expand, so an error
# inside a command substitution is invisible until run time. And it accepts
# some constructs that fail at run time for non-syntactic reasons.
set -u
while IFS= read -r path; do
  [ -n "$path" ] || continue
  if bash -n -- "$path" 2>/dev/null; then
    printf '%s\tvalid\n' "$path"
  else
    printf '%s\tinvalid\n' "$path"
  fi
done
