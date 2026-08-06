#!/usr/bin/env bash
# Parse file(s) with the grammar in the current directory using its pinned
# CLI. Multi-grammar repos route by extension via tree-sitter.json.
# Usage (from a grammar repo root): ../../scripts/parse.sh <file> [...]
set -euo pipefail
[ -f ledger.json ] || { echo "agent-parse: run from a grammar repo root (no ledger.json here)"; exit 2; }
CLI=$(jq -r .generate_cli ledger.json)
npx -y "tree-sitter-cli@$CLI" parse "$@"
