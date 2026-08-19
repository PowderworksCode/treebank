#!/usr/bin/env bash
# Build a treebank wasm pack: one grammar, self-contained, byte-reproducible.
#
#   crates/treebank-<lang>/src/     (generated, committed, checked by CI)
#   + the tree-sitter runtime       (pinned by sha256, cached)
#   + tools/wasm-pack/shim.c        (the pack ABI)
#   + provenance + roles generated from ledger.toml and roles.json
#   -> dist/wasm/treebank-<lang>.wasm
#
# The pack is a STANDALONE wasm module: it imports only WASI, so any runtime
# loads it with no emscripten glue and no native tree-sitter. Contrast with
# `tree-sitter build --wasm`, which emits an emscripten SIDE MODULE carrying
# the grammar tables only and expecting the runtime to already be in its
# linear memory — right for web-tree-sitter, unusable from anything else
# without implementing emscripten's dynamic-linking protocol.
#
# Two things travel INSIDE the module, not beside it:
#
#   tb_provenance()  what this pack is and how it was measured
#   tb_roles()       the facet manifest (_callable/_binding/_scope/_clause)
#
# The second is not a nicety. Table-tier roles are real supertypes and are
# queryable from the parser itself; facets are not in the parse table, so a
# consumer that cannot read roles.json cannot expand `(_callable)` at all.
# Shipping it inside means a pack answers that from its own bytes — the file
# next to the binary is the thing that goes missing.
set -euo pipefail

ROOT=$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)
# shellcheck source=tools/wasm-pack/toolchain.sh
. "$ROOT/tools/wasm-pack/toolchain.sh"

LANG=${1:?usage: build.sh <language> [--out DIR]}
OUT="$ROOT/dist/wasm"
shift
while [ $# -gt 0 ]; do
  case $1 in
    --out) OUT=${2:?--out needs a value}; shift 2 ;;
    *) echo "build.sh: unknown argument $1" >&2; exit 2 ;;
  esac
done

CRATE="$ROOT/crates/treebank-$LANG"
[ -d "$CRATE" ] || { echo "build.sh: no such grammar: $LANG" >&2; exit 2; }

# A multi-variant language keeps its generated parser under a variant
# directory (VARIANTS.md §2). The pack is built for the DEFAULT variant --
# the first one tree-sitter.json declares -- because that is what the
# language's name means to a consumer: `treebank-python.wasm` is python 3,
# the same way treebank_python::LANGUAGE is. A pack per variant is a
# separate decision and a separate artifact name; nothing needs it yet.
VARIANT=$(python3 - "$CRATE" <<'PY'
import json, sys, pathlib
crate = pathlib.Path(sys.argv[1])
try:
    grammars = json.load(open(crate / "tree-sitter.json"))["grammars"]
    print(crate / grammars[0].get("path", "."))
except Exception:
    print(crate)
PY
)
[ -d "$VARIANT/src" ] || { echo "build.sh: no generated grammar at $VARIANT" >&2; exit 2; }

SDK=$(wasi_sdk_ensure)
BINARYEN=$(binaryen_ensure)
RUNTIME=$(runtime_ensure)

CLANG="$SDK/bin/clang"
WASM_OPT="$BINARYEN/bin/wasm-opt"

# The grammar's exported entry point comes from grammar.json, not the
# directory: a grammar may declare a name its directory does not spell.
GRAMMAR_NAME=$(python3 -c "import json,sys; print(json.load(open(sys.argv[1]))['name'])" "$VARIANT/src/grammar.json")

WORK=$(mktemp -d)
trap 'rm -rf "$WORK"' EXIT

# ---- provenance and roles, generated into C -----------------------------
# Deliberately carries no timestamp, build host or git sha: anything ambient
# would break the byte-reproducibility the provenance exists to make
# checkable.
python3 - "$CRATE" "$VARIANT" "$GRAMMAR_NAME" "$RUNTIME_VERSION" "$WORK/embedded.c" <<'PY'
import hashlib, json, os, sys, tomllib

crate, variant, grammar_name, runtime_version, out = sys.argv[1:6]
ledger = tomllib.load(open(f"{crate}/ledger.toml", "rb"))
roles_text = open(f"{crate}/roles.json").read()

def sha256_file(p):
    return hashlib.sha256(open(p, "rb").read()).hexdigest()

sources = {"grammar.js": sha256_file(f"{variant}/grammar.js")}
try:
    # The variant's scanner.c is a stub that includes the shared one, so
    # hash what it includes as well: otherwise the provenance is blind to
    # every change in common/scanner.c.
    sources["scanner.c"] = sha256_file(f"{variant}/src/scanner.c")
    shared = f"{crate}/common/scanner.c"
    if os.path.exists(shared):
        sources["common/scanner.c"] = sha256_file(shared)
except FileNotFoundError:
    pass
sources["parser.c"] = sha256_file(f"{variant}/src/parser.c")

corpus = ledger.get("corpus", {})
sweeps = {k: v for k, v in corpus.items() if k.endswith("sweep")}

prov = {
    "pack_abi": 1,
    "producer": "treebank",
    "language": ledger["language"],
    "grammar_name": grammar_name,
    "versions": ledger.get("versions"),
    "vocabulary": ledger.get("vocabulary"),
    "generate_cli": ledger.get("generate_cli"),
    "runtime": runtime_version,
    # The grammar is ours, so provenance is the SOURCE HASH rather than an
    # upstream sha and a patch series: there is no upstream to point at.
    "sources": sources,
    "oracles": [
        {k: o.get(k) for k in ("family", "tool", "version") if k in o}
        for o in ledger.get("oracles", [])
    ],
    "sweeps": sweeps,
    "known_gaps": ledger.get("known_gaps", []),
    # A pack cannot re-derive these: validate() drives a real toolchain
    # (CPython, syn, tsc, V8) and a wasm module cannot. They are evidence
    # recorded at build time, not something the pack can check.
    "evidence_note": (
        "sweep and gap numbers were measured when this pack was built; a pack "
        "carries the parser, not the oracle, and cannot re-derive them"
    ),
}

def c_bytes(name, text):
    data = text.encode()
    body = ",".join(str(b) for b in data)
    return (
        f"const unsigned char {name}[] = {{{body},0}};\n"
        f"const unsigned {name.replace('_raw','')}_len = {len(data)}u;\n"
    )

with open(out, "w") as f:
    f.write("/* generated by tools/wasm-pack/build.sh - do not edit */\n")
    f.write(c_bytes("treebank_provenance_raw", json.dumps(prov, separators=(",", ":"), sort_keys=True)))
    f.write(c_bytes("treebank_roles_raw", json.dumps(json.loads(roles_text), separators=(",", ":"), sort_keys=True)))
PY

# ---- compile ------------------------------------------------------------
# -O3 not -Oz: -Oz buys about 6% size for 28% throughput, the wrong trade for
# something whose job is parsing corpora.
#
# NO -flto. It silently exports _start instead of _initialize, which loses
# the WASI reactor exec model, and every host then refuses to instantiate
# the module. That failure looks like a runtime bug and is a link flag.
"$CLANG" \
  --target=wasm32-wasip1 \
  -O3 \
  -fno-exceptions \
  -mexec-model=reactor \
  -I "$RUNTIME/lib/include" \
  -I "$RUNTIME/lib/src" \
  -I "$VARIANT/src" \
  -DTREEBANK_LANGUAGE_FN="tree_sitter_$GRAMMAR_NAME" \
  -Wl,--no-entry \
  -Wl,--export-dynamic \
  -o "$WORK/pack.wasm" \
  "$RUNTIME/lib/src/lib.c" \
  "$VARIANT/src/parser.c" \
  $([ -f "$VARIANT/src/scanner.c" ] && echo "$VARIANT/src/scanner.c") \
  "$ROOT/tools/wasm-pack/shim.c" \
  "$WORK/embedded.c"

# binaryen is not optional, and that only shows up if you check every
# grammar rather than one. lld emits a single data segment spanning the
# whole static image; wasm-opt's memory packing splits it and drops the long
# zero runs parse tables are full of. The effect scales with table size, so
# generalising from the smallest grammar would ship the largest one far
# bigger than necessary.
mkdir -p "$OUT"
"$WASM_OPT" -O3 --enable-bulk-memory --enable-nontrapping-float-to-int \
  "$WORK/pack.wasm" -o "$OUT/treebank-$LANG.wasm"

printf 'pack: %s  %s bytes (pre-opt %s)\n' \
  "$OUT/treebank-$LANG.wasm" \
  "$(stat -c%s "$OUT/treebank-$LANG.wasm")" \
  "$(stat -c%s "$WORK/pack.wasm")"
