#!/usr/bin/env bash
# Print every grammar that owns a wasm pack, one canonical name per line.
# A grammar.js under crates/treebank-<name>/ is the repository's definition
# of a shipped grammar; consumers should not maintain a second list.
set -euo pipefail

ROOT=$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)
cd "$ROOT"

find crates -mindepth 2 -maxdepth 2 -type f -name grammar.js \
  | sed 's|^crates/treebank-||; s|/grammar.js$||' \
  | sort
