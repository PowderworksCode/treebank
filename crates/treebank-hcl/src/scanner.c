// The HCL external scanner: two decisions the parse table cannot make,
// and nothing else.
//
// 1. THE LINE TERMINATOR. HCL is newline sensitive at body level, not
//    inside `(…)`, `[…]`, a call or an index, and sensitive again inside
//    an object. hclsyntax expresses that with a newline-inclusion stack in
//    a recursive-descent parser. Here it is expressed with `valid_symbols`
//    instead: `_newline` is emitted only where the parse table admits one,
//    and everywhere else the newline falls through to `extras` and is
//    trivia. No rule in grammar.js mentions newline-insensitivity, because
//    a rule that never admits `_newline` never gets offered one.
//
// 2. THE TEMPLATE MODE. A quoted template and a heredoc are the same
//    grammar over different literal text, and either can nest inside the
//    other through an interpolation — `"${<<EOT\n…\nEOT\n}"` is valid HCL.
//    So the state is a STACK of modes rather than a heredoc flag, and the
//    quote characters and the heredoc markers are scanner-owned so that
//    the stack stays in step with the parse, and finding a heredoc's
//    delimiter LINE is part of the same job. One owner per spelling
//    (FIELD_GUIDE.md §4): the internal lexer never produces a `"` token,
//    and `string_lit` — the block-label form, which must not open a
//    template — is one whole token that happens to contain quotes.
//
// `${` and `%{` are NOT here, and the reason is worth stating because it
// looks like an omission: the scanner takes template text greedily, so at
// a template-part position that begins with one of them the literal run is
// EMPTY, the scanner declines, and the internal lexer matches the
// introduction. They are valid nowhere else, so nothing else can lex them.
//
// Everything else in HCL is decided by the table or by keyword
// extraction, and is not here.

#include "tree_sitter/parser.h"

#include <stdlib.h>
#include <string.h>

enum TokenType {
  NEWLINE,
  QUOTE_OPEN,
  QUOTE_CLOSE,
  TEMPLATE_LITERAL,
  HEREDOC_OPEN,
  HEREDOC_CLOSE,
  ERROR_SENTINEL,
};

// The serialize buffer is 1 KiB and the state must FIT (FIELD_GUIDE.md
// §8), so both dimensions are bounded and overflow degrades rather than
// allocating: a heredoc nested deeper than this, or with a delimiter
// longer than this, stops being tracked and the file fails to parse. Both
// limits are far past anything hclsyntax's own callers write; the ledger
// records that the failure mode is a rejection, never a mis-parse.
#define MAX_FRAMES 24
#define MAX_DELIMITER 40

typedef enum {
  MODE_QUOTED = 0,
  MODE_HEREDOC = 1,
} ModeKind;

typedef struct {
  ModeKind kind;
  uint8_t length;
  char delimiter[MAX_DELIMITER];
} Frame;

typedef struct {
  uint8_t depth;
  Frame frames[MAX_FRAMES];
} Scanner;

static inline bool is_delimiter_char(int32_t c) {
  return (c >= 'a' && c <= 'z') || (c >= 'A' && c <= 'Z') ||
         (c >= '0' && c <= '9') || c == '_' || c == '-';
}

static inline Frame *top(Scanner *scanner) {
  return scanner->depth > 0 ? &scanner->frames[scanner->depth - 1] : NULL;
}

static inline bool in_heredoc(Scanner *scanner) {
  Frame *frame = top(scanner);
  return frame != NULL && frame->kind == MODE_HEREDOC;
}

// Whitespace before a scanner-owned token is the scanner's to skip: it is
// consulted before `extras` runs, so a token that expects the internal
// lexer to have cleaned up in front of it never fires (FIELD_GUIDE.md §8).
// Newlines are NOT skipped here — deciding what a newline means is the
// scanner's first job, not a thing to do on the way to something else.
static void skip_spaces(TSLexer *lexer) {
  while (lexer->lookahead == ' ' || lexer->lookahead == '\t') {
    lexer->advance(lexer, true);
  }
}

// The same, plus the line endings, for the states where a newline is not a
// terminator and so is only trivia.
//
// This is not belt-and-braces. `extras` and the external scanner are not
// interchangeable ways of getting rid of whitespace: measured against
// tree-sitter 0.26, a NAMED extra (a comment) sends the lexer round the
// loop again and the scanner is re-consulted, while an anonymous
// whitespace extra is skipped inside the internal lexer and the scanner is
// NOT. So a scanner-owned token that sits behind a newline — the `"` of
// `[\n"a"]`, the `<<` of a heredoc as a tuple element — is never offered
// unless the scanner steps over the newline itself. Every such element in
// a multi-line list failed until this existed.
static void skip_trivia(TSLexer *lexer) {
  while (lexer->lookahead == ' ' || lexer->lookahead == '\t' ||
         lexer->lookahead == '\n' || lexer->lookahead == '\r') {
    lexer->advance(lexer, true);
  }
}

// A run of newlines is ONE token. Blank lines between two attributes are
// not two terminators, and a grammar that had to admit `repeat1` here
// would be admitting a shape the language does not distinguish. Trailing
// horizontal whitespace after the last newline is left behind for
// `extras`, so the token ends exactly at a line boundary.
static bool scan_newline(TSLexer *lexer) {
  skip_spaces(lexer);
  if (lexer->lookahead != '\n' && lexer->lookahead != '\r') {
    return false;
  }

  bool consumed = false;
  for (;;) {
    if (lexer->lookahead == '\n') {
      lexer->advance(lexer, false);
      consumed = true;
      lexer->mark_end(lexer);
    } else if (lexer->lookahead == '\r') {
      lexer->advance(lexer, false);
      if (lexer->lookahead != '\n') {
        // A bare carriage return is not a line ending in HCL. Whatever was
        // already consumed stands; this one is left for the internal lexer
        // to fail on, which is the verdict hclsyntax gives it.
        break;
      }
      lexer->advance(lexer, false);
      consumed = true;
      lexer->mark_end(lexer);
    } else if (lexer->lookahead == ' ' || lexer->lookahead == '\t') {
      lexer->advance(lexer, false);
    } else {
      break;
    }
  }

  if (!consumed) {
    return false;
  }
  lexer->result_symbol = NEWLINE;
  return true;
}

// `("<<" | "<<-") Identifier Newline`, with the delimiter pushed onto the
// mode stack. The intro's own newline belongs to the heredoc and is
// consumed here; the one after the CLOSING delimiter is not, so a heredoc
// that runs to end of file without it is rejected exactly as HCL rejects
// it.
static bool scan_heredoc_open(Scanner *scanner, TSLexer *lexer) {
  skip_spaces(lexer);
  if (lexer->lookahead != '<') {
    return false;
  }
  lexer->advance(lexer, false);
  if (lexer->lookahead != '<') {
    return false;
  }
  lexer->advance(lexer, false);
  if (lexer->lookahead == '-') {
    lexer->advance(lexer, false);
  }

  char delimiter[MAX_DELIMITER];
  uint8_t length = 0;
  while (is_delimiter_char(lexer->lookahead)) {
    if (length < MAX_DELIMITER) {
      delimiter[length] = (char)lexer->lookahead;
    }
    length++;
    lexer->advance(lexer, false);
  }
  if (length == 0 || length > MAX_DELIMITER || scanner->depth >= MAX_FRAMES) {
    return false;
  }

  // The newline must follow the delimiter IMMEDIATELY. Skipping spaces
  // here accepted `<<EOT  ` with trailing whitespace, which hclsyntax
  // rejects -- found by a shape fixture that the reference parser then
  // declined to read. The closing line is the opposite case and does admit
  // trailing spaces; see `scan_heredoc_close`.
  if (lexer->lookahead == '\r') {
    lexer->advance(lexer, false);
  }
  if (lexer->lookahead != '\n') {
    return false;
  }
  lexer->advance(lexer, false);

  Frame *frame = &scanner->frames[scanner->depth++];
  frame->kind = MODE_HEREDOC;
  frame->length = length;
  memcpy(frame->delimiter, delimiter, length);

  lexer->mark_end(lexer);
  lexer->result_symbol = HEREDOC_OPEN;
  return true;
}

// Is the lexer sitting on the heredoc's closing delimiter line? Optional
// indentation, the delimiter, optional trailing spaces, and a newline that
// is deliberately NOT consumed — so a heredoc that runs to end of file
// without one is rejected exactly as HCL rejects it.
//
// This ADVANCES whether or not it matches, and that is what makes it safe
// to call from inside the literal scan: everything it can consume on a
// non-matching line (spaces, and a prefix of the delimiter's own
// characters) is ordinary literal text, containing no `$`, no `%` and no
// newline, so the caller can mark it as part of the run and carry on.
//
// The caller must have checked that the lexer is at column 0, and that
// check is load-bearing rather than decorative: `<<EOT\n${a}EOT\nEOT\n` has
// the delimiter's text mid-line, and a terminator test without it would
// close the heredoc there.
static bool scan_heredoc_close(Scanner *scanner, TSLexer *lexer, bool commit) {
  Frame *frame = top(scanner);
  while (lexer->lookahead == ' ' || lexer->lookahead == '\t') {
    lexer->advance(lexer, false);
  }
  for (uint8_t i = 0; i < frame->length; i++) {
    if (lexer->lookahead != (int32_t)(unsigned char)frame->delimiter[i]) {
      return false;
    }
    lexer->advance(lexer, false);
  }
  // A longer identifier is a different line, not this one: `EOTX` does not
  // close `EOT`.
  if (is_delimiter_char(lexer->lookahead)) {
    return false;
  }
  while (lexer->lookahead == ' ' || lexer->lookahead == '\t') {
    lexer->advance(lexer, false);
  }
  // The trailing spaces are PART of the delimiter line, which is what
  // hclsyntax's `TokenCHeredoc` spans; ending the token at the word left
  // them in nobody's node and the lexical check saw the boundary go
  // missing. The newline is still not consumed -- HCL requires one after
  // the delimiter, and the attribute's own terminator is what takes it.
  if (commit) {
    lexer->mark_end(lexer);
  }
  if (lexer->lookahead == '\r') {
    lexer->advance(lexer, false);
  }
  if (lexer->lookahead != '\n') {
    return false;
  }

  if (commit) {
    scanner->depth--;
    lexer->result_symbol = HEREDOC_CLOSE;
  }
  return true;
}

// A run of literal template text. The two modes differ in exactly two
// places: what ends the run, and whether a backslash is an escape. A
// heredoc's backslash is ordinary text — `<<EOT\na\\qb\nEOT` is valid HCL
// and `"a\\qb"` is not — so only the quoted mode stops for one.
//
// In heredoc mode this is also where the run ENDS, which is why the
// terminator probe lives inside the loop rather than beside it: at every
// line boundary the next line is either the delimiter (the run is over,
// having already been marked at the newline) or more text (the probe's
// consumed characters are part of it). A caller that tried the probe first
// and returned on failure would throw away those characters, and the
// indented first line of every `<<-` heredoc would start with an ERROR.
static bool scan_template_literal(Scanner *scanner, TSLexer *lexer,
                                  const bool *valid_symbols) {
  bool heredoc = in_heredoc(scanner);
  bool consumed = false;
  lexer->mark_end(lexer);

  // The run may begin on a line that is already the delimiter: an empty
  // heredoc, or one whose last content ended with an interpolation.
  if (heredoc && valid_symbols[HEREDOC_CLOSE] && lexer->get_column(lexer) == 0) {
    if (scan_heredoc_close(scanner, lexer, true)) {
      return true;
    }
    consumed = lexer->get_column(lexer) > 0;
    lexer->mark_end(lexer);
  }

  for (;;) {
    if (lexer->eof(lexer)) {
      break;
    }
    int32_t c = lexer->lookahead;

    if (!heredoc && (c == '"' || c == '\\' || c == '\n' || c == '\r')) {
      break;
    }

    if (c == '$' || c == '%') {
      lexer->advance(lexer, false);
      if (lexer->lookahead == '{') {
        // The interpolation or directive introduction. The token ends
        // before the `$`/`%`, which is where mark_end already is.
        break;
      }
      if (lexer->lookahead == c) {
        // `$${` and `%%{` escape the introduction and are literal text.
        lexer->advance(lexer, false);
        if (lexer->lookahead == '{') {
          lexer->advance(lexer, false);
        }
      }
      consumed = true;
      lexer->mark_end(lexer);
      continue;
    }

    if (heredoc && c == '\n') {
      lexer->advance(lexer, false);
      consumed = true;
      lexer->mark_end(lexer);
      // A heredoc's literal text is ONE TOKEN PER LINE, which is what
      // hclsyntax's scanner does too. The grammar puts the run back
      // together under one `template_literal`, so the token is not what a
      // consumer sees; what it buys is incremental reparsing that does not
      // re-lex a 500-line policy document because one line of it changed.
      //
      // The terminator probe has to run either way: it is what decides
      // whether the NEXT line is the delimiter. It deliberately does not
      // commit, so the frame stays on the stack and `_heredoc_close` is
      // lexed on the next call.
      scan_heredoc_close(scanner, lexer, false);
      break;
    }

    lexer->advance(lexer, false);
    consumed = true;
    lexer->mark_end(lexer);
  }

  if (!consumed) {
    return false;
  }
  lexer->result_symbol = TEMPLATE_LITERAL;
  return true;
}

void *tree_sitter_hcl_external_scanner_create(void) {
  return calloc(1, sizeof(Scanner));
}

void tree_sitter_hcl_external_scanner_destroy(void *payload) { free(payload); }

unsigned tree_sitter_hcl_external_scanner_serialize(void *payload, char *buffer) {
  Scanner *scanner = (Scanner *)payload;
  unsigned size = 0;
  buffer[size++] = (char)scanner->depth;
  for (uint8_t i = 0; i < scanner->depth; i++) {
    Frame *frame = &scanner->frames[i];
    buffer[size++] = (char)frame->kind;
    buffer[size++] = (char)frame->length;
    if (frame->length > 0) {
      memcpy(&buffer[size], frame->delimiter, frame->length);
      size += frame->length;
    }
  }
  return size;
}

void tree_sitter_hcl_external_scanner_deserialize(void *payload, const char *buffer,
                                                  unsigned length) {
  Scanner *scanner = (Scanner *)payload;
  memset(scanner, 0, sizeof(Scanner));
  if (length == 0) {
    return;
  }
  unsigned size = 0;
  uint8_t depth = (uint8_t)buffer[size++];
  for (uint8_t i = 0; i < depth && size < length; i++) {
    Frame *frame = &scanner->frames[i];
    frame->kind = (ModeKind)buffer[size++];
    frame->length = (uint8_t)buffer[size++];
    if (frame->length > 0) {
      memcpy(frame->delimiter, &buffer[size], frame->length);
      size += frame->length;
    }
    scanner->depth++;
  }
}

bool tree_sitter_hcl_external_scanner_scan(void *payload, TSLexer *lexer,
                                           const bool *valid_symbols) {
  Scanner *scanner = (Scanner *)payload;

  // Every symbol is marked valid during error recovery, so a scanner that
  // trusts `valid_symbols` there will push and pop template modes for a
  // parse that is not happening. Decline everything and leave the stack
  // alone (FIELD_GUIDE.md §8).
  if (valid_symbols[ERROR_SENTINEL]) {
    return false;
  }

  // Inside a template the scanner owns every character, including the
  // spaces: `" a"` is a one-space literal, and anything that skipped
  // whitespace on the way in would eat it.
  if (valid_symbols[TEMPLATE_LITERAL]) {
    if (valid_symbols[QUOTE_CLOSE] && lexer->lookahead == '"') {
      lexer->advance(lexer, false);
      lexer->mark_end(lexer);
      if (scanner->depth > 0) {
        scanner->depth--;
      }
      lexer->result_symbol = QUOTE_CLOSE;
      return true;
    }
    return scan_template_literal(scanner, lexer, valid_symbols);
  }

  // A heredoc or a quoted template with no literal text left in it still
  // has to close.
  if (valid_symbols[HEREDOC_CLOSE] && in_heredoc(scanner) &&
      lexer->get_column(lexer) == 0) {
    return scan_heredoc_close(scanner, lexer, true);
  }
  if (valid_symbols[QUOTE_CLOSE] && lexer->lookahead == '"') {
    lexer->advance(lexer, false);
    lexer->mark_end(lexer);
    if (scanner->depth > 0) {
      scanner->depth--;
    }
    lexer->result_symbol = QUOTE_CLOSE;
    return true;
  }

  if (!valid_symbols[NEWLINE] && !valid_symbols[HEREDOC_OPEN] &&
      !valid_symbols[QUOTE_OPEN]) {
    return false;
  }

  if (valid_symbols[NEWLINE]) {
    if (scan_newline(lexer)) {
      return true;
    }
    skip_spaces(lexer);
  } else {
    skip_trivia(lexer);
  }

  if (valid_symbols[HEREDOC_OPEN] && lexer->lookahead == '<') {
    if (scan_heredoc_open(scanner, lexer)) {
      return true;
    }
    return false;
  }

  if (valid_symbols[QUOTE_OPEN] && lexer->lookahead == '"') {
    if (scanner->depth >= MAX_FRAMES) {
      return false;
    }
    lexer->advance(lexer, false);
    lexer->mark_end(lexer);
    Frame *frame = &scanner->frames[scanner->depth++];
    frame->kind = MODE_QUOTED;
    frame->length = 0;
    lexer->result_symbol = QUOTE_OPEN;
    return true;
  }

  return false;
}
