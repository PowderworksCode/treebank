// treebank-yaml external scanner.
//
// YAML's structure is columns, and columns are not expressible in a
// context-free rule, so every indicator whose meaning depends on what is
// already open is decided here rather than in the parse table
// (FIELD_GUIDE.md §1: use the highest rung of the ladder that can express
// the decision). This scanner is therefore closer to a full YAML lexer
// than to the small helpers the other treebank grammars carry, and the
// parse table it feeds has no declared conflicts at all.
//
// The state it keeps is three things:
//
//   * an INDENT STACK of the open block collections, each with its column
//     and whether it is a sequence. `_block_end` pops it; `-`, `?` and `:`
//     push it. Nothing else in the system can hold this.
//   * `line_column`, the column of the first content on the current line.
//     A block mapping's indentation is this column at the moment its first
//     `:` is emitted, which is why `- a: 1` opens its mapping at 2 and not
//     at the bullet's 0. It is also how a token knows whether it is where
//     the LINE began, which is what separates `&a` / newline / `  foo: 1`
//     (anchoring the mapping) from `&a foo: 1` (anchoring the key).
//   * `flow_depth`, non-zero inside `[…]` and `{…}`, where indentation
//     stops meaning anything and the plain-scalar and `:` rules change.
//
// EVERY token that can begin a line is owned here, including `&`, `*`,
// tags and the document markers, and that is not a stylistic choice. The
// runtime restores the scanner's serialized state before each scan, so a
// scan that returns FALSE loses everything it wrote — a `line_column` set
// while declining a token the internal lexer then took is a `line_column`
// the next call will not see. Ownership is what makes the state true.
//
// The scanner emits nothing it cannot justify from its own state: in error
// recovery every symbol is offered, so it declines outright rather than
// inventing structure (FIELD_GUIDE.md §8).

#include "tree_sitter/parser.h"

#include <stdlib.h>
#include <string.h>

enum TokenType {
  BLOCK_END,
  INDENTED,
  EMPTY_NODE,
  BLOCK_SEQ_BULLET,
  BLOCK_MAP_COLON,
  OWN_LINE_COLON,
  BLOCK_MAP_QUESTION,
  DOCUMENT_START,
  DOCUMENT_END,
  ANCHOR_SIGIL,
  ALIAS_SIGIL,
  FLOW_SEQ_START,
  FLOW_SEQ_END,
  FLOW_MAP_START,
  FLOW_MAP_END,
  TAG,
  PLAIN_SCALAR,
  SINGLE_QUOTE_SCALAR,
  DOUBLE_QUOTE_SCALAR,
  BLOCK_SCALAR,
  ERROR_SENTINEL,
};

// 3 bytes each, against a 1 KiB serialization buffer; deep enough for any
// real document and small enough that the state always fits.
#define MAX_LEVELS 128
#define NO_COLUMN 0xFFFF

typedef struct {
  uint16_t column;
  bool is_sequence;
} Level;

typedef struct {
  uint8_t level_count;
  Level levels[MAX_LEVELS];
  uint16_t flow_depth;
  uint16_t line_column;
  // The previous token was a quoted scalar or a closing flow bracket, so
  // an immediately adjacent `:` is a mapping indicator. This is YAML 1.2's
  // JSON-compatibility rule, and one of the very few places where the
  // version matters to the PARSE rather than to resolution.
  bool json_key;
  // The scalar just emitted crossed a line break. YAML's simple-key rule
  // says an implicit key is one line, so the `:` that follows such a
  // scalar cannot be a mapping indicator — which is what stops
  // `a: 1` / `  b: 2` from parsing as a nested mapping keyed on a folded
  // scalar instead of being the error it is.
  bool multiline_scalar;
} Scanner;

static inline void advance(TSLexer *lexer) { lexer->advance(lexer, false); }
static inline void skip(TSLexer *lexer) { lexer->advance(lexer, true); }

static inline bool is_space(int32_t c) { return c == ' ' || c == '\t'; }
static inline bool is_break(int32_t c) { return c == '\n' || c == '\r'; }
static inline bool is_line_end(int32_t c) { return c == 0 || is_break(c); }
static inline bool is_blank_or_end(int32_t c) {
  return c == 0 || is_space(c) || is_break(c);
}
static inline bool is_flow_indicator(int32_t c) {
  return c == ',' || c == '[' || c == ']' || c == '{' || c == '}';
}

// The innermost open collection's column, or -1 at the document level,
// where a node may sit at column 0 and still be inside nothing.
static inline int32_t top_column(const Scanner *s) {
  return s->level_count > 0 ? (int32_t)s->levels[s->level_count - 1].column : -1;
}

static inline bool top_is_sequence(const Scanner *s) {
  return s->level_count > 0 && s->levels[s->level_count - 1].is_sequence;
}

// Overflow declines rather than corrupting: past the cap the level is not
// tracked and the collection closes with its parent, which degrades a
// pathologically nested document instead of scribbling over the stack.
static void push_level(Scanner *s, uint32_t column, bool is_sequence) {
  if (s->level_count >= MAX_LEVELS) return;
  s->levels[s->level_count].column = (uint16_t)column;
  s->levels[s->level_count].is_sequence = is_sequence;
  s->level_count++;
}

// After a `-`, a `? ` or a document marker, whatever follows may still open
// a block collection of its own: the indicator counts as indentation.
static inline void reopen_line(Scanner *s) {
  s->line_column = NO_COLUMN;
  s->json_key = false;
  s->multiline_scalar = false;
}

// After ordinary content, nothing later on this line begins it.
static inline void took_content(Scanner *s) {
  s->json_key = false;
  s->multiline_scalar = false;
}

// `---` or `...`, three of the same character followed by a space or a
// break. Consumes what it inspects; every caller is either past `mark_end`
// already or about to return false, both of which discard the advance.
static bool scan_document_marker(TSLexer *lexer, int32_t first) {
  advance(lexer);
  if (lexer->lookahead != first) return false;
  advance(lexer);
  if (lexer->lookahead != first) return false;
  advance(lexer);
  return is_blank_or_end(lexer->lookahead);
}

static bool at_document_marker(TSLexer *lexer) {
  int32_t c = lexer->lookahead;
  if (c != '-' && c != '.') return false;
  return scan_document_marker(lexer, c);
}

// ── plain scalars ───────────────────────────────────────────────────────

// Does the plain scalar run on past this line break? The continuation must
// be indented past the collection that holds the scalar; a document marker
// and a comment line both end it, and so does the end of input. Consumes
// the breaks and the following indentation when the answer is yes.
static bool plain_continues(TSLexer *lexer, Scanner *s) {
  int32_t indent = top_column(s);
  for (;;) {
    while (is_break(lexer->lookahead)) advance(lexer);
    while (is_space(lexer->lookahead)) advance(lexer);
    if (is_break(lexer->lookahead)) continue;
    break;
  }
  int32_t c = lexer->lookahead;
  if (c == 0) return false;
  // A comment cannot appear inside a multi-line plain scalar, so a line
  // that starts one is past the scalar's end.
  if (c == '#') return false;
  uint32_t column = lexer->get_column(lexer);
  if (s->flow_depth == 0 && (int32_t)column <= indent) return false;
  if (column == 0 && at_document_marker(lexer)) return false;
  return true;
}

// A plain scalar, from its first character to wherever the indentation, a
// `:` indicator, a comment or a flow indicator ends it. `mark_end` moves
// only past CONTENT, so trailing spaces before a terminator are left out
// of the token; everything the loop consumes beyond it is free lookahead.
static bool scan_plain(TSLexer *lexer, Scanner *s, bool started) {
  bool has_content = started;
  bool folded = false;
  if (started) lexer->mark_end(lexer);

  for (;;) {
    int32_t c = lexer->lookahead;
    if (c == 0) break;
    if (is_break(c)) {
      if (!plain_continues(lexer, s)) break;
      folded = true;
      continue;
    }
    if (is_space(c)) {
      while (is_space(lexer->lookahead)) advance(lexer);
      // ` #` opens a comment; the scalar ended before the spaces.
      if (lexer->lookahead == '#') break;
      continue;
    }
    if (c == ':') {
      advance(lexer);
      int32_t after = lexer->lookahead;
      if (is_blank_or_end(after) ||
          (s->flow_depth > 0 && is_flow_indicator(after))) {
        break;
      }
      lexer->mark_end(lexer);
      has_content = true;
      continue;
    }
    if (s->flow_depth > 0 && is_flow_indicator(c)) break;
    advance(lexer);
    lexer->mark_end(lexer);
    has_content = true;
  }

  if (!has_content) return false;
  took_content(s);
  s->multiline_scalar = folded;
  lexer->result_symbol = PLAIN_SCALAR;
  return true;
}

// The indicators a plain scalar may not open with. `-`, `?` and `:` are
// absent because they are decided one character later, by the caller.
static bool plain_can_start(int32_t c) {
  switch (c) {
    case 0:
    case ' ':
    case '\t':
    case '\n':
    case '\r':
    case ',':
    case '[':
    case ']':
    case '{':
    case '}':
    case '#':
    case '&':
    case '*':
    case '!':
    case '|':
    case '>':
    case '\'':
    case '"':
    case '%':
    case '@':
    case '`':
      return false;
    default:
      return true;
  }
}

// ── quoted scalars ──────────────────────────────────────────────────────

static bool scan_single_quote(TSLexer *lexer, Scanner *s) {
  bool folded = false;
  advance(lexer);
  for (;;) {
    int32_t c = lexer->lookahead;
    if (is_break(c)) folded = true;
    if (c == 0) return false; // unterminated: not a token, let the parse fail
    if (c == '\'') {
      advance(lexer);
      if (lexer->lookahead == '\'') { // '' is one escaped quote
        advance(lexer);
        continue;
      }
      lexer->mark_end(lexer);
      took_content(s);
      s->json_key = true;
      s->multiline_scalar = folded;
      lexer->result_symbol = SINGLE_QUOTE_SCALAR;
      return true;
    }
    advance(lexer);
  }
}

// The escape after a backslash inside a double-quoted scalar. YAML's list
// is closed and short, and a scalar carrying anything else is not YAML —
// `"\\."` is rejected by every implementation. Validating it here is the
// cheapest kind of correctness: one switch, no grammar, and the widening it
// closes is one a corpus of valid files could never have shown.
static bool scan_hex(TSLexer *lexer, int digits) {
  for (int i = 0; i < digits; i++) {
    int32_t c = lexer->lookahead;
    bool hex = (c >= '0' && c <= '9') || (c >= 'a' && c <= 'f') ||
               (c >= 'A' && c <= 'F');
    if (!hex) return false;
    advance(lexer);
  }
  return true;
}

static bool scan_escape(TSLexer *lexer) {
  int32_t c = lexer->lookahead;
  switch (c) {
    case '0': case 'a': case 'b': case 't': case 'n': case 'v': case 'f':
    case 'r': case 'e': case ' ': case '"': case '/': case '\\':
    case 'N': case '_': case 'L': case 'P': case '\t':
      advance(lexer);
      return true;
    case 'x':
      advance(lexer);
      return scan_hex(lexer, 2);
    case 'u':
      advance(lexer);
      return scan_hex(lexer, 4);
    case 'U':
      advance(lexer);
      return scan_hex(lexer, 8);
    case '\r':
    case '\n':
      // An escaped line break: the scalar continues on the next line.
      advance(lexer);
      return true;
    default:
      return false;
  }
}

static bool scan_double_quote(TSLexer *lexer, Scanner *s) {
  bool folded = false;
  advance(lexer);
  for (;;) {
    int32_t c = lexer->lookahead;
    if (is_break(c)) folded = true;
    if (c == 0) return false;
    if (c == '\\') {
      advance(lexer);
      if (!scan_escape(lexer)) return false;
      continue;
    }
    if (c == '"') {
      advance(lexer);
      lexer->mark_end(lexer);
      took_content(s);
      s->json_key = true;
      s->multiline_scalar = folded;
      lexer->result_symbol = DOUBLE_QUOTE_SCALAR;
      return true;
    }
    advance(lexer);
  }
}

// ── tags ────────────────────────────────────────────────────────────────

// `!`, `!!str`, `!local`, `!e!suffix`, `!<verbatim:uri>`. Owned here rather
// than written as a token in the grammar because a tag can begin a line,
// and a token the scanner does not see is a `line_column` it cannot keep.
static bool scan_tag(TSLexer *lexer, Scanner *s) {
  advance(lexer); // '!'
  if (lexer->lookahead == '<') {
    advance(lexer);
    while (lexer->lookahead != '>' && !is_line_end(lexer->lookahead)) {
      advance(lexer);
    }
    if (lexer->lookahead != '>') return false;
    advance(lexer);
  } else {
    while (!is_blank_or_end(lexer->lookahead) &&
           !is_flow_indicator(lexer->lookahead)) {
      advance(lexer);
    }
  }
  lexer->mark_end(lexer);
  took_content(s);
  lexer->result_symbol = TAG;
  return true;
}

// ── block scalars ───────────────────────────────────────────────────────

// `|` or `>`, an optional indentation indicator and chomping marker in
// either order, then every following line indented past the parent. The
// token ends at the last CONTENT line, so trailing blank lines are left
// outside it and the next token starts at a line boundary.
static bool scan_block_scalar(TSLexer *lexer, Scanner *s) {
  int32_t parent = top_column(s);
  advance(lexer); // '|' or '>'

  int32_t explicit_indent = 0;
  bool seen_indent = false;
  bool seen_chomp = false;
  for (;;) {
    int32_t c = lexer->lookahead;
    if (!seen_indent && c >= '1' && c <= '9') {
      explicit_indent = c - '0';
      seen_indent = true;
      advance(lexer);
    } else if (!seen_chomp && (c == '+' || c == '-')) {
      seen_chomp = true;
      advance(lexer);
    } else {
      break;
    }
  }
  bool spaced = false;
  while (is_space(lexer->lookahead)) {
    spaced = true;
    advance(lexer);
  }
  // `>#comment` is not a comment: YAML requires whitespace in front of one,
  // and without the rule the header would swallow anything at all.
  if (lexer->lookahead == '#') {
    if (!spaced) return false;
    while (!is_line_end(lexer->lookahead)) advance(lexer);
  }
  // Anything else on the header line means this was never a block scalar.
  if (!is_line_end(lexer->lookahead)) return false;
  lexer->mark_end(lexer);

  int32_t content_indent =
      seen_indent ? (parent < 0 ? 0 : parent) + explicit_indent : -1;

  for (;;) {
    if (!is_break(lexer->lookahead)) break;
    if (lexer->lookahead == '\r') advance(lexer);
    if (lexer->lookahead == '\n') advance(lexer);
    while (lexer->lookahead == ' ') advance(lexer);
    int32_t c = lexer->lookahead;
    if (c == 0) break;
    if (is_break(c)) continue; // a blank line, which may yet be interior
    uint32_t column = lexer->get_column(lexer);
    if (content_indent < 0) {
      // No indicator: the first content line sets the indentation.
      if ((int32_t)column <= parent) break;
      content_indent = (int32_t)column;
    }
    if ((int32_t)column < content_indent) break;
    if (column == 0 && at_document_marker(lexer)) break;
    while (!is_line_end(lexer->lookahead)) advance(lexer);
    lexer->mark_end(lexer);
  }

  took_content(s);
  lexer->result_symbol = BLOCK_SCALAR;
  return true;
}

// ── the scan ────────────────────────────────────────────────────────────

bool tree_sitter_yaml_external_scanner_scan(void *payload, TSLexer *lexer,
                                            const bool *valid_symbols) {
  Scanner *s = (Scanner *)payload;

  // Every symbol valid means error recovery, where nothing this scanner
  // could emit is justified by its own state and a zero-width token would
  // loop the parser forever.
  if (valid_symbols[ERROR_SENTINEL]) return false;

  // One skip pass. The scanner runs BEFORE extras, so every kind of
  // trivia in front of a token it owns has to be handled here.
  //
  // It also watches for the one whitespace rule YAML states outright: a
  // tab may never be used for INDENTATION, though it is ordinary
  // separation everywhere else.
  //
  // Where indentation ends is the whole of the rule, and it is a COLUMN.
  // A line's indentation runs up to the column of the collection that
  // holds it, so a tab at or before that column is indentation and a tab
  // past it is separation: `a:` then a tab then `- b` is not YAML, while
  // `foo:` then space-tab-`bar` is, and at the document level — where the
  // enclosing indentation is -1 — a leading tab is separation and
  // `\t{}` is a legal document.
  bool saw_break = false;
  bool at_line_head = lexer->get_column(lexer) == 0;
  bool tab_in_indent = false;
  for (;;) {
    int32_t c = lexer->lookahead;
    if (is_space(c)) {
      if (c == '\t' && at_line_head &&
          (int32_t)lexer->get_column(lexer) <= top_column(s)) {
        tab_in_indent = true;
      }
      skip(lexer);
    } else if (is_break(c)) {
      skip(lexer);
      saw_break = true;
      at_line_head = true;
      tab_in_indent = false;
    } else {
      break;
    }
  }
  if (saw_break) {
    s->line_column = NO_COLUMN;
    // Inside a flow collection a line break is not a boundary, so a quoted
    // key keeps its right to an adjacent `:` across one: `{ "foo"` then
    // `  :bar }` is YAML 1.2's JSON rule spanning two lines.
    if (s->flow_depth == 0) s->json_key = false;
  }

  // Everything consumed past here is lookahead: a zero-width token still
  // lands at this position, and a token that consumes text either starts
  // here or is not produced at all.
  lexer->mark_end(lexer);

  int32_t c = lexer->lookahead;
  uint32_t column = lexer->get_column(lexer);
  bool starts_the_line =
      s->line_column == NO_COLUMN || (uint16_t)column == s->line_column;
  if (s->line_column == NO_COLUMN && !is_line_end(c)) {
    s->line_column = (uint16_t)column;
  }

  // A tab in a line's indentation is not YAML, and nothing downstream can
  // say so: the parse table sees columns, not characters. Declining every
  // token here is what turns it into a parse error. Comments are exempt
  // because a comment line carries no indentation of its own, and so is
  // everything inside a flow collection, where indentation has no meaning
  // left to violate.
  if (tab_in_indent && s->flow_depth == 0 && c != '#' && !is_line_end(c)) {
    return false;
  }

  // A comment is a node, not trivia this scanner may swallow: decline and
  // let the extra take it. But close first any block the comment is not
  // inside, so a trailing comment does not extend the collection above it.
  if (c == '#') {
    if (valid_symbols[BLOCK_END] && s->flow_depth == 0 && s->level_count > 0) {
      // Scan to the next line with CODE on it, past any run of comment and
      // blank lines. A blank line is not always empty — a line of trailing
      // spaces is the commonest whitespace in a hand-edited file — and
      // measuring the column of its indentation instead of the next real
      // line's closed every collection above a `# comment` that happened
      // to be followed by one.
      for (;;) {
        while (!is_line_end(lexer->lookahead)) advance(lexer);
        if (lexer->lookahead == 0) break;
        while (is_break(lexer->lookahead)) advance(lexer);
        while (is_space(lexer->lookahead)) advance(lexer);
        if (lexer->lookahead == '#' || is_break(lexer->lookahead)) continue;
        break;
      }
      if (lexer->lookahead == 0 ||
          (int32_t)lexer->get_column(lexer) < top_column(s)) {
        s->level_count--;
        lexer->result_symbol = BLOCK_END;
        return true;
      }
    }
    return false;
  }

  // What does this line open with? Both probes consume, and both are safe:
  // a `-` or a `.` that turns out to be neither an indicator nor a marker
  // is the first character of a plain scalar and the scan continues from
  // where the probe stopped.
  bool is_bullet = false;
  bool is_marker = false;
  bool dash_then_space = false;
  // The probes below consume, and what they consume belongs to a plain
  // scalar when they come to nothing: `-: ""` is a mapping whose key is a
  // one-character scalar, and `..foo` is a scalar too. Losing that prefix
  // is how `-: ""` became a parse error over a hundred thousand files.
  bool probed = false;
  if (c == '-') {
    advance(lexer);
    // A dash is plain-scalar content only where a plain scalar could
    // continue past it. `[-]` is not a sequence and not a scalar either:
    // inside a flow collection the bracket that follows is not plain-safe,
    // so the dash belongs to nothing and the file is not YAML.
    probed = !(s->flow_depth > 0 && is_flow_indicator(lexer->lookahead));
    if (is_blank_or_end(lexer->lookahead)) {
      dash_then_space = true;
      // An entry indicator only where a block collection may begin: at the
      // head of a line, or behind another indicator that counts as
      // indentation. `key: - a` is not a sequence, it is an error, and
      // this is where that is decided — the parse table sees no columns
      // and could not tell the two apart.
      is_bullet = s->flow_depth == 0 && starts_the_line;
    } else if (column == 0 && lexer->lookahead == '-') {
      advance(lexer);
      if (lexer->lookahead == '-') {
        advance(lexer);
        is_marker = is_blank_or_end(lexer->lookahead);
      }
    }
  } else if (c == '.' && column == 0) {
    probed = true;
    is_marker = scan_document_marker(lexer, '.');
  }

  // Close the innermost collection where the column, a document marker or
  // the end of input says it is over. Zero width, so the probes above cost
  // nothing here.
  if (valid_symbols[BLOCK_END] && s->flow_depth == 0 && s->level_count > 0) {
    bool close = false;
    if (c == 0 || is_marker) {
      close = true;
    } else {
      int32_t top = top_column(s);
      if ((int32_t)column < top) {
        close = true;
      } else if ((int32_t)column == top) {
        if (top_is_sequence(s)) {
          // A sequence continues only through another entry indicator.
          close = !is_bullet;
        } else {
          // A bullet at a mapping's own column is that mapping's compact
          // sequence value — but only where the parser can still take one,
          // either directly or behind the `_indented` that a node carrying
          // properties needs first. `seq:` / ` &anchor` / `- a` is the
          // case that needs the second half: the bullet is not yet valid
          // there, only the marker in front of it is, and closing the
          // mapping would strand the anchor.
          close = is_bullet && !valid_symbols[BLOCK_SEQ_BULLET] &&
                  !valid_symbols[INDENTED];
        }
      }
    }
    if (close) {
      s->level_count--;
      lexer->result_symbol = BLOCK_END;
      return true;
    }
  }

  if (c == 0) return false;

  // The two answers to "is there a node here, and where does it start".
  // Both are zero width and both are offered only where a node MAY follow,
  // never where a KEY may — which is what lets the same token at the same
  // column be a value on one line and the next key on the other.
  if (s->flow_depth == 0 && starts_the_line) {
    int32_t top = top_column(s);
    // A block sequence that is a MAPPING's value may sit at the mapping's
    // own column — `key:` then `- a` at column 0 is YAML's compact form,
    // and the entry indicator is what makes it unambiguous. So the value
    // position accepts it as indented even though the column has not
    // moved; without that, an anchor in front of one (`key: &a` then `- a`)
    // ends the mapping instead of anchoring the sequence.
    bool compact_sequence =
        is_bullet && (int32_t)column == top && !top_is_sequence(s);
    if (valid_symbols[INDENTED] && ((int32_t)column > top || compact_sequence)) {
      lexer->result_symbol = INDENTED;
      return true;
    }
    // Not indented past the collection, so the node is empty — unless a
    // bullet at a mapping's own column is about to open that mapping's
    // compact sequence value.
    if (valid_symbols[EMPTY_NODE] && s->level_count > 0 &&
        (int32_t)column <= top && !compact_sequence) {
      lexer->result_symbol = EMPTY_NODE;
      return true;
    }
  }

  if (is_marker) {
    bool start = c == '-';
    if (start ? !valid_symbols[DOCUMENT_START] : !valid_symbols[DOCUMENT_END]) {
      return false;
    }
    lexer->mark_end(lexer); // the three characters, not the space after
    reopen_line(s);
    lexer->result_symbol = start ? DOCUMENT_START : DOCUMENT_END;
    return true;
  }

  // `- ` where an entry indicator cannot go is not a plain scalar either:
  // a plain scalar may open with a dash only when a non-space follows it.
  if (dash_then_space && !is_bullet) return false;

  if (is_bullet) {
    if (!valid_symbols[BLOCK_SEQ_BULLET]) return false;
    lexer->mark_end(lexer); // the token is the `-` alone
    int32_t top = top_column(s);
    if (s->level_count == 0 || (int32_t)column > top ||
        ((int32_t)column == top && !top_is_sequence(s))) {
      push_level(s, column, true);
    }
    reopen_line(s);
    lexer->result_symbol = BLOCK_SEQ_BULLET;
    return true;
  }

  if (c == ':') {
    advance(lexer);
    int32_t after = lexer->lookahead;
    bool indicator =
        is_blank_or_end(after) ||
        (s->flow_depth > 0 && (is_flow_indicator(after) || s->json_key));
    // Two tokens for one spelling, and the split is the whole reason
    // `? a` / `: b` parses as one pair rather than as a mapping nested
    // inside its own key. An implicit key and its `:` share a line, so a
    // colon that BEGINS a line cannot belong to the key above it — it can
    // only be the value indicator of an explicit `?` pair or of a pair
    // with no key. The parse table offers both readings and the lexer
    // starves the wrong one (FIELD_GUIDE.md §1, rung 1; §4's hazard used
    // on purpose, with the scanner owning the spelling everywhere).
    int colon = starts_the_line ? OWN_LINE_COLON : BLOCK_MAP_COLON;
    // An implicit key in BLOCK context is one line — YAML's simple-key
    // rule — so a `:` that follows a scalar which folded across a break is
    // not an indicator at all, and `a: 1` / `  b: 2` is the error it looks
    // like rather than a mapping keyed on a folded scalar.
    //
    // Two exemptions, both measured on yaml-test-suite rather than
    // reasoned from the specification, which forbids the shape in both
    // contexts. Inside a flow collection every implementation accepts
    // `{ multi` / `  line: value }`, so the rule is not applied there. And
    // an EXPLICIT `? key` may span as many lines as it likes; its `:`
    // begins a line of its own and is the other token.
    if (colon == BLOCK_MAP_COLON && s->flow_depth == 0 && s->multiline_scalar) {
      indicator = false;
    }
    if (indicator && valid_symbols[colon]) {
      lexer->mark_end(lexer);
      if (s->flow_depth == 0) {
        uint32_t key_column =
            s->line_column == NO_COLUMN ? column : s->line_column;
        if (s->level_count == 0 || (int32_t)key_column > top_column(s)) {
          push_level(s, key_column, false);
        }
      }
      // A `:` that begins its line is an explicit pair's value indicator,
      // and YAML lets a compact collection follow it on the same line the
      // way one may follow a `-`: `: moon: white` is a mapping. A `:` that
      // follows its key on the same line may NOT, which is what keeps
      // `a: b: c` from parsing as a nested mapping.
      if (colon == OWN_LINE_COLON) {
        reopen_line(s);
      } else {
        took_content(s);
      }
      lexer->result_symbol = colon;
      return true;
    }
    if (indicator || !valid_symbols[PLAIN_SCALAR]) return false;
    return scan_plain(lexer, s, true);
  }

  if (c == '?') {
    advance(lexer);
    // `?foo` is a plain scalar that happens to start with one.
    if (!is_blank_or_end(lexer->lookahead)) {
      if (!valid_symbols[PLAIN_SCALAR]) return false;
      return scan_plain(lexer, s, true);
    }
    if (!valid_symbols[BLOCK_MAP_QUESTION]) return false;
    lexer->mark_end(lexer);
    if (s->flow_depth == 0) {
      // The explicit-key form is the one shape of mapping with no `:` to
      // open it — `? a` / `? b` is a set — so the level is pushed here.
      uint32_t key_column =
          s->line_column == NO_COLUMN ? column : s->line_column;
      if (s->level_count == 0 || (int32_t)key_column > top_column(s)) {
        push_level(s, key_column, false);
      }
    }
    reopen_line(s);
    lexer->result_symbol = BLOCK_MAP_QUESTION;
    return true;
  }

  if (c == '[' && valid_symbols[FLOW_SEQ_START]) {
    advance(lexer);
    lexer->mark_end(lexer);
    s->flow_depth++;
    took_content(s);
    lexer->result_symbol = FLOW_SEQ_START;
    return true;
  }
  if (c == '{' && valid_symbols[FLOW_MAP_START]) {
    advance(lexer);
    lexer->mark_end(lexer);
    s->flow_depth++;
    took_content(s);
    lexer->result_symbol = FLOW_MAP_START;
    return true;
  }
  if (c == ']' && valid_symbols[FLOW_SEQ_END]) {
    advance(lexer);
    lexer->mark_end(lexer);
    if (s->flow_depth > 0) s->flow_depth--;
    s->json_key = true;
    lexer->result_symbol = FLOW_SEQ_END;
    return true;
  }
  if (c == '}' && valid_symbols[FLOW_MAP_END]) {
    advance(lexer);
    lexer->mark_end(lexer);
    if (s->flow_depth > 0) s->flow_depth--;
    s->json_key = true;
    lexer->result_symbol = FLOW_MAP_END;
    return true;
  }

  if (c == '&' && valid_symbols[ANCHOR_SIGIL]) {
    advance(lexer);
    lexer->mark_end(lexer);
    took_content(s);
    lexer->result_symbol = ANCHOR_SIGIL;
    return true;
  }
  if (c == '*' && valid_symbols[ALIAS_SIGIL]) {
    advance(lexer);
    lexer->mark_end(lexer);
    took_content(s);
    lexer->result_symbol = ALIAS_SIGIL;
    return true;
  }

  if (c == '!' && valid_symbols[TAG]) return scan_tag(lexer, s);

  if ((c == '|' || c == '>') && valid_symbols[BLOCK_SCALAR] &&
      s->flow_depth == 0) {
    return scan_block_scalar(lexer, s);
  }

  if (c == '\'' && valid_symbols[SINGLE_QUOTE_SCALAR]) {
    return scan_single_quote(lexer, s);
  }
  if (c == '"' && valid_symbols[DOUBLE_QUOTE_SCALAR]) {
    return scan_double_quote(lexer, s);
  }

  if (valid_symbols[PLAIN_SCALAR] && plain_can_start(c)) {
    return scan_plain(lexer, s, probed);
  }

  return false;
}

unsigned tree_sitter_yaml_external_scanner_serialize(void *payload,
                                                     char *buffer) {
  Scanner *s = (Scanner *)payload;
  unsigned i = 0;
  buffer[i++] = (char)s->level_count;
  for (uint32_t k = 0; k < s->level_count; k++) {
    memcpy(&buffer[i], &s->levels[k].column, sizeof(uint16_t));
    i += sizeof(uint16_t);
    buffer[i++] = (char)s->levels[k].is_sequence;
  }
  memcpy(&buffer[i], &s->flow_depth, sizeof(uint16_t));
  i += sizeof(uint16_t);
  memcpy(&buffer[i], &s->line_column, sizeof(uint16_t));
  i += sizeof(uint16_t);
  buffer[i++] = (char)s->json_key;
  buffer[i++] = (char)s->multiline_scalar;
  return i;
}

void tree_sitter_yaml_external_scanner_deserialize(void *payload,
                                                   const char *buffer,
                                                   unsigned length) {
  Scanner *s = (Scanner *)payload;
  memset(s, 0, sizeof(Scanner));
  // A fresh parse starts at the head of the first line, inside nothing.
  s->line_column = NO_COLUMN;
  if (length == 0) return;
  unsigned i = 0;
  s->level_count = (uint8_t)buffer[i++];
  for (uint32_t k = 0; k < s->level_count; k++) {
    memcpy(&s->levels[k].column, &buffer[i], sizeof(uint16_t));
    i += sizeof(uint16_t);
    s->levels[k].is_sequence = buffer[i++] != 0;
  }
  memcpy(&s->flow_depth, &buffer[i], sizeof(uint16_t));
  i += sizeof(uint16_t);
  memcpy(&s->line_column, &buffer[i], sizeof(uint16_t));
  i += sizeof(uint16_t);
  s->json_key = buffer[i++] != 0;
  s->multiline_scalar = buffer[i++] != 0;
}

void *tree_sitter_yaml_external_scanner_create(void) {
  Scanner *s = calloc(1, sizeof(Scanner));
  if (s != NULL) s->line_column = NO_COLUMN;
  return s;
}

void tree_sitter_yaml_external_scanner_destroy(void *payload) { free(payload); }
