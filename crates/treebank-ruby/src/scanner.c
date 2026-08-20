// treebank-ruby external scanner.
//
// Four jobs, all of them things ruby's parse table cannot do alone:
//
// 1. Statement boundaries. A newline ends a statement except where the
//    expression is unfinished (the parse state says so: _line_break is not
//    valid there) or the NEXT line begins with `.` / `&.`, which continues
//    a method chain — a fact one token of lookahead past the newline
//    settles, and nothing in a parse table can.
// 2. Delimited literals: strings, symbols, subshells, regexes, %-literals
//    and word arrays, all on one stack so interpolations can nest them.
//    Content stops at `#{` so the grammar parses interpolations, and at
//    `\` so escape sequences are real nodes.
// 3. Heredocs: `<<~EOS` is recognised here (telling it from `<<` shift
//    needs the delimiter shape), and the BODY is found after the line
//    ends, off a queue — two heredocs on one line are two queued bodies.
// 4. The spacing-sensitive operators. `a * b` is a product and `foo *args`
//    a splat; `a / b` a quotient and `foo /x/` a regex argument. Ruby's
//    own lexer decides these from whitespace and parser state, so this one
//    does too: each spelling is two different tokens, and the parse table
//    never sees the ambiguity.
//
// The scanner never emits a verdict it cannot justify from its own state:
// in error recovery (the sentinel is valid) it resets its stacks and
// declines everything but a plain newline, so it can never loop.

#include "tree_sitter/parser.h"

#include <stdlib.h>
#include <string.h>

enum TokenType {
  LINE_BREAK,
  STRING_START,
  SYMBOL_START,
  SUBSHELL_START,
  REGEX_START,
  WORDS_START,
  SYMBOLS_START,
  STRING_CONTENT,
  STRING_END,
  ESCAPE_SEQUENCE,
  HEREDOC_BEGINNING,
  HEREDOC_BODY_START,
  HEREDOC_CONTENT,
  HEREDOC_END,
  HASH_KEY,
  IDENTIFIER_SUFFIX,
  BINARY_STAR,
  SPLAT_STAR,
  BINARY_STAR_STAR,
  SPLAT_STAR_STAR,
  BINARY_AMP,
  BLOCK_AMP,
  BINARY_SLASH,
  BINARY_MINUS,
  UNARY_MINUS,
  BINARY_PLUS,
  UNARY_PLUS,
  BLOCK_COMMENT,
  SIMPLE_SYMBOL,
  ERROR_SENTINEL,
};

enum LiteralKind {
  LIT_STRING,
  LIT_SYMBOL,
  LIT_SUBSHELL,
  LIT_REGEX,
  LIT_WORDS,
  LIT_SYMBOLS,
};

#define MAX_LITERALS 24
#define MAX_HEREDOCS 8
#define MAX_HEREDOC_ID 60

typedef struct {
  uint8_t kind;
  uint8_t open;     // 0 when the delimiter pair is not nestable
  uint8_t close;
  uint8_t nesting;
  bool interpolates;
} Literal;

typedef struct {
  uint8_t len;
  bool indent_close;   // <<- and <<~: the closer may be indented
  bool interpolates;   // false for <<'EOS'
  bool started;        // body reached; content/end may be produced
  bool at_line_start;  // the next content request sits at a line start
  char id[MAX_HEREDOC_ID];
} Heredoc;

typedef struct {
  uint8_t literal_count;
  Literal literals[MAX_LITERALS];
  uint8_t heredoc_count;
  Heredoc heredocs[MAX_HEREDOCS];
} Scanner;

static inline void advance(TSLexer *lexer) { lexer->advance(lexer, false); }
static inline void skip(TSLexer *lexer) { lexer->advance(lexer, true); }

static inline bool is_ident_start(int32_t c) {
  return (c >= 'a' && c <= 'z') || (c >= 'A' && c <= 'Z') || c == '_';
}
static inline bool is_ident_char(int32_t c) {
  return is_ident_start(c) || (c >= '0' && c <= '9');
}
static inline bool is_space(int32_t c) {
  return c == ' ' || c == '\t' || c == '\r' || c == '\f' || c == '\v';
}

// The nestable delimiter pairs %-literals may use.
static int32_t close_for(int32_t open) {
  switch (open) {
    case '(': return ')';
    case '[': return ']';
    case '{': return '}';
    case '<': return '>';
    default: return 0;
  }
}

// ── escapes ──────────────────────────────────────────────────────────────

// One escape sequence, backslash included: `\n`, `\x41`, `\u{1f600}`,
// `\C-c`, and the line continuation `\<newline>`. Only called where the
// literal interpolates — raw literals consume their two escapes as content.
static bool scan_escape(TSLexer *lexer) {
  advance(lexer); // the backslash
  int32_t c = lexer->lookahead;
  if (c == 0) return false;
  switch (c) {
    case 'u':
      advance(lexer);
      if (lexer->lookahead == '{') {
        advance(lexer);
        while (lexer->lookahead != 0 && lexer->lookahead != '}' &&
               lexer->lookahead != '\n') {
          advance(lexer);
        }
        if (lexer->lookahead == '}') advance(lexer);
      } else {
        for (int i = 0; i < 4; i++) {
          int32_t h = lexer->lookahead;
          bool hex = (h >= '0' && h <= '9') || (h >= 'a' && h <= 'f') ||
                     (h >= 'A' && h <= 'F');
          if (!hex) break;
          advance(lexer);
        }
      }
      break;
    case 'x':
      advance(lexer);
      for (int i = 0; i < 2; i++) {
        int32_t h = lexer->lookahead;
        bool hex = (h >= '0' && h <= '9') || (h >= 'a' && h <= 'f') ||
                   (h >= 'A' && h <= 'F');
        if (!hex) break;
        advance(lexer);
      }
      break;
    case '0': case '1': case '2': case '3':
    case '4': case '5': case '6': case '7':
      for (int i = 0; i < 3 && lexer->lookahead >= '0' && lexer->lookahead <= '7'; i++) {
        advance(lexer);
      }
      break;
    case 'c': case 'C': case 'M':
      advance(lexer);
      if (lexer->lookahead == '-') advance(lexer);
      if (lexer->lookahead == '\\') advance(lexer);
      if (lexer->lookahead != 0) advance(lexer);
      break;
    case '\r':
      advance(lexer);
      if (lexer->lookahead == '\n') advance(lexer);
      break;
    default:
      advance(lexer);
      break;
  }
  lexer->mark_end(lexer);
  lexer->result_symbol = ESCAPE_SEQUENCE;
  return true;
}

// ── delimited literals ───────────────────────────────────────────────────

static bool literal_start_valid(const bool *valid, uint8_t kind) {
  switch (kind) {
    case LIT_STRING: return valid[STRING_START];
    case LIT_SYMBOL: return valid[SYMBOL_START];
    case LIT_SUBSHELL: return valid[SUBSHELL_START];
    case LIT_REGEX: return valid[REGEX_START];
    case LIT_WORDS: return valid[WORDS_START];
    case LIT_SYMBOLS: return valid[SYMBOLS_START];
    default: return false;
  }
}

static uint16_t literal_start_symbol(uint8_t kind) {
  switch (kind) {
    case LIT_SYMBOL: return SYMBOL_START;
    case LIT_SUBSHELL: return SUBSHELL_START;
    case LIT_REGEX: return REGEX_START;
    case LIT_WORDS: return WORDS_START;
    case LIT_SYMBOLS: return SYMBOLS_START;
    default: return STRING_START;
  }
}

static bool push_literal(Scanner *s, TSLexer *lexer, uint8_t kind,
                         int32_t open, int32_t close, bool interpolates) {
  if (s->literal_count >= MAX_LITERALS) return false;
  Literal *l = &s->literals[s->literal_count++];
  l->kind = kind;
  l->open = (uint8_t)(open > 0 && open < 128 ? open : 0);
  l->close = (uint8_t)close;
  l->nesting = 0;
  l->interpolates = interpolates;
  lexer->mark_end(lexer);
  lexer->result_symbol = literal_start_symbol(kind);
  return true;
}

// `%q(…)`, `%w[…]`, `%r{…}`, bare `%(…)` — the whole family. Consumes the
// `%`, the letter if any, and the opening delimiter. Returns false (all
// consumption discarded) when the shape is not a percent literal, so
// `a % b` and `a %= b` fall back to the operators they are.
static bool scan_percent_literal(Scanner *s, TSLexer *lexer, const bool *valid) {
  advance(lexer); // %
  uint8_t kind;
  bool interpolates;
  int32_t c = lexer->lookahead;
  switch (c) {
    case 'q': kind = LIT_STRING; interpolates = false; advance(lexer); break;
    case 'Q': kind = LIT_STRING; interpolates = true; advance(lexer); break;
    case 'w': kind = LIT_WORDS; interpolates = false; advance(lexer); break;
    case 'W': kind = LIT_WORDS; interpolates = true; advance(lexer); break;
    case 'i': kind = LIT_SYMBOLS; interpolates = false; advance(lexer); break;
    case 'I': kind = LIT_SYMBOLS; interpolates = true; advance(lexer); break;
    case 'r': kind = LIT_REGEX; interpolates = true; advance(lexer); break;
    case 's': kind = LIT_SYMBOL; interpolates = false; advance(lexer); break;
    case 'x': kind = LIT_SUBSHELL; interpolates = true; advance(lexer); break;
    default: kind = LIT_STRING; interpolates = true; break;
  }
  int32_t delim = lexer->lookahead;
  // `=` is a legal delimiter to CRuby but taking it would shadow `%=`;
  // ledgered. Alphanumerics and whitespace never delimit.
  if (delim == 0 || delim == '=' || is_space(delim) || delim == '\n' ||
      is_ident_char(delim) || delim > 127) {
    return false;
  }
  if (!literal_start_valid(valid, kind)) return false;
  int32_t close = close_for(delim);
  int32_t open = close ? delim : 0;
  if (!close) close = delim;
  advance(lexer);
  return push_literal(s, lexer, kind, open, close, interpolates);
}

// Content and end for the literal on top of the stack. Nothing here skips:
// spaces are content (except between the words of a %w array, which the
// grammar never sees).
static bool scan_literal_body(Scanner *s, TSLexer *lexer, const bool *valid) {
  Literal *l = &s->literals[s->literal_count - 1];
  bool words = l->kind == LIT_WORDS || l->kind == LIT_SYMBOLS;
  bool has_content = false;

  // In a word array, inter-word whitespace (newlines included) is trivia;
  // skipping it here keeps each word one content token.
  if (words) {
    while (is_space(lexer->lookahead) || lexer->lookahead == '\n') {
      skip(lexer);
    }
  }

  if (l->interpolates && lexer->lookahead == '\\') {
    if (!valid[ESCAPE_SEQUENCE]) return false;
    return scan_escape(lexer);
  }

  lexer->mark_end(lexer);
  for (;;) {
    int32_t c = lexer->lookahead;
    if (c == 0) break; // unterminated: emit what exists, else fail
    if (c == '\\') {
      if (l->interpolates) break; // the escape token takes over
      // Raw literal: a backslash shields exactly the next character —
      // which is how `'it\'s'` stays open — and both stay content.
      advance(lexer);
      if (lexer->lookahead != 0) advance(lexer);
      lexer->mark_end(lexer);
      has_content = true;
      continue;
    }
    if (l->interpolates && c == '#') {
      advance(lexer);
      if (lexer->lookahead == '{') {
        // Content ends before the `#`; the grammar's token.immediate
        // `#{` opens the interpolation.
        if (has_content && valid[STRING_CONTENT]) {
          lexer->result_symbol = STRING_CONTENT;
          return true;
        }
        return false;
      }
      lexer->mark_end(lexer);
      has_content = true;
      continue;
    }
    if (l->open != 0 && c == l->open) {
      l->nesting++;
      advance(lexer);
      lexer->mark_end(lexer);
      has_content = true;
      continue;
    }
    if (c == l->close) {
      if (l->nesting > 0) {
        l->nesting--;
        advance(lexer);
        lexer->mark_end(lexer);
        has_content = true;
        continue;
      }
      if (has_content) {
        if (!valid[STRING_CONTENT]) return false;
        lexer->result_symbol = STRING_CONTENT;
        return true;
      }
      if (!valid[STRING_END]) return false;
      advance(lexer);
      if (l->kind == LIT_REGEX) {
        // Trailing flags belong to the literal: /x/im
        while (lexer->lookahead >= 'a' && lexer->lookahead <= 'z') {
          advance(lexer);
        }
      }
      lexer->mark_end(lexer);
      s->literal_count--;
      lexer->result_symbol = STRING_END;
      return true;
    }
    if (words && (is_space(c) || c == '\n')) {
      break; // one word ends; the next call skips the separator
    }
    advance(lexer);
    lexer->mark_end(lexer);
    has_content = true;
  }

  if (has_content && valid[STRING_CONTENT]) {
    lexer->result_symbol = STRING_CONTENT;
    return true;
  }
  return false;
}

// ── heredocs ─────────────────────────────────────────────────────────────

// `<<~EOS`, `<<-EOS`, `<<'EOS'`, `<<EOS`. The bare form requires an
// uppercase (or underscore) first letter: `a <<b` is a shift whose right
// operand is b in almost all real code, and the lexical state ruby uses to
// know better is not visible from here. Ledgered.
static bool scan_heredoc_beginning(Scanner *s, TSLexer *lexer) {
  if (s->heredoc_count >= MAX_HEREDOCS) return false;
  advance(lexer);
  if (lexer->lookahead != '<') return false;
  advance(lexer);

  bool indent_close = false;
  bool require_upper = true;
  if (lexer->lookahead == '~' || lexer->lookahead == '-') {
    indent_close = true;
    require_upper = false;
    advance(lexer);
  }

  int32_t quote = 0;
  bool interpolates = true;
  if (lexer->lookahead == '\'' || lexer->lookahead == '"' ||
      lexer->lookahead == '`') {
    quote = lexer->lookahead;
    interpolates = quote != '\'';
    advance(lexer);
  }

  char id[MAX_HEREDOC_ID];
  uint8_t len = 0;
  if (quote) {
    while (lexer->lookahead != quote) {
      if (lexer->lookahead == 0 || lexer->lookahead == '\n' ||
          len >= MAX_HEREDOC_ID) {
        return false;
      }
      id[len++] = (char)lexer->lookahead;
      advance(lexer);
    }
    if (len == 0) return false;
    advance(lexer); // closing quote
  } else {
    int32_t first = lexer->lookahead;
    if (!is_ident_start(first)) return false;
    if (require_upper && !((first >= 'A' && first <= 'Z') || first == '_')) {
      return false;
    }
    while (is_ident_char(lexer->lookahead) && len < MAX_HEREDOC_ID) {
      id[len++] = (char)lexer->lookahead;
      advance(lexer);
    }
    if (is_ident_char(lexer->lookahead)) return false; // identifier too long
  }

  Heredoc *h = &s->heredocs[s->heredoc_count++];
  h->len = len;
  memcpy(h->id, id, len);
  h->indent_close = indent_close;
  h->interpolates = interpolates;
  h->started = false;
  h->at_line_start = false;
  lexer->mark_end(lexer);
  lexer->result_symbol = HEREDOC_BEGINNING;
  return true;
}

// At a line start inside a heredoc body: does this line close it?
// Consumes what it matches; the caller relies on discard-on-false.
static bool heredoc_closer_here(Heredoc *h, TSLexer *lexer) {
  if (h->indent_close) {
    while (lexer->lookahead == ' ' || lexer->lookahead == '\t') {
      advance(lexer);
    }
  }
  for (uint8_t i = 0; i < h->len; i++) {
    if (lexer->lookahead != h->id[i]) return false;
    advance(lexer);
  }
  if (is_ident_char(lexer->lookahead)) return false;
  while (lexer->lookahead == ' ' || lexer->lookahead == '\t') {
    advance(lexer);
  }
  return lexer->lookahead == '\n' || lexer->lookahead == '\r' ||
         lexer->lookahead == 0;
}

static bool scan_heredoc_body(Scanner *s, TSLexer *lexer, const bool *valid) {
  Heredoc *h = &s->heredocs[0];

  if (h->at_line_start && heredoc_closer_here(h, lexer)) {
    if (!valid[HEREDOC_END]) return false;
    lexer->mark_end(lexer);
    memmove(&s->heredocs[0], &s->heredocs[1],
            (size_t)(s->heredoc_count - 1) * sizeof(Heredoc));
    s->heredoc_count--;
    lexer->result_symbol = HEREDOC_END;
    return true;
  }
  // Not the closer: whatever was consumed while checking is body content,
  // but content is scanned below from wherever the check stopped — every
  // character it passed over is an ordinary word or blank, so nothing that
  // needs a boundary (`#{`, `\`, a newline) was crossed.

  if (h->interpolates && lexer->lookahead == '\\') {
    if (!valid[ESCAPE_SEQUENCE]) return false;
    h->at_line_start = false;
    return scan_escape(lexer);
  }

  bool has_content = false;
  lexer->mark_end(lexer);
  for (;;) {
    int32_t c = lexer->lookahead;
    if (c == 0) break;
    if (h->interpolates && c == '\\') {
      h->at_line_start = false;
      break;
    }
    if (h->interpolates && c == '#') {
      advance(lexer);
      if (lexer->lookahead == '{') {
        if (has_content && valid[HEREDOC_CONTENT]) {
          h->at_line_start = false;
          lexer->result_symbol = HEREDOC_CONTENT;
          return true;
        }
        h->at_line_start = false;
        return false;
      }
      lexer->mark_end(lexer);
      has_content = true;
      continue;
    }
    if (c == '\n') {
      advance(lexer);
      lexer->mark_end(lexer);
      has_content = true;
      if (heredoc_closer_here(h, lexer)) {
        h->at_line_start = true;
        lexer->result_symbol = HEREDOC_CONTENT;
        return valid[HEREDOC_CONTENT];
      }
      continue;
    }
    advance(lexer);
    lexer->mark_end(lexer);
    has_content = true;
  }

  if (has_content && valid[HEREDOC_CONTENT]) {
    lexer->result_symbol = HEREDOC_CONTENT;
    return true;
  }
  return false;
}

// ── simple symbols ───────────────────────────────────────────────────────

// `:foo`, `:foo?`, `:foo=`, `:@ivar`, `:$gvar`, `:+`, `:[]=`. The `=`
// suffix is the reason this is scanner code: it belongs to the symbol
// unless it opens `=>`, `==` or `=~`, which one character of lookahead
// settles and no token regex can.
static bool scan_simple_symbol(TSLexer *lexer) {
  // Called with the ':' already consumed.
  int32_t c = lexer->lookahead;

  if (c == '@' || c == '$') {
    advance(lexer);
    if (c == '@' && lexer->lookahead == '@') advance(lexer);
    if (c == '$' && lexer->lookahead >= '0' && lexer->lookahead <= '9') {
      while (lexer->lookahead >= '0' && lexer->lookahead <= '9') {
        advance(lexer); // :$0, :$1 …
      }
    } else {
      if (!is_ident_start(lexer->lookahead)) return false;
      while (is_ident_char(lexer->lookahead)) advance(lexer);
    }
    lexer->mark_end(lexer);
    lexer->result_symbol = SIMPLE_SYMBOL;
    return true;
  }

  if (is_ident_start(c)) {
    while (is_ident_char(lexer->lookahead)) advance(lexer);
    if (lexer->lookahead == '?') {
      advance(lexer);
    } else if (lexer->lookahead == '!') {
      lexer->mark_end(lexer);
      advance(lexer);
      if (lexer->lookahead == '=') {
        // `:a!=b` never occurs, but symmetry with identifiers is free.
        lexer->result_symbol = SIMPLE_SYMBOL;
        return true;
      }
    } else if (lexer->lookahead == '=') {
      lexer->mark_end(lexer);
      advance(lexer);
      if (lexer->lookahead == '>' || lexer->lookahead == '=' ||
          lexer->lookahead == '~') {
        lexer->result_symbol = SIMPLE_SYMBOL; // :key=>… — the rocket's
        return true;                          // `=` is not ours
      }
    }
    lexer->mark_end(lexer);
    lexer->result_symbol = SIMPLE_SYMBOL;
    return true;
  }

  // Operator symbols, decided character by character because the lexer
  // cannot rewind a failed longer candidate.
  switch (c) {
    case '[':
      advance(lexer);
      if (lexer->lookahead != ']') return false;
      advance(lexer);
      if (lexer->lookahead == '=') advance(lexer);
      break;
    case '<':
      advance(lexer);
      if (lexer->lookahead == '=') {
        advance(lexer);
        if (lexer->lookahead == '>') advance(lexer);
      } else if (lexer->lookahead == '<') {
        advance(lexer);
      }
      break;
    case '>':
      advance(lexer);
      if (lexer->lookahead == '=' || lexer->lookahead == '>') advance(lexer);
      break;
    case '=':
      advance(lexer);
      if (lexer->lookahead == '=') {
        advance(lexer);
        if (lexer->lookahead == '=') advance(lexer);
      } else if (lexer->lookahead == '~') {
        advance(lexer);
      } else {
        return false;
      }
      break;
    case '!':
      advance(lexer);
      if (lexer->lookahead == '=' || lexer->lookahead == '~') advance(lexer);
      break;
    case '+':
    case '-':
      advance(lexer);
      if (lexer->lookahead == '@') advance(lexer);
      break;
    case '*':
      advance(lexer);
      if (lexer->lookahead == '*') advance(lexer);
      break;
    case '/': case '%': case '^': case '&': case '|': case '~': case '`':
      advance(lexer);
      break;
    default:
      return false;
  }
  lexer->mark_end(lexer);
  lexer->result_symbol = SIMPLE_SYMBOL;
  return true;
}

// ── block comments ───────────────────────────────────────────────────────

// `=begin` … `=end`, both anchored at column 0 — which is why this cannot
// be a token regex. Runs through the `=end` line's end.
static bool scan_block_comment(TSLexer *lexer) {
  static const char open[] = "=begin";
  for (int i = 0; open[i]; i++) {
    if (lexer->lookahead != open[i]) return false;
    advance(lexer);
  }
  if (lexer->lookahead != ' ' && lexer->lookahead != '\t' &&
      lexer->lookahead != '\n' && lexer->lookahead != '\r' &&
      lexer->lookahead != 0) {
    return false;
  }
  for (;;) {
    // To the end of the current line, then test the next for `=end`.
    while (lexer->lookahead != 0 && lexer->lookahead != '\n') {
      advance(lexer);
    }
    if (lexer->lookahead == 0) break; // unterminated: everything is comment
    advance(lexer);
    static const char close[] = "=end";
    int i = 0;
    while (close[i] && lexer->lookahead == close[i]) {
      advance(lexer);
      i++;
    }
    if (close[i] == 0 &&
        (lexer->lookahead == ' ' || lexer->lookahead == '\t' ||
         lexer->lookahead == '\n' || lexer->lookahead == '\r' ||
         lexer->lookahead == 0)) {
      while (lexer->lookahead != 0 && lexer->lookahead != '\n') {
        advance(lexer);
      }
      break;
    }
  }
  lexer->mark_end(lexer);
  lexer->result_symbol = BLOCK_COMMENT;
  return true;
}

// ── spacing-sensitive operators ──────────────────────────────────────────

// The shared shape: when both readings are valid, whitespace before but not
// after says PREFIX (splat, unary, block-pass, regex); anything else says
// binary. When only one reading is valid, validity decides and spacing is
// ignored — `def f(*a)` has no left operand and needs no heuristic.
static bool emit(TSLexer *lexer, uint16_t symbol) {
  lexer->mark_end(lexer);
  lexer->result_symbol = symbol;
  return true;
}

static bool scan_op_pair(TSLexer *lexer, const bool *valid, bool ws_before,
                         uint16_t binary, uint16_t prefix) {
  bool prefix_reading = valid[prefix] &&
      (!valid[binary] ||
       (ws_before && !is_space(lexer->lookahead) && lexer->lookahead != '\n' &&
        lexer->lookahead != '=' && lexer->lookahead != 0));
  if (prefix_reading) return emit(lexer, prefix);
  if (valid[binary]) return emit(lexer, binary);
  return false;
}

// ── the entry point ──────────────────────────────────────────────────────

bool tree_sitter_ruby_external_scanner_scan(void *payload, TSLexer *lexer,
                                            const bool *valid) {
  Scanner *s = (Scanner *)payload;
  bool error_recovery = valid[ERROR_SENTINEL];

  if (error_recovery) {
    // The parser is recovering; literal state may describe a string that no
    // longer exists. Drop it, and offer nothing but statement boundaries.
    s->literal_count = 0;
    s->heredoc_count = 0;
    if (lexer->lookahead == '\n' && valid[LINE_BREAK]) {
      advance(lexer);
      return emit(lexer, LINE_BREAK);
    }
    return false;
  }

  // `foo?` / `foo!` — one token with the identifier, but only when glued to
  // it (this runs before any whitespace is skipped) and only when not
  // really `!=` (`foo!=bar` is a comparison; `foo!==bar` is a call compared
  // with `=b`... no — it is `foo! == bar`, which the second peek admits).
  if (valid[IDENTIFIER_SUFFIX] &&
      (lexer->lookahead == '?' || lexer->lookahead == '!')) {
    advance(lexer);
    lexer->mark_end(lexer);
    if (lexer->lookahead == '=') {
      advance(lexer);
      if (lexer->lookahead != '=') return false;
    }
    lexer->result_symbol = IDENTIFIER_SUFFIX;
    return true;
  }

  // Inside a delimited literal, content outranks everything and nothing may
  // be skipped: spaces are content.
  if (s->literal_count > 0 &&
      (valid[STRING_CONTENT] || valid[STRING_END] || valid[ESCAPE_SEQUENCE])) {
    return scan_literal_body(s, lexer, valid);
  }

  // Inside a heredoc body.
  if (s->heredoc_count > 0 && s->heredocs[0].started &&
      (valid[HEREDOC_CONTENT] || valid[HEREDOC_END] || valid[ESCAPE_SEQUENCE])) {
    return scan_heredoc_body(s, lexer, valid);
  }

  // One unified skip pass: horizontal space, escaped newlines, and the
  // newline decision itself.
  bool ws_before = lexer->get_column(lexer) == 0;
  for (;;) {
    int32_t c = lexer->lookahead;
    if (is_space(c)) {
      ws_before = true;
      skip(lexer);
      continue;
    }
    if (c == '\\') {
      // Only a line continuation is trivia; any other backslash belongs to
      // the internal lexer (where it is an error outside a literal).
      lexer->mark_end(lexer);
      advance(lexer);
      if (lexer->lookahead == '\r') advance(lexer);
      if (lexer->lookahead != '\n') return false;
      advance(lexer);
      ws_before = true;
      continue;
    }
    if (c == '\n') {
      // A pending heredoc claims the line end: its body starts here.
      if (s->heredoc_count > 0 && !s->heredocs[0].started &&
          valid[HEREDOC_BODY_START]) {
        advance(lexer);
        s->heredocs[0].started = true;
        s->heredocs[0].at_line_start = true;
        return emit(lexer, HEREDOC_BODY_START);
      }
      if (valid[LINE_BREAK]) {
        advance(lexer);
        lexer->mark_end(lexer);
        // A line whose first token is `.` or `&.` continues the previous
        // statement (a method chain written leading-dot style), so this
        // newline is whitespace, not a terminator. Blank and comment lines
        // between do not break the chain, so look through them.
        for (;;) {
          int32_t p = lexer->lookahead;
          if (is_space(p) || p == '\n') {
            advance(lexer);
            continue;
          }
          if (p == '#') {
            while (lexer->lookahead != 0 && lexer->lookahead != '\n') {
              advance(lexer);
            }
            continue;
          }
          break;
        }
        if (lexer->lookahead == '.') {
          advance(lexer);
          if (lexer->lookahead != '.') return false; // `.foo` continues
        } else if (lexer->lookahead == '&') {
          advance(lexer);
          if (lexer->lookahead == '.') return false; // `&.foo` continues
        }
        lexer->result_symbol = LINE_BREAK;
        return true;
      }
      // Mid-expression newline: plain whitespace.
      ws_before = true;
      skip(lexer);
      continue;
    }
    break;
  }

  int32_t c = lexer->lookahead;
  switch (c) {
    case '=':
      if (valid[BLOCK_COMMENT] && lexer->get_column(lexer) == 0) {
        return scan_block_comment(lexer);
      }
      return false;

    case '<':
      if (valid[HEREDOC_BEGINNING]) return scan_heredoc_beginning(s, lexer);
      return false;

    case '"':
      if (!valid[STRING_START]) return false;
      advance(lexer);
      return push_literal(s, lexer, LIT_STRING, 0, '"', true);

    case '\'':
      if (!valid[STRING_START]) return false;
      advance(lexer);
      return push_literal(s, lexer, LIT_STRING, 0, '\'', false);

    case '`':
      if (!valid[SUBSHELL_START]) return false;
      advance(lexer);
      return push_literal(s, lexer, LIT_SUBSHELL, 0, '`', true);

    case ':':
      if (!valid[SYMBOL_START] && !valid[SIMPLE_SYMBOL]) return false;
      advance(lexer);
      if (lexer->lookahead == ':') return false; // scope resolution
      if (valid[SYMBOL_START] && lexer->lookahead == '"') {
        advance(lexer);
        return push_literal(s, lexer, LIT_SYMBOL, 0, '"', true);
      }
      if (valid[SYMBOL_START] && lexer->lookahead == '\'') {
        advance(lexer);
        return push_literal(s, lexer, LIT_SYMBOL, 0, '\'', false);
      }
      if (valid[SIMPLE_SYMBOL]) return scan_simple_symbol(lexer);
      return false; // `::`, ternary `:` — internal tokens

    case '%':
      if (valid[STRING_START] || valid[WORDS_START] || valid[SYMBOLS_START] ||
          valid[REGEX_START] || valid[SYMBOL_START] || valid[SUBSHELL_START]) {
        return scan_percent_literal(s, lexer, valid);
      }
      return false;

    case '/':
      if (valid[REGEX_START] && !valid[BINARY_SLASH]) {
        advance(lexer);
        return push_literal(s, lexer, LIT_REGEX, 0, '/', true);
      }
      if (valid[REGEX_START] || valid[BINARY_SLASH]) {
        advance(lexer);
        int32_t after = lexer->lookahead;
        bool regex = valid[REGEX_START] && ws_before && !is_space(after) &&
                     after != '=' && after != '\n' && after != 0;
        if (regex) return push_literal(s, lexer, LIT_REGEX, 0, '/', true);
        if (!valid[BINARY_SLASH]) return false;
        if (after == '=') return false; // `/=` is the operator
        return emit(lexer, BINARY_SLASH);
      }
      return false;

    case '*':
      if (!(valid[BINARY_STAR] || valid[SPLAT_STAR] ||
            valid[BINARY_STAR_STAR] || valid[SPLAT_STAR_STAR])) {
        return false;
      }
      advance(lexer);
      if (lexer->lookahead == '*') {
        advance(lexer);
        if (lexer->lookahead == '=') return false; // **=
        return scan_op_pair(lexer, valid, ws_before,
                            BINARY_STAR_STAR, SPLAT_STAR_STAR);
      }
      if (lexer->lookahead == '=') return false; // *=
      return scan_op_pair(lexer, valid, ws_before, BINARY_STAR, SPLAT_STAR);

    case '&':
      if (!(valid[BINARY_AMP] || valid[BLOCK_AMP])) return false;
      advance(lexer);
      if (lexer->lookahead == '&' || lexer->lookahead == '.' ||
          lexer->lookahead == '=') {
        return false; // && &. &= — internal tokens
      }
      return scan_op_pair(lexer, valid, ws_before, BINARY_AMP, BLOCK_AMP);

    case '-':
      if (!(valid[BINARY_MINUS] || valid[UNARY_MINUS])) return false;
      advance(lexer);
      if (lexer->lookahead == '=' || lexer->lookahead == '>') {
        return false; // -= and the -> of a lambda
      }
      return scan_op_pair(lexer, valid, ws_before, BINARY_MINUS, UNARY_MINUS);

    case '+':
      if (!(valid[BINARY_PLUS] || valid[UNARY_PLUS])) return false;
      advance(lexer);
      if (lexer->lookahead == '=') return false; // +=
      return scan_op_pair(lexer, valid, ws_before, BINARY_PLUS, UNARY_PLUS);

    default:
      // `key:` — an identifier-shaped run glued to a single colon. Two
      // colons are a scope resolution, which is the case a plain token
      // could never separate without lookahead.
      if (valid[HASH_KEY] && is_ident_start(c)) {
        while (is_ident_char(lexer->lookahead)) advance(lexer);
        if (lexer->lookahead == '?' || lexer->lookahead == '!') advance(lexer);
        if (lexer->lookahead != ':') return false;
        advance(lexer);
        if (lexer->lookahead == ':') return false; // Foo::Bar
        return emit(lexer, HASH_KEY);
      }
      return false;
  }
}

// ── state round-trip ─────────────────────────────────────────────────────

unsigned tree_sitter_ruby_external_scanner_serialize(void *payload,
                                                     char *buffer) {
  Scanner *s = (Scanner *)payload;
  unsigned i = 0;
  buffer[i++] = (char)s->literal_count;
  for (uint8_t k = 0; k < s->literal_count; k++) {
    Literal *l = &s->literals[k];
    buffer[i++] = (char)l->kind;
    buffer[i++] = (char)l->open;
    buffer[i++] = (char)l->close;
    buffer[i++] = (char)l->nesting;
    buffer[i++] = (char)l->interpolates;
  }
  buffer[i++] = (char)s->heredoc_count;
  for (uint8_t k = 0; k < s->heredoc_count; k++) {
    Heredoc *h = &s->heredocs[k];
    buffer[i++] = (char)h->len;
    buffer[i++] = (char)((h->indent_close ? 1 : 0) | (h->interpolates ? 2 : 0) |
                         (h->started ? 4 : 0) | (h->at_line_start ? 8 : 0));
    memcpy(&buffer[i], h->id, h->len);
    i += h->len;
  }
  return i;
}

void tree_sitter_ruby_external_scanner_deserialize(void *payload,
                                                   const char *buffer,
                                                   unsigned length) {
  Scanner *s = (Scanner *)payload;
  memset(s, 0, sizeof(Scanner));
  if (length == 0) return;
  unsigned i = 0;
  s->literal_count = (uint8_t)buffer[i++];
  for (uint8_t k = 0; k < s->literal_count; k++) {
    Literal *l = &s->literals[k];
    l->kind = (uint8_t)buffer[i++];
    l->open = (uint8_t)buffer[i++];
    l->close = (uint8_t)buffer[i++];
    l->nesting = (uint8_t)buffer[i++];
    l->interpolates = buffer[i++] != 0;
  }
  s->heredoc_count = (uint8_t)buffer[i++];
  for (uint8_t k = 0; k < s->heredoc_count; k++) {
    Heredoc *h = &s->heredocs[k];
    h->len = (uint8_t)buffer[i++];
    uint8_t flags = (uint8_t)buffer[i++];
    h->indent_close = (flags & 1) != 0;
    h->interpolates = (flags & 2) != 0;
    h->started = (flags & 4) != 0;
    h->at_line_start = (flags & 8) != 0;
    memcpy(h->id, &buffer[i], h->len);
    i += h->len;
  }
}

void *tree_sitter_ruby_external_scanner_create(void) {
  return calloc(1, sizeof(Scanner));
}

void tree_sitter_ruby_external_scanner_destroy(void *payload) {
  free(payload);
}
