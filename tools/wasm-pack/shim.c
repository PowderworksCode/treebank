/* The treebank wasm pack ABI.
 *
 * A pack is ONE self-contained wasm module: the tree-sitter runtime, one
 * patched grammar, and this shim, statically linked by wasi-sdk's clang. It
 * imports nothing but WASI, so any wasm runtime can drive it — wasmtime,
 * wazero, wasmer, or a browser — with no JS glue and no native tree-sitter
 * anywhere.
 *
 * Contrast with what `tree-sitter build --wasm` emits, which is an emscripten
 * SIDE MODULE: it carries the grammar tables only and expects the tree-sitter
 * runtime to already be present in its linear memory. That format is the right
 * drop-in for web-tree-sitter and wrong for everything else, because a host
 * that isn't web-tree-sitter has to implement emscripten's dynamic-linking
 * protocol (__memory_base/__table_base allocation, data relocations) before it
 * can load one. Packs exist so a binding in any language is a few dozen lines.
 *
 * ABI rules, so bindings can be written against this and stay written:
 *
 *   - Every parameter and result is i32 or u32. Nothing is passed or returned
 *     by struct: TSNode is a by-value struct in C and wasm cannot return one,
 *     which is why node accessors take a POINTER to a node slot instead.
 *   - A "node slot" is sizeof(TSNode) bytes of module memory owned by the
 *     caller: tb_node_new() to get one, tb_node_free() when done. Accessors
 *     that produce a node write into a caller-supplied out slot, and the out
 *     slot may alias the input slot (each reads its input fully first).
 *   - Strings returned as a pointer are NUL-terminated. Use tb_strlen to size
 *     them. Those returned by tb_node_sexp are owned by the caller and must be
 *     released with tb_cstr_free; every other string points into static data
 *     and must NOT be freed.
 *   - tb_pack_abi() is THIS interface's version, independent of the grammar
 *     and of tree-sitter's own language ABI (tb_language_abi()). It changes
 *     only when a binding written against it would break.
 *
 * The provenance travels INSIDE the module (tb_provenance), not only beside
 * it. A .wasm that gets copied out of a release, vendored into a repo and
 * rediscovered two years later still answers "which upstream, which sha, which
 * patches, which toolchain" from its own bytes. A sibling JSON file cannot
 * make that promise, because the thing that goes missing is always the file
 * next to the binary.
 */
#include <stdlib.h>
#include <string.h>
#include "tree_sitter/api.h"

/* -D at build time: the grammar's exported entry point (tree_sitter_python,
 * tree_sitter_tsx, ...). One pack, one grammar. */
const TSLanguage *TREEBANK_LANGUAGE_FN(void);

/* Generated into embedded.c by tools/wasm-pack/build.sh from ledger.json,
 * roles.json and node-types.json. */
extern const unsigned char treebank_provenance_raw[];
extern const unsigned treebank_provenance_len;
extern const unsigned char treebank_roles_raw[];
extern const unsigned treebank_roles_len;
extern const unsigned char treebank_node_types_raw[];
extern const unsigned treebank_node_types_len;

#define TB_PACK_ABI 2

#define EXPORT(name) __attribute__((export_name(#name))) name

static TSParser *parser;

/* ---- identity and provenance ------------------------------------------- */

int EXPORT(tb_pack_abi)(void) { return TB_PACK_ABI; }
int EXPORT(tb_language_abi)(void) { return (int)ts_language_abi_version(TREEBANK_LANGUAGE_FN()); }
const char *EXPORT(tb_language_name)(void) { return ts_language_name(TREEBANK_LANGUAGE_FN()); }
const char *EXPORT(tb_provenance)(void) { return (const char *)treebank_provenance_raw; }
unsigned EXPORT(tb_provenance_len)(void) { return treebank_provenance_len; }

/* The facet manifest (roles.json). Table-tier roles are real supertypes and
 * queryable from the parser itself; facets are NOT in the parse table, so a
 * consumer without this cannot expand `(_callable)` at all. It ships inside
 * the module for the same reason provenance does. */
const char *EXPORT(tb_roles)(void) { return (const char *)treebank_roles_raw; }
unsigned EXPORT(tb_roles_len)(void) { return treebank_roles_len; }

/* The node manifest (node-types.json), which is where TABLE-tier membership
 * lives: that `while_statement` derives from `_loop`, and `_loop` from
 * `_statement`, is recorded here and nowhere else a pack consumer can reach.
 *
 * Supertypes are queryable from the parser only through a tree-sitter QUERY,
 * and this ABI deliberately exposes node walking rather than a query engine.
 * A host walking the tree therefore sees concrete kinds -- `while_statement`
 * -- and without this manifest has no way to learn it is a `_loop`. That made
 * the vocabulary, which is the whole point of a treebank grammar, the one
 * thing a pack could not answer for itself.
 *
 * It ships inside for the same reason provenance and roles do: the file next
 * to the binary is the thing that goes missing. */
const char *EXPORT(tb_node_types)(void) { return (const char *)treebank_node_types_raw; }
unsigned EXPORT(tb_node_types_len)(void) { return treebank_node_types_len; }

/* ---- memory ------------------------------------------------------------ */

/* The host writes source bytes into a buffer it gets from here. There is no
 * other way in: wasm linear memory is not the host's memory. */
char *EXPORT(tb_alloc)(unsigned len) { return (char *)malloc(len); }
void  EXPORT(tb_free)(void *p) { free(p); }
unsigned EXPORT(tb_strlen)(const char *s) { return s ? (unsigned)strlen(s) : 0u; }
void  EXPORT(tb_cstr_free)(char *s) { free(s); }

/* ---- parsing ----------------------------------------------------------- */

/* Returns a tree handle, or 0 if the parse failed outright (cancellation or
 * allocation failure — NOT a syntax error, which produces a tree with ERROR
 * nodes in it and is the normal way a parser reports bad input). */
TSTree *EXPORT(tb_parse)(const char *src, unsigned len) {
  if (!parser) {
    parser = ts_parser_new();
    if (!parser) return NULL;
    if (!ts_parser_set_language(parser, TREEBANK_LANGUAGE_FN())) return NULL;
  }
  return ts_parser_parse_string(parser, NULL, src, len);
}

void EXPORT(tb_tree_free)(TSTree *tree) { if (tree) ts_tree_delete(tree); }

/* ---- nodes ------------------------------------------------------------- */

TSNode *EXPORT(tb_node_new)(void) { return (TSNode *)calloc(1, sizeof(TSNode)); }
void    EXPORT(tb_node_free)(TSNode *n) { free(n); }
unsigned EXPORT(tb_node_size)(void) { return (unsigned)sizeof(TSNode); }

void EXPORT(tb_tree_root)(TSTree *tree, TSNode *out) { *out = ts_tree_root_node(tree); }

/* Node -> node. `out` may be the same slot as `n`: the input is read into a
 * local before the output is written. */
int EXPORT(tb_node_child)(const TSNode *n, unsigned i, TSNode *out) {
  TSNode self = *n;
  if (i >= ts_node_child_count(self)) return 0;
  *out = ts_node_child(self, i);
  return 1;
}
int EXPORT(tb_node_named_child)(const TSNode *n, unsigned i, TSNode *out) {
  TSNode self = *n;
  if (i >= ts_node_named_child_count(self)) return 0;
  *out = ts_node_named_child(self, i);
  return 1;
}
int EXPORT(tb_node_parent)(const TSNode *n, TSNode *out) {
  TSNode self = *n, p = ts_node_parent(self);
  if (ts_node_is_null(p)) return 0;
  *out = p;
  return 1;
}

const char *EXPORT(tb_node_type)(const TSNode *n) { return ts_node_type(*n); }
const char *EXPORT(tb_node_field_name_for_child)(const TSNode *n, unsigned i) {
  return ts_node_field_name_for_child(*n, i);
}
unsigned EXPORT(tb_node_child_count)(const TSNode *n) { return ts_node_child_count(*n); }
unsigned EXPORT(tb_node_named_child_count)(const TSNode *n) { return ts_node_named_child_count(*n); }
unsigned EXPORT(tb_node_start_byte)(const TSNode *n) { return ts_node_start_byte(*n); }
unsigned EXPORT(tb_node_end_byte)(const TSNode *n) { return ts_node_end_byte(*n); }
unsigned EXPORT(tb_node_start_row)(const TSNode *n) { return ts_node_start_point(*n).row; }
unsigned EXPORT(tb_node_start_column)(const TSNode *n) { return ts_node_start_point(*n).column; }
unsigned EXPORT(tb_node_end_row)(const TSNode *n) { return ts_node_end_point(*n).row; }
unsigned EXPORT(tb_node_end_column)(const TSNode *n) { return ts_node_end_point(*n).column; }

/* One call rather than five, because a cross-module call is the expensive part
 * of walking a tree from a host language. */
#define TB_NAMED   1u
#define TB_ERROR   2u   /* this node IS an ERROR node */
#define TB_HASERR  4u   /* this node or a descendant is an ERROR or MISSING */
#define TB_MISSING 8u
#define TB_EXTRA  16u
unsigned EXPORT(tb_node_flags)(const TSNode *n) {
  TSNode self = *n;
  return (ts_node_is_named(self)   ? TB_NAMED   : 0u)
       | (ts_node_is_error(self)   ? TB_ERROR   : 0u)
       | (ts_node_has_error(self)  ? TB_HASERR  : 0u)
       | (ts_node_is_missing(self) ? TB_MISSING : 0u)
       | (ts_node_is_extra(self)   ? TB_EXTRA   : 0u);
}

/* Caller owns the result; release it with tb_cstr_free. */
char *EXPORT(tb_node_sexp)(const TSNode *n) { return ts_node_string(*n); }
