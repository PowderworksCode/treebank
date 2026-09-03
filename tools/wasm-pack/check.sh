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
PY=${TREEBANK_WASM_PYTHON:-python3}
GRAMMARS=("$@")
if [ ${#GRAMMARS[@]} -eq 0 ]; then
  while IFS= read -r grammar; do
    GRAMMARS+=("$grammar")
  done < <(./tools/wasm-pack/list-grammars.sh)
fi

fail() { echo "wasm-check: FAIL — $*" >&2; exit 1; }

WORK=$(mktemp -d)
trap 'rm -rf "$WORK"' EXIT

for g in "${GRAMMARS[@]}"; do
  fixtures=("test/sweep-smoke/$g/src/sweep-smoke/"[Vv]alid.*)
  if [ ${#fixtures[@]} -ne 1 ] || [ ! -f "${fixtures[0]}" ]; then
    fail "$g: expected exactly one test/sweep-smoke valid fixture"
  fi

  ./tools/wasm-pack/build.sh "$g" --out "$OUT" >/dev/null

  # 1. A pack must be byte-reproducible: build it twice and compare. This is
  #    the property that makes provenance worth anything.
  before=$(sha256sum "$OUT/treebank-$g.wasm" | cut -d' ' -f1)
  ./tools/wasm-pack/build.sh "$g" --out "$OUT" >/dev/null
  after=$(sha256sum "$OUT/treebank-$g.wasm" | cut -d' ' -f1)
  [ "$before" = "$after" ] || fail "$g: not byte-reproducible ($before vs $after)"

  #    Rebuilding in place cannot see the failure that actually happened: the
  #    runtime's assertions bake __FILE__ into the module, so a pack built
  #    under /home/runner and one built under /home/exedev differed by 65
  #    bytes of cache path -- same size, same provenance, different hash, and
  #    two rebuilds on one machine agreeing every time.
  #
  #    So build once more through a differently named path to the same cache.
  #    A symlink is enough: clang records the path it was given, not the one
  #    it resolves to, which is exactly the ambient input being tested for.
  alt="$WORK/cache-under-another-name"
  ln -sfn "${TREEBANK_WASM_CACHE:-${XDG_CACHE_HOME:-$HOME/.cache}/treebank}" "$alt"
  TREEBANK_WASM_CACHE="$alt" ./tools/wasm-pack/build.sh "$g" --out "$WORK/alt" >/dev/null
  elsewhere=$(sha256sum "$WORK/alt/treebank-$g.wasm" | cut -d' ' -f1)
  [ "$before" = "$elsewhere" ] \
    || fail "$g: build depends on where the toolchain lives ($before vs $elsewhere)"

  # 2. It must load in a real WASI runtime and agree with the repo about
  #    what it is. A pack built from the wrong grammar would otherwise look
  #    fine: the name is the one fact that catches it, and grammar.json is
  #    its authority, not the directory.
  "$PY" - "$g" "$OUT/treebank-$g.wasm" "${fixtures[0]}" <<'PY'
import json, tomllib, sys
from wasmtime import Engine, Linker, Module, Store, WasiConfig

lang, path, fixture = sys.argv[1:4]
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

# The nominal manifest must be the repo's, exactly — a pack whose terms drift
# from terms.json would expand (_callable) to the wrong thing.
terms = blob(e["tb_terms"](store), e["tb_terms_len"](store))
assert terms == json.load(open(f"crates/treebank-{lang}/terms.json")), f"{lang}: terms.json does not match the pack"
# The pre-rename export names are kept for one cycle and must carry the same
# document, or a consumer pinned to the old ABI silently gets something else.
assert blob(e["tb_roles"](store), e["tb_roles_len"](store)) == terms, f"{lang}: tb_roles disagrees with tb_terms"

# The node manifest likewise, and this one fails quieter: structural
# membership drifting from node-types.json makes (_loop) match the wrong
# nodes, so a consumer sees a rule stop firing rather than an error.
node_types = blob(e["tb_node_types"](store), e["tb_node_types_len"](store))
assert node_types == json.load(open(f"crates/treebank-{lang}/src/node-types.json")), \
    f"{lang}: node-types.json does not match the pack"

# The ABI version lives in shim.c and again in the provenance the build
# generates. They are written in two places, so check they agree.
assert prov["pack_abi"] == e["tb_pack_abi"](store), \
    f"{lang}: provenance says pack_abi {prov['pack_abi']}, module says {e['tb_pack_abi'](store)}"

# And it must actually parse a checked-in program. These are the same valid
# fixtures the native sweep smoke sends through the production parser path.
src = open(fixture, "rb").read()
p = e["tb_alloc"](store, len(src)); mem.write(store, src, p)
tree = e["tb_parse"](store, p, len(src)); e["tb_free"](store, p)
node = e["tb_node_new"](store); e["tb_tree_root"](store, tree, node)
assert not (e["tb_node_flags"](store, node) & 4), f"{lang}: pack failed to parse {fixture}"
print(f"  {lang}: loads, names itself {got_name}, provenance, terms and node types match, parses {fixture}")
PY
done
echo "wasm-check: OK"
