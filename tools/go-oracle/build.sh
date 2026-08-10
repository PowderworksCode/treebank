#!/usr/bin/env bash
# Build the Go validity oracle.
#
# stdlib only, so this needs no module, no go.mod and no network — `go
# build` on a single file is the whole build. It is a build rather than a
# `go run` because `go run` re-links on every invocation (~0.3 s), and the
# sweep calls the oracle once per batch.
#
# The Go version is load-bearing the way py-oracle's CPython version is:
# the toolchain's parser is the language definition, so syntax newer than
# this toolchain is not valid Go *here*. ledger.json records the version
# the sweep numbers were produced with; keep the two in step.
set -euo pipefail
cd "$(dirname "$0")"

if ! command -v go >/dev/null 2>&1; then
  echo "go-oracle: no go toolchain on PATH (https://go.dev/dl/)" >&2
  exit 1
fi

go build -o go-oracle oracle.go
go version
echo "go-oracle: built"
