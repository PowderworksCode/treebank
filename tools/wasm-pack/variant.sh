#!/usr/bin/env bash
# Where a language crate's DEFAULT variant lives.
#
# A single-variant language keeps grammar.js, src/ and test/ at its crate
# root; a multi-variant one (VARIANTS.md §2) keeps them under a variant
# directory and declares the order in tree-sitter.json, first entry first.
# The pack is built for the default variant because that is what the
# language's name means to a consumer: treebank-python.wasm is python 3,
# the same way treebank_python::LANGUAGE is.
#
# This exists as one function because it was three inline copies, and the
# copy that was never written is what broke the release rehearsal: the
# fixture path still pointed at the pre-split layout. A resolver that
# cannot silently return a directory with no grammar in it is the point —
# hence the check below.

# variant_dir <crate-dir> -> the default variant's directory
variant_dir() {
  local crate=${1:?variant_dir needs a crate directory}
  local dir
  dir=$(python3 - "$crate" <<'PY'
import json, pathlib, sys
crate = pathlib.Path(sys.argv[1])
try:
    grammars = json.load(open(crate / "tree-sitter.json"))["grammars"]
    print(crate / grammars[0].get("path", "."))
except Exception:
    print(crate)
PY
  )
  if [ ! -f "$dir/src/parser.c" ]; then
    echo "variant_dir: no generated grammar under $dir (from $crate)" >&2
    return 1
  fi
  printf '%s\n' "$dir"
}
