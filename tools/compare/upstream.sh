#!/bin/bash
# Ours vs upstream, same corpus, same oracle — the evidence behind the
# "beats the grammar it replaces" claim (issue #151). Fetches the current
# npm release of each upstream grammar and runs `treebank sweep` against
# its src/ (the loader needs only src/parser.c + grammar.json).
#
# Two caveats that must travel with any number this prints:
#
#   1. `noise` means the reference parser rejects the file too — so raw
#      "passed" understates agreement, and gap_files (valid code wrongly
#      rejected) is the only honest headline. On python, upstream "passes"
#      more files than we do while holding 6.7x the real gaps.
#   2. We ground our grammars against exactly this corpus and upstream
#      never saw it: this measures fit to our sample, not general
#      robustness. It cuts both ways — upstream's `&raw` regression broke
#      every identifier named `raw`, and only differential testing sees it.
set -euo pipefail
cd "$(dirname "$0")/../.."
WORK="${TMPDIR:-/tmp}/treebank-upstream"
mkdir -p "$WORK" && cd "$WORK"
for p in tree-sitter-python tree-sitter-rust tree-sitter-typescript \
         tree-sitter-java tree-sitter-bash; do
  [ -d "$p" ] && continue
  tgz=$(npm pack "$p" --silent | tail -1)
  mkdir -p "$p" && tar xzf "$tgz" -C "$p" --strip-components=1
done
cd - >/dev/null

run() { # lang ours theirs
  for who in ours theirs; do
    [ "$who" = ours ] && g=$2 || g=$3
    echo "### $1 $who"
    # A fresh cache per grammar, or the pass-set of one poisons the other.
    rm -f "corpus/$1/sweep-cache.json"
    cargo run -q --release -p treebank-cli -- sweep --lang "$1" --grammar "$g" \
      --out "$WORK/cmp-$1-$who.json" 2>/dev/null | grep -E "^sweep: [0-9]+ files —"
  done
  rm -f "corpus/$1/sweep-cache.json"
}
run typescript crates/treebank-typescript "$WORK/tree-sitter-typescript/tsx"
run javascript crates/treebank-typescript "$WORK/tree-sitter-typescript/tsx"
run rust       crates/treebank-rust       "$WORK/tree-sitter-rust"
run bash       crates/treebank-bash       "$WORK/tree-sitter-bash"
run java       crates/treebank-java       "$WORK/tree-sitter-java"
run python     crates/treebank-python     "$WORK/tree-sitter-python"
