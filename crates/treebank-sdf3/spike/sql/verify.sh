#!/usr/bin/env bash
# Lower every target of the SQL family from its module, generate its parser,
# check its vocabulary roles, and hold all of them to the corpus -- with a
# real PostgreSQL 16 and MariaDB 10.11 as the oracles for those two targets
# when the environment names them (see tools/targets_check.py).
#
#   TREEBANK_PSQL="psql -h /tmp/pg -p 54329 -U postgres -d postgres" \
#   TREEBANK_MARIADB="mariadb -S /tmp/my/sock -u root" ./verify.sh
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

python3 "$HERE/../../tools/targets_check.py" "$HERE" "$@"
