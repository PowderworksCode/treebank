#!/usr/bin/env bash
# Regenerate the grammar lists in README.md and PUBLISHING.md from the ledgers.
#
# Why this exists. Adding a language updates GRAMMARS.md's contract, the
# ledger and the consumer test, and CI enforces all three. Nothing enforced
# the two prose lists, so they drifted: they said six grammars when there
# were ten, that was fixed by hand in 51b08d1 — and zig fell off the list
# again within a day of the fix. A list that several sessions must remember
# to edit by hand is a list that is wrong, which is the same lesson
# verify-grammars.yml already learned when it stopped hard-coding its matrix.
#
# So both blocks are generated between markers, and CI runs --check.
# Everything comes from files that CANNOT be forgotten, because verify.sh
# already fails without them:
#
#   grammar + version   ledger.json's upstream.git_url basename and .version
#   patch count         patches/*.patch minus the ones the ledger marks
#                       "kind": "packaging" (the redistribution notice and
#                       the crate identity patch are not parser fixes)
#   multi-grammar note  generate_dirs, when there is more than one
#   library name        the +[lib] name line in the crate identity patch,
#                       which is the only place it is decided
#
# Order is alphabetical: it is the one ordering that cannot drift and needs
# no git history to reproduce, and it matches what daily.sh already iterates
# (a shell glob over crates/treebank-*/ledger.json).
#
# A grammar can carry an optional `docs_note` in its ledger for something the
# ledger cannot otherwise know — javascript's "JSX included". That is the
# only editorial hook, and it lives with the grammar rather than in prose
# somebody has to find.
#
# Usage:
#   scripts/grammar-docs.sh            # rewrite the blocks in place
#   scripts/grammar-docs.sh --check    # fail if they are out of date (CI)
set -euo pipefail
cd "$(dirname "$0")/.." || exit 1

CHECK=0
[ "${1:-}" = "--check" ] && CHECK=1

readme_block() {
  for ledger in crates/treebank-*/ledger.json; do
    dir=$(dirname "$ledger")
    upstream=$(jq -r '.upstream.git_url | split("/") | last' "$ledger")
    version=$(jq -r '.upstream.version // "?"' "$ledger")
    packaging=$(jq '[.patches[] | select(.kind == "packaging")] | length' "$ledger")
    total=$(find "$dir/patches" -maxdepth 1 -name '*.patch' | wc -l)
    n=$((total - packaging))
    case $n in
      0) patches="no grammar patches" ;;
      1) patches="1 grammar patch" ;;
      *) patches="$n grammar patches" ;;
    esac
    # Parenthetical: the ledger's own note if it has one, else the extra
    # grammars generate_dirs names.
    note=$(jq -r '.docs_note // empty' "$ledger")
    if [ -z "$note" ]; then
      dirs=$(jq -r '(.generate_dirs // ["."]) | select(length > 1) | join(" + ")' "$ledger")
      [ -n "$dirs" ] && note="$dirs grammars"
    fi
    [ -n "$note" ] && note=" ($note)"
    echo "- \`$dir\` — $upstream $version$note, $patches"
  done
}

publishing_block() {
  echo '| directory | crate | library |'
  echo '|---|---|---|'
  for ledger in crates/treebank-*/ledger.json; do
    dir=$(dirname "$ledger"); name=${dir#crates/treebank-}
    identity=$(find "$dir/patches" -maxdepth 1 -name '*crate-identity*.patch' | head -1)
    if [ -z "$identity" ]; then
      echo "grammar-docs: $dir has no crate identity patch" >&2; exit 1
    fi
    # `[lib] name` is decided in exactly one place, and it is not derivable
    # from the directory: csharp publishes tree_sitter_c_sharp.
    lib=$(grep -m1 -E '^\+name = "tree_sitter' "$identity" | sed 's/.*"\(.*\)".*/\1/')
    if [ -z "$lib" ]; then
      echo "grammar-docs: no '+[lib] name' line in $identity" >&2; exit 1
    fi
    echo "| \`$dir\` | \`treebank-grammar-$name\` | \`$lib\` |"
  done
}

# Replace everything between the markers in $1 with the body in $2.
render() {
  local file=$1 body=$2 tmp
  tmp=$(mktemp)
  local begin="<!-- BEGIN GENERATED: scripts/grammar-docs.sh -->"
  local end="<!-- END GENERATED -->"
  if ! grep -qF "$begin" "$file" || ! grep -qF "$end" "$file"; then
    echo "grammar-docs: $file has no generated block markers" >&2; exit 1
  fi
  awk -v begin="$begin" -v end="$end" -v body="$body" '
    $0 == begin { print; print body; skip = 1; next }
    $0 == end   { skip = 0 }
    !skip       { print }
  ' "$file" > "$tmp"
  local rc=0
  if [ "$CHECK" = 1 ]; then
    if diff -u "$file" "$tmp" > /dev/null; then
      echo "grammar-docs: $file up to date"
    else
      echo "grammar-docs: $file is out of date — run scripts/grammar-docs.sh" >&2
      diff -u "$file" "$tmp" | sed -n '1,60p' >&2
      rc=1
    fi
  elif diff -q "$file" "$tmp" > /dev/null; then
    echo "grammar-docs: $file unchanged"
  else
    cat "$tmp" > "$file"
    echo "grammar-docs: $file regenerated"
  fi
  rm -f "$tmp"
  return $rc
}

status=0
render README.md "$(readme_block)" || status=1
render PUBLISHING.md "$(publishing_block)" || status=1
exit $status
