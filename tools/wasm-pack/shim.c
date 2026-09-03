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
 *   - A query is compiled once (tb_query_new) and run against a node through a
 *     cursor (tb_query_exec). Captures come out one at a time rather than as
 *     an array of structs, for the same reason nodes do: wasm cannot return a
 *     struct, and a match list would have to be marshalled somewhere.
 *   - tb_pack_abi() is THIS interface's version, independent of the grammar
 *     and of tree-sitter's own language ABI (tb_language_abi()). It changes
 *     only when a binding written against it would break. Adding exports does
 *     not break one, so 2 -> 3 advertises queries rather than warning of a
 *     breakage.
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
 * terms.json and node-types.json. */
extern const unsigned char treebank_provenance_raw[];
extern const unsigned treebank_provenance_len;
extern const unsigned char treebank_terms_raw[];
extern const unsigned treebank_terms_len;
extern const unsigned char treebank_node_types_raw[];
extern const unsigned treebank_node_types_len;

#define TB_PACK_ABI 3

#define EXPORT(name) __attribute__((export_name(#name))) name

static TSParser *parser;

/* ---- identity and provenance ------------------------------------------- */

int EXPORT(tb_pack_abi)(void) { return TB_PACK_ABI; }
int EXPORT(tb_language_abi)(void) { return (int)ts_language_abi_version(TREEBANK_LANGUAGE_FN()); }
const char *EXPORT(tb_language_name)(void) { return ts_language_name(TREEBANK_LANGUAGE_FN()); }
const char *EXPORT(tb_provenance)(void) { return (const char *)treebank_provenance_raw; }
unsigned EXPORT(tb_provenance_len)(void) { return treebank_provenance_len; }

/* The nominal manifest (terms.json). A STRUCTURAL term is a real supertype
 * and queryable from the parser itself; a NOMINAL one is NOT in the parse
 * table, so a consumer without this cannot expand `(_callable)` at all. It
 * ships inside the module for the same reason provenance does.
 *
 * `tb_roles` is the same blob under the export name this had before the
 * vocabulary rename. Kept for one cycle because packs are content-addressed
 * and consumers pin them: a host built against the old name keeps working,
 * and treebank's own loader prefers `tb_terms` and falls back. */
const char *EXPORT(tb_terms)(void) { return (const char *)treebank_terms_raw; }
unsigned EXPORT(tb_terms_len)(void) { return treebank_terms_len; }
const char *EXPORT(tb_roles)(void) { return (const char *)treebank_terms_raw; }
unsigned EXPORT(tb_roles_len)(void) { return treebank_terms_len; }

/* The node manifest (node-types.json), which is where STRUCTURAL membership
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
 * It ships inside for the same reason provenance and terms do: the file next
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

/* ---- queries ------------------------------------------------------------
 *
 * This is what the shared vocabulary is for. `(_declaration) @d` runs against
 * any treebank grammar and finds that language's declarations, because the
 * role is a real supertype threaded through the productions rather than a
 * naming convention. Nominal terms are expanded into an alternation before
 * they get here, against the manifest in tb_terms.
 *
 * Captures are pulled one at a time. tree-sitter reports a match as an array
 * of TSQueryCapture structs, and marshalling that across the boundary would
 * mean either allocating a parallel array in module memory or teaching every
 * binding the struct layout. Pulling captures is the same shape the node
 * accessors already use, so a binding that can walk a tree can run a query.
 */

/* Compile a query. Returns NULL on a syntax error, and writes the byte offset
 * and TSQueryError into the out params so a caller can say WHERE it broke --
 * a query is usually written by a person, so the position is the whole
 * message. */
TSQuery *EXPORT(tb_query_new)(const char *src, unsigned len,
                              unsigned *err_offset, unsigned *err_type) {
  uint32_t offset = 0;
  TSQueryError type = TSQueryErrorNone;
  TSQuery *q = ts_query_new(TREEBANK_LANGUAGE_FN(), src, len, &offset, &type);
  if (err_offset) *err_offset = offset;
  if (err_type) *err_type = (unsigned)type;
  return q;
}

void EXPORT(tb_query_delete)(TSQuery *q) { ts_query_delete(q); }

unsigned EXPORT(tb_query_pattern_count)(const TSQuery *q) {
  return ts_query_pattern_count(q);
}

/* Start a run over `node`. The cursor owns the iteration state and must be
 * released with tb_query_cursor_delete. */
TSQueryCursor *EXPORT(tb_query_exec)(const TSQuery *q, const TSNode *node) {
  TSQueryCursor *cursor = ts_query_cursor_new();
  if (!cursor) return NULL;
  ts_query_cursor_exec(cursor, q, *node);
  return cursor;
}

void EXPORT(tb_query_cursor_delete)(TSQueryCursor *c) { ts_query_cursor_delete(c); }

/* Next capture, or 0 when the run is finished.
 *
 * Writes the captured node into the caller's slot and the pattern index into
 * *out_pattern. The capture NAME is fetched separately by index, because it
 * points into the query's own static strings and has a different lifetime
 * from the node. */
int EXPORT(tb_query_next_capture)(TSQueryCursor *c, const TSQuery *q,
                                  TSNode *out_node, unsigned *out_pattern,
                                  unsigned *out_capture) {
  TSQueryMatch match;
  uint32_t index = 0;
  if (!ts_query_cursor_next_capture(c, &match, &index)) return 0;
  if (index >= match.capture_count) return 0;
  const TSQueryCapture capture = match.captures[index];
  if (out_node) *out_node = capture.node;
  if (out_pattern) *out_pattern = match.pattern_index;
  if (out_capture) *out_capture = capture.index;
  (void)q;
  return 1;
}

/* The name a capture index stands for: `@name` without the at sign. Points
 * into the query and must not be freed; it dies with tb_query_delete. */
const char *EXPORT(tb_query_capture_name)(const TSQuery *q, unsigned index) {
  uint32_t len = 0;
  return ts_query_capture_name_for_id(q, index, &len);
}
