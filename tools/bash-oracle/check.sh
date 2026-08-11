#!/usr/bin/env bash
# Syntax-only Bash validity check for the treebank oracle.
#
# stdin:  one file path per line
# stdout: "<path>\tvalid|invalid" per line
#
# The reference parser is bash's own, driven through `bash -n`: read the
# whole script, build the command tree, execute nothing. That last part is
# the entire safety argument for pointing a shell at thousands of strangers'
# scripts, so it was verified rather than trusted, on this machine, before a
# single corpus file was swept:
#
#   * `source /absent/file` and `. /absent/file` exit 0 — a missing include
#     is not an error, so each file is judged on its own text (the same
#     property that makes ast.parse usable for Python and luac -p for Lua).
#   * `rm -rf "$HOME/CANARY"` against a real directory left it untouched.
#   * $(...), `...`, ${x:=$(...)}, <(...), >(...), a heredoc body, an eval
#     argument and a `.`-sourced file at top level all ran nothing.
#   * BASH_ENV and ENV pointing at a script that touches a file ran nothing.
#
# Contrast `perl -c`, which runs BEGIN blocks, and the ROADMAP's Tier C.
#
# Exit codes are the verdict, and there are more than two of them:
#   0   parsed             -> valid
#   2   syntax error       -> invalid
#   126 "cannot execute binary file" — bash refuses a file containing a NUL
#       byte, and refuses a directory                      -> invalid
#   127 file does not exist                                -> a harness bug
# 126 is a real verdict (bash will not run that file, so it is not a valid
# script) but 127 is not, so the readable-regular-file check below comes
# first and complains on stderr rather than quietly recording "invalid".
#
# WHAT THIS DOES NOT CHECK. bash's parser defers everything that is not
# syntax: `$(( 1 + + ))` and `${!}` are accepted here and fail at runtime,
# and an unterminated heredoc is a warning, not an error. That is correct
# for a parse-only oracle — the grammar treats those regions as text too —
# but it means this oracle can never be used as evidence for TIGHTENING the
# grammar in them.
#
# DIALECT. This is bash, so it is the reference parser for exactly the files
# lang/bash.rs admits: shebangs matching upstream's own first-line-regex
# (sh|bash|dash) plus the extensions tree-sitter.json claims. It is NOT the
# reference parser for zsh, fish, csh or ksh, and calling it one would turn
# the grammar's correct rejection of those dialects into reported gaps — the
# same trap as pointing the JavaScript oracle at the TypeScript parser.
#
# WHY IT FORKS. bash cannot syntax-check a file from inside a long-lived
# shell: `set -n` stops that shell from executing the very `source` that
# would read the next file. So it is one process per file, like `php -l`,
# and measured on this machine that fork is 83% of the cost (2.0 ms of
# 2.4 ms; parsing 6 KB of shell is 0.4 ms). The answer is the one the
# ROADMAP prescribes for php: run the forks in parallel. TREEBANK_ORACLE_JOBS
# overrides the width; the default is nproc.
set -u

if [ "${1-}" = "--batch" ]; then
    shift
    for f in "$@"; do
        if [ ! -f "$f" ] || [ ! -r "$f" ]; then
            printf 'bash-oracle: %s is not a readable regular file\n' "$f" >&2
            printf '%s\tinvalid\n' "$f"
            continue
        fi
        # ulimit is a CPU-second cap on a pathological parse. It costs no
        # extra process: the subshell is the fork that `bash -n` needed
        # anyway, and exec turns it into the parse.
        if (ulimit -t 10; exec bash -n -- "$f") 2>/dev/null; then
            printf '%s\tvalid\n' "$f"
        else
            printf '%s\tinvalid\n' "$f"
        fi
    done
    exit 0
fi

self=$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)/$(basename "${BASH_SOURCE[0]}")
jobs=${TREEBANK_ORACLE_JOBS:-$(nproc 2>/dev/null || echo 4)}
# -d '\n' so that spaces and quotes in a corpus path are not word-split;
# a path containing a newline cannot be expressed in this protocol at all
# and does not survive the sweep's manifest either.
# -n 64 amortizes xargs's own spawn over 64 parses. Each verdict is one
# short line written by one process, which is atomic on a pipe below
# PIPE_BUF; verified at 10k paths and -P16 (see ORACLE.md).
exec xargs -d '\n' -r -P "$jobs" -n 64 "$self" --batch
