#!/usr/bin/env bash
# Build the C validity oracle against a PINNED libclang.
#
# The libclang version is load-bearing exactly the way ledger.json's
# generate_cli is: clang's error-recovery behaviour decides whether a file is
# called invalid or indeterminate, so a different libclang moves the sweep
# numbers. ledger.json records the version this was built against; keep the
# two in step.
set -euo pipefail
cd "$(dirname "$0")"

LLVM_DIR="${TREEBANK_LLVM_DIR:-/usr/lib/llvm-20}"
if [ ! -f "$LLVM_DIR/include/clang-c/Index.h" ]; then
  echo "c-oracle: no libclang headers at $LLVM_DIR (apt install libclang-20-dev," >&2
  echo "          or set TREEBANK_LLVM_DIR to an llvm install)" >&2
  exit 1
fi

cc -O2 -Wall -Wextra -o c-oracle oracle.c \
  -I"$LLVM_DIR/include" -L"$LLVM_DIR/lib" -lclang -Wl,-rpath,"$LLVM_DIR/lib"
"$LLVM_DIR/bin/clang" --version 2>/dev/null | head -1 || true
echo "c-oracle: built against $LLVM_DIR"
