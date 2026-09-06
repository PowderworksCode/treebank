#!/usr/bin/env bash
# Lower each Rust edition to its own parser and hold all four to the corpus
# and to rustc --edition (tools/targets_check.py).
set -euo pipefail

HERE=$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)
ROOT=$(cd "$HERE/../../../.." && pwd)

cd "$ROOT"
for t in $(python3 -c "import json;print(' '.join(json.load(open('$HERE/targets.json'))['targets']))"); do
  out="$HERE/targets/${t//\//-}"
  cargo run -q -p treebank-sdf3 --example lower -- "$HERE/$t.sdf3" --generate --out "$out"
  cp "$out/grammar.json" "$out/src/grammar.json"
  cargo run -q -p treebank-sdf3 --example roles -- "$out"
done

python3 "$HERE/../../tools/targets_check.py" "$HERE" --require-oracles
