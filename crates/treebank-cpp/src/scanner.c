// C++'s external scanner: the raw string literal, and nothing else.
//
// `R"delim(...)delim"` is the one C++ token that no regular expression can
// match, because its terminator is chosen by the program: the delimiter is
// whatever appears between the `"` and the `(`, and the literal ends at the
// first `)` followed by that same text and a `"`. A token has to REMEMBER
// what it read at its own start in order to know where it stops, which is
// exactly the thing a DFA cannot do.
//
// Everything else C++ lexes is regular and stays in the parse table. In
// particular the scanner deliberately does NOT try to disambiguate `<`:
// whether `a < b > c` is a comparison or a template instantiation is a
// PARSING question, not a lexical one, and answering it here would need the
// symbol table the scanner does not have. It is left to the parse table,
// where the ambiguity is declared and the two readings are both carried.
//
// There is no state to carry across a token boundary, so serialize and
// deserialize are empty — an incremental reparse resumes with nothing to
// restore.

#include "tree_sitter/parser.h"

enum TokenType {
  RAW_STRING_LITERAL,
};

void *tree_sitter_cpp_external_scanner_create(void) { return NULL; }
void tree_sitter_cpp_external_scanner_destroy(void *payload) { (void)payload; }

unsigned tree_sitter_cpp_external_scanner_serialize(void *payload, char *buffer) {
  (void)payload;
  (void)buffer;
  return 0;
}

void tree_sitter_cpp_external_scanner_deserialize(void *payload, const char *buffer,
                                                  unsigned length) {
  (void)payload;
  (void)buffer;
  (void)length;
}

// The longest delimiter C++ allows is 16 characters (C++20 [lex.string]).
// A source that exceeds it is not C++, and stopping here rather than
// growing a buffer means the scanner has no allocation to get wrong.
#define MAX_DELIMITER 16

bool tree_sitter_cpp_external_scanner_scan(void *payload, TSLexer *lexer,
                                           const bool *valid_symbols) {
  (void)payload;
  if (!valid_symbols[RAW_STRING_LITERAL]) return false;

  // Skip the whitespace the parser would otherwise have skipped for us.
  while (lexer->lookahead == ' ' || lexer->lookahead == '\t' ||
         lexer->lookahead == '\n' || lexer->lookahead == '\r') {
    lexer->advance(lexer, true);
  }

  // An encoding prefix may precede the R: `u8R"(…)"`, `LR"(…)"`.
  if (lexer->lookahead == 'L' || lexer->lookahead == 'U') {
    lexer->advance(lexer, false);
  } else if (lexer->lookahead == 'u') {
    lexer->advance(lexer, false);
    if (lexer->lookahead == '8') lexer->advance(lexer, false);
  }

  if (lexer->lookahead != 'R') return false;
  lexer->advance(lexer, false);
  if (lexer->lookahead != '"') return false;
  lexer->advance(lexer, false);

  char delimiter[MAX_DELIMITER];
  unsigned length = 0;
  while (lexer->lookahead != '(') {
    // A delimiter may not contain a space, a backslash, a paren or a quote,
    // and running off the end of the file is not a raw string either.
    if (lexer->lookahead == 0 || lexer->lookahead == ' ' ||
        lexer->lookahead == '\\' || lexer->lookahead == ')' ||
        lexer->lookahead == '"' || length == MAX_DELIMITER) {
      return false;
    }
    delimiter[length++] = (char)lexer->lookahead;
    lexer->advance(lexer, false);
  }
  lexer->advance(lexer, false);  // the '('

  // Now read until `)` + delimiter + `"`. A `)` that does not begin the
  // terminator is ordinary content, so the scan resumes from the character
  // after it rather than treating it as an end — `)x)delim"` inside a
  // `delim` string is real.
  for (;;) {
    if (lexer->lookahead == 0) return false;
    if (lexer->lookahead == ')') {
      lexer->advance(lexer, false);
      unsigned i = 0;
      while (i < length && lexer->lookahead == (unsigned char)delimiter[i]) {
        lexer->advance(lexer, false);
        i++;
      }
      if (i == length && lexer->lookahead == '"') {
        lexer->advance(lexer, false);
        lexer->result_symbol = RAW_STRING_LITERAL;
        return true;
      }
      // Not the terminator: whatever was consumed is content, and the
      // character now in front is examined afresh.
      continue;
    }
    lexer->advance(lexer, false);
  }
}
