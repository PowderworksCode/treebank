// treebank-typescript external scanner: automatic semicolon insertion.
//
// One zero-width token. Emitted where the grammar offers it AND the source
// justifies it: after a real line break (or before `}` / at EOF), unless
// the next token is one that legally CONTINUES the previous expression —
// because the token stream is shared across GLR forks, emitting here would
// kill the continuation fork that JavaScript's ASI rules say must win.
//
// The continuation set is the flip side of the famous ASI hazards: a line
// starting with ( [ ` + - * / % < > = & | ^ ? : , . or a binary keyword
// (in, instanceof) does NOT get a semicolon in front of it. `++`/`--` are
// the exception inside that set: they are restricted productions and DO
// force insertion.

#include "tree_sitter/parser.h"

#include <string.h>

enum TokenType {
  AUTOMATIC_SEMICOLON,
  TYPE_MEMBER_END,
};

static inline void skip(TSLexer *lexer) { lexer->advance(lexer, true); }

/* Skip whitespace AND comments, setting *crossed when a line terminator is
 * passed -- a block comment containing one counts, which is what the ASI
 * rules say.
 *
 * Comments matter here because both callers decide by looking at the next
 * character, and a comment starts with '/', which is IN both continuation
 * sets. Without this, `clear(): void` followed by a doc comment and then an
 * index signature never got a member boundary: the scanner saw the comment's
 * '/' as division and read `void [Symbol.iterator]` as an indexed access.
 *
 * Returns false for a BARE '/' -- real division or a regex -- which
 * continues the previous construct and is never a boundary.
 */
static bool skip_trivia(TSLexer *lexer, bool *crossed) {
  for (;;) {
    int32_t c = lexer->lookahead;
    if (c == ' ' || c == '\t' || c == '\f' || c == 0x0b) {
      skip(lexer);
      continue;
    }
    if (c == '\n' || c == '\r' || c == 0x2028 || c == 0x2029) {
      *crossed = true;
      skip(lexer);
      continue;
    }
    if (c != '/') return true;
    skip(lexer);
    if (lexer->lookahead == '/') {
      while (lexer->lookahead != 0 && lexer->lookahead != '\n' &&
             lexer->lookahead != '\r') {
        skip(lexer);
      }
      continue;
    }
    if (lexer->lookahead == '*') {
      skip(lexer);
      int32_t prev = 0;
      while (lexer->lookahead != 0) {
        int32_t d = lexer->lookahead;
        if (d == '\n' || d == '\r' || d == 0x2028 || d == 0x2029) *crossed = true;
        skip(lexer);
        if (prev == '*' && d == '/') break;
        prev = d;
      }
      continue;
    }
    return false;
  }
}

/* True when the token ahead continues the type of the member just parsed,
 * so no member boundary belongs here. Mirrors the ASI continuation set
 * EXCEPT for '[': a bracket after an expression is a subscript, but a
 * bracket after a type member is the next member's index signature. */
static bool continues_type_member(TSLexer *lexer) {
  int32_t c = lexer->lookahead;
  switch (c) {
    case '(': case '`':
    case '*': case '%': case '<': case '>': case '=': case '&': case '|':
    case '^': case '?': case ':': case ',': case '.': case ';': case '!':
    case '-': case '+': case '/':
      return true;
    default:
      break;
  }
  /* `extends` after a type continues a conditional type. */
  if (c == 'e') {
    char word[9];
    int n = 0;
    while (n < 8 && lexer->lookahead >= 'a' && lexer->lookahead <= 'z') {
      word[n++] = (char)lexer->lookahead;
      lexer->advance(lexer, false);
    }
    word[n] = 0;
    return strcmp(word, "extends") == 0;
  }
  return false;
}

bool tree_sitter_typescript_external_scanner_scan(void *payload, TSLexer *lexer,
                                                  const bool *valid) {
  (void)payload;

  // A member boundary inside an object type. It needs the SAME continuation
  // set as ASI — a member's type spans lines all the time in .d.ts
  // (`a: A` then `| B`, or a qualified name broken across lines) — with
  // exactly ONE token flipped: `[` continues an expression (`foo\n[0]` is a
  // subscript) and BEGINS a type member (an index signature). That single
  // difference is the whole reason this is a separate token rather than
  // reusing ASI, and dropping the rest of the set is what a first attempt
  // got wrong: it severed every multi-line member type and took the corpus
  // from 22 to 100 gaps.
  if (valid[TYPE_MEMBER_END] && !valid[AUTOMATIC_SEMICOLON]) {
    lexer->mark_end(lexer);
    bool crossed = false;
    if (!skip_trivia(lexer, &crossed)) return false;
    if (!crossed) return false;
    if (!continues_type_member(lexer)) {
      lexer->result_symbol = TYPE_MEMBER_END;
      return true;
    }
    return false;
  }

  if (!valid[AUTOMATIC_SEMICOLON]) return false;

  lexer->mark_end(lexer);

  bool crossed_newline = false;
  if (!skip_trivia(lexer, &crossed_newline)) return false;

  int32_t c = lexer->lookahead;

  // Always a statement end, newline or not.
  if (c == 0) {
    lexer->result_symbol = AUTOMATIC_SEMICOLON;
    return true;
  }
  if (c == '}') {
    lexer->result_symbol = AUTOMATIC_SEMICOLON;
    return true;
  }

  if (!crossed_newline) return false;

  // Comments: decline and let the parser lex the comment as an extra; we
  // are consulted again on the far side with the newline already crossed —
  // which the next call re-detects because comments end in newlines (line
  // comments) or the trailing newline of this group is still ahead.
  if (c == '/') {
    // Peek: `//` or `/*` is a comment — decline so it can be lexed. A lone
    // `/` is division or regex; division continues the line (no ASI).
    lexer->advance(lexer, false);
    int32_t d = lexer->lookahead;
    if (d == '/' || d == '*') return false;
    return false; // division / regex: never insert before '/'
  }

  // ++ / -- are restricted: a newline before them forces insertion.
  if (c == '+' || c == '-') {
    lexer->advance(lexer, false);
    if (lexer->lookahead == c) {
      lexer->result_symbol = AUTOMATIC_SEMICOLON;
      return true;
    }
    return false; // binary + or -: continuation
  }

  // Tokens that continue the previous expression: no insertion.
  switch (c) {
    case '(': case '[': case '`':
    case '*': case '%': case '<': case '>': case '=': case '&': case '|':
    case '^': case '?': case ':': case ',': case '.': case ';': case '!':
      return false;
  }

  // `in` / `instanceof` / `as` / `satisfies` continue too. Word-peek, safe
  // because none of them can begin a statement.
  if (c == 'i' || c == 'a' || c == 's' || c == 'e') {
    static const char *continuations[] = {"in", "instanceof", "as", "satisfies", "extends"};
    char word[11];
    int n = 0;
    while (n < 10 && lexer->lookahead >= 'a' && lexer->lookahead <= 'z') {
      word[n++] = (char)lexer->lookahead;
      lexer->advance(lexer, false);
    }
    word[n] = 0;
    bool ends_word = !(lexer->lookahead == '_' || lexer->lookahead == '$' ||
                       (lexer->lookahead >= 'a' && lexer->lookahead <= 'z') ||
                       (lexer->lookahead >= 'A' && lexer->lookahead <= 'Z') ||
                       (lexer->lookahead >= '0' && lexer->lookahead <= '9'));
    if (ends_word) {
      for (unsigned k = 0; k < 5; k++) {
        if (strcmp(word, continuations[k]) == 0) return false;
      }
    }
  }

  lexer->result_symbol = AUTOMATIC_SEMICOLON;
  return true;
}

unsigned tree_sitter_typescript_external_scanner_serialize(void *payload, char *buffer) {
  (void)payload;
  (void)buffer;
  return 0;
}

void tree_sitter_typescript_external_scanner_deserialize(void *payload,
                                                         const char *buffer,
                                                         unsigned length) {
  (void)payload;
  (void)buffer;
  (void)length;
}

void *tree_sitter_typescript_external_scanner_create(void) { return NULL; }
void tree_sitter_typescript_external_scanner_destroy(void *payload) { (void)payload; }
