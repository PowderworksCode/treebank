#!/usr/bin/env bash
# Gate for wasm packs. Builds each pack and checks the things that are only
# checkable from OUTSIDE the build: that a real runtime loads it, that what
# it says about itself matches the repo, and that both bindings agree.
#
# Publishing is what this cannot rehearse, so everything below is what can
# be checked without it.
set -euo pipefail

ROOT=$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)
cd "$ROOT"
OUT=${TREEBANK_WASM_OUT:-dist/wasm}
GRAMMARS=${*:-python rust typescript}
PY=${TREEBANK_WASM_PYTHON:-python3}

fail() { echo "wasm-check: FAIL — $*" >&2; exit 1; }

for g in $GRAMMARS; do
  ./tools/wasm-pack/build.sh "$g" --out "$OUT" >/dev/null

  # 1. A pack must be byte-reproducible: build it twice and compare. This is
  #    the property that makes provenance worth anything.
  before=$(sha256sum "$OUT/treebank-$g.wasm" | cut -d' ' -f1)
  ./tools/wasm-pack/build.sh "$g" --out "$OUT" >/dev/null
  after=$(sha256sum "$OUT/treebank-$g.wasm" | cut -d' ' -f1)
  [ "$before" = "$after" ] || fail "$g: not byte-reproducible ($before vs $after)"

  # 2. It must load in a real WASI runtime and agree with the repo about
  #    what it is. A pack built from the wrong grammar would otherwise look
  #    fine: the name is the one fact that catches it, and grammar.json is
  #    its authority, not the directory.
  "$PY" - "$g" "$OUT/treebank-$g.wasm" <<'PY'
import json, tomllib, sys
from wasmtime import Engine, Linker, Module, Store, WasiConfig

lang, path = sys.argv[1:3]
eng = Engine(); store = Store(eng); store.set_wasi(WasiConfig())
lk = Linker(eng); lk.define_wasi()
e = lk.instantiate(store, Module.from_file(eng, path)).exports(store)
mem = e["memory"]; e["_initialize"](store)
blob = lambda p, n: json.loads(mem.read(store, p, p + n))

want_name = json.load(open(f"crates/treebank-{lang}/src/grammar.json"))["name"]
got_name = mem.read(store, e["tb_language_name"](store),
                    e["tb_language_name"](store) + e["tb_strlen"](store, e["tb_language_name"](store))).decode()
assert got_name == want_name, f"{lang}: module says {got_name!r}, grammar.json says {want_name!r}"

prov = blob(e["tb_provenance"](store), e["tb_provenance_len"](store))
ledger = tomllib.load(open(f"crates/treebank-{lang}/ledger.toml", "rb"))
for field in ("language", "vocabulary", "generate_cli"):
    assert prov[field] == ledger[field], f"{lang}: provenance {field} {prov[field]!r} != ledger {ledger[field]!r}"

# The facet manifest must be the repo's, exactly — a pack whose roles drift
# from roles.json would expand (_callable) to the wrong thing.
roles = blob(e["tb_roles"](store), e["tb_roles_len"](store))
assert roles == json.load(open(f"crates/treebank-{lang}/roles.json")), f"{lang}: roles.json does not match the pack"

# And it must actually parse.
src = b"" if lang != "python" else b"def f():\n    pass\n"
if lang == "rust":       src = b"fn f() {}\n"
if lang == "typescript": src = b"const f = <T>(x: T) => x;\n"
p = e["tb_alloc"](store, len(src)); mem.write(store, src, p)
tree = e["tb_parse"](store, p, len(src)); e["tb_free"](store, p)
node = e["tb_node_new"](store); e["tb_tree_root"](store, tree, node)
assert not (e["tb_node_flags"](store, node) & 4), f"{lang}: pack failed to parse its own smoke source"
print(f"  {lang}: loads, names itself {got_name}, provenance and roles match, parses")
PY
done
echo "wasm-check: OK"
