// treebank-rust external scanner.
//
// Three tokens, each one thing internal lexing cannot do:
//
// 1. float — `1.5` is a float, `1..2` is an integer and a range, `1.max(2)`
//    is an integer and a method call. The dot's meaning needs one character
//    of lookahead past it, so the whole float is lexed here. Running before
//    the internal lexer, an emitted float beats the internal integer.
// 2. raw_string — r"..." / r#"..."# / br##"..."## with any number of
//    hashes; the closing fence must match the opening one.
// 3. block_comment — /* ... */ nests in Rust.
//
// The scanner skips leading whitespace itself: the external scanner is
// consulted once per token request, BEFORE the internal lexer touches
// extras — a lesson the python scanner taught three times over.

#include "tree_sitter/parser.h"

enum TokenType {
  FLOAT,
  RAW_STRING,
  BLOCK_COMMENT,
};

static inline void advance(TSLexer *lexer) { lexer->advance(lexer, false); }
static inline void skip_ws(TSLexer *lexer) {
  for (;;) {
    int32_t c = lexer->lookahead;
    if (c == ' ' || c == '\t' || c == '\n' || c == '\r' || c == '\f') {
      lexer->advance(lexer, true);
    } else {
      break;
    }
  }
}

static bool is_digit(int32_t c) { return c >= '0' && c <= '9'; }
static bool is_ident(int32_t c) {
  return (c >= 'a' && c <= 'z') || (c >= 'A' && c <= 'Z') ||
         (c >= '0' && c <= '9') || c == '_' || c > 127;
}

static void eat_digits(TSLexer *lexer) {
  while (is_digit(lexer->lookahead) || lexer->lookahead == '_') advance(lexer);
}

// `f32`/`f64` suffix, consumed blind: by the time we are here the token is
// already a float, and `1.5f32x` is not valid Rust anyway.
static void eat_float_suffix(TSLexer *lexer) {
  if (lexer->lookahead == 'f') {
    advance(lexer);
    if (lexer->lookahead == '3' || lexer->lookahead == '6') {
      advance(lexer);
      if (lexer->lookahead == '2' || lexer->lookahead == '4') advance(lexer);
    }
  }
}

static bool scan_float(TSLexer *lexer) {
  if (!is_digit(lexer->lookahead)) return false;
  // 0x / 0o / 0b are never floats.
  if (lexer->lookahead == '0') {
    advance(lexer);
    if (lexer->lookahead == 'x' || lexer->lookahead == 'o' ||
        lexer->lookahead == 'b') {
      return false;
    }
  }
  eat_digits(lexer);

  bool is_float = false;
  if (lexer->lookahead == '.') {
    lexer->mark_end(lexer); // integer end, in case the dot is not ours
    advance(lexer);
    if (is_digit(lexer->lookahead)) {
      eat_digits(lexer);          // 1.5
      is_float = true;
    } else if (lexer->lookahead == '.' || lexer->lookahead == '_' ||
               is_ident(lexer->lookahead)) {
      return false;               // 1..2 range, 1.max() method
    } else {
      is_float = true;            // trailing dot: `1.`
    }
  }

  if (lexer->lookahead == 'e' || lexer->lookahead == 'E') {
    lexer->mark_end(lexer);
    advance(lexer);
    if (lexer->lookahead == '+' || lexer->lookahead == '-') advance(lexer);
    if (is_digit(lexer->lookahead)) {
      eat_digits(lexer);          // 1e3 / 1.5e-3
      is_float = true;
    } else {
      // `1e` alone: an integer then an identifier; hand back what we had.
      lexer->result_symbol = FLOAT;
      return is_float;
    }
  }

  if (!is_float && lexer->lookahead == 'f') {
    // 1f32 — float by suffix alone.
    lexer->mark_end(lexer);
    advance(lexer);
    int32_t a = lexer->lookahead;
    if (a == '3' || a == '6') {
      advance(lexer);
      int32_t b = lexer->lookahead;
      if ((a == '3' && b == '2') || (a == '6' && b == '4')) {
        advance(lexer);
        if (!is_ident(lexer->lookahead)) {
          lexer->mark_end(lexer);
          lexer->result_symbol = FLOAT;
          return true;
        }
      }
    }
    return false;
  }

  if (!is_float) return false;
  eat_float_suffix(lexer);
  lexer->mark_end(lexer);
  lexer->result_symbol = FLOAT;
  return true;
}

static bool scan_raw_string(TSLexer *lexer) {
  // [bc]?r#*" — byte raw br"", C-string raw cr"" (1.77), plain r"".
  if (lexer->lookahead == 'b' || lexer->lookahead == 'c') advance(lexer);
  if (lexer->lookahead != 'r') return false;
  advance(lexer);

  unsigned hashes = 0;
  while (lexer->lookahead == '#') {
    hashes++;
    advance(lexer);
  }
  if (lexer->lookahead != '"') return false;
  advance(lexer);

  for (;;) {
    if (lexer->lookahead == 0) return false; // unterminated
    if (lexer->lookahead == '"') {
      advance(lexer);
      unsigned seen = 0;
      while (seen < hashes && lexer->lookahead == '#') {
        seen++;
        advance(lexer);
      }
      if (seen == hashes) {
        lexer->mark_end(lexer);
        lexer->result_symbol = RAW_STRING;
        return true;
      }
    } else {
      advance(lexer);
    }
  }
}

static bool scan_block_comment(TSLexer *lexer) {
  if (lexer->lookahead != '/') return false;
  advance(lexer);
  if (lexer->lookahead != '*') return false;
  advance(lexer);

  unsigned depth = 1;
  while (depth > 0) {
    switch (lexer->lookahead) {
      case 0:
        return false; // unterminated
      case '/':
        advance(lexer);
        if (lexer->lookahead == '*') {
          advance(lexer);
          depth++;
        }
        break;
      case '*':
        advance(lexer);
        if (lexer->lookahead == '/') {
          advance(lexer);
          depth--;
        }
        break;
      default:
        advance(lexer);
    }
  }
  lexer->mark_end(lexer);
  lexer->result_symbol = BLOCK_COMMENT;
  return true;
}

bool tree_sitter_rust_external_scanner_scan(void *payload, TSLexer *lexer,
                                            const bool *valid) {
  (void)payload;
  skip_ws(lexer);
  int32_t c = lexer->lookahead;
  if (valid[BLOCK_COMMENT] && c == '/') {
    return scan_block_comment(lexer);
  }
  if (valid[RAW_STRING] && (c == 'r' || c == 'b' || c == 'c')) {
    return scan_raw_string(lexer);
  }
  if (valid[FLOAT] && is_digit(c)) {
    return scan_float(lexer);
  }
  return false;
}

unsigned tree_sitter_rust_external_scanner_serialize(void *payload, char *buffer) {
  (void)payload;
  (void)buffer;
  return 0;
}

void tree_sitter_rust_external_scanner_deserialize(void *payload,
                                                   const char *buffer,
                                                   unsigned length) {
  (void)payload;
  (void)buffer;
  (void)length;
}

void *tree_sitter_rust_external_scanner_create(void) { return NULL; }
void tree_sitter_rust_external_scanner_destroy(void *payload) { (void)payload; }
