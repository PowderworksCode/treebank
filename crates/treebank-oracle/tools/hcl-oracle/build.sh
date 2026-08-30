#!/usr/bin/env bash
# Build the HCL validity oracle against a PINNED hashicorp/hcl.
#
# The version is load-bearing the way ledger.json's generate_cli is:
# hclsyntax IS the reference parser, so a different release moves the sweep
# numbers. `go.mod` and `go.sum` pin it exactly and are committed beside
# this script; `-mod=readonly` is what makes the pin a pin rather than a
# preference, because a plain `go build` will happily rewrite `go.mod` to
# whatever it resolves.
#
# The dependency is MPL-2.0 and is fetched at build time rather than
# vendored, the same arrangement as the C oracle's libclang and the
# TypeScript oracle's `npm ci`: a tool the gate calls, never code this
# repository ships.
set -euo pipefail
cd "$(dirname "$0")"

if ! command -v go >/dev/null; then
  echo "hcl-oracle: go is not on PATH (needed to build the HCL validity oracle)" >&2
  exit 1
fi

GOFLAGS=-mod=readonly go build -o hcl-oracle .
echo "hcl-oracle: built against hashicorp/hcl $(awk '/hashicorp\/hcl\/v2 v/ {print $NF; exit}' go.mod)"
