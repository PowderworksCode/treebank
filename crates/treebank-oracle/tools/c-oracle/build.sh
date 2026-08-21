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

# The version is PINNED by whatever this resolves to, and the pin is
# recorded in each ledger. Searching newest-first rather than hard-coding
# one is a convenience for a fresh machine, not a licence to let it float:
# a build that picks a different libclang than the ledger names is a build
# whose numbers are not the ledger's numbers, which is why the version is
# echoed at the end and expected to be read.
LLVM_DIR="${TREEBANK_LLVM_DIR:-}"
if [ -z "$LLVM_DIR" ]; then
  for d in $(ls -d /usr/lib/llvm-* 2>/dev/null | sort -V -r); do
    if [ -f "$d/include/clang-c/Index.h" ]; then LLVM_DIR="$d"; break; fi
  done
fi
if [ -z "$LLVM_DIR" ] || [ ! -f "$LLVM_DIR/include/clang-c/Index.h" ]; then
  echo "c-oracle: no libclang headers found under /usr/lib/llvm-*" >&2
  echo "          (apt install libclang-dev, or set TREEBANK_LLVM_DIR)" >&2
  exit 1
fi

cc -O2 -Wall -Wextra -o c-oracle oracle.c \
  -I"$LLVM_DIR/include" -L"$LLVM_DIR/lib" -lclang -Wl,-rpath,"$LLVM_DIR/lib"
"$LLVM_DIR/bin/clang" --version 2>/dev/null | head -1 || true
echo "c-oracle: built against $LLVM_DIR"
