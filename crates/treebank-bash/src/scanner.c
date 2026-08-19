// Bash's external scanner: heredocs, and the word-concatenation boundary.
//
// Two things the parse table cannot express.
//
// A HEREDOC body is delimited by a word that appeared earlier on the same
// line: `<<EOF` means "read until a line that is exactly EOF". No regular
// token can say that, because the terminator is not known until the
// redirect has been parsed. So the scanner remembers the word and matches
// it — which is also why it needs serialize/deserialize to survive an
// incremental reparse.
//
// CONCATENATION is the other. `a$b"c"` is one word in shell and three in
// any tokenizer, and what joins them is the absence of whitespace. The
// parser cannot see absence, so the scanner emits a zero-width token when
// two word parts abut.

#include "tree_sitter/parser.h"
#include <string.h>
#include <wctype.h>

enum TokenType {
  HEREDOC_START,
  HEREDOC_BODY,
  HEREDOC_END,
  CONCAT,
  ASSIGNMENT_NAME,
  FILE_DESCRIPTOR,
  ERROR_SENTINEL,
};

#define MAX_DELIM 64

typedef struct {
  char delimiter[MAX_DELIM];
  unsigned length;
  bool allows_indent;   // `<<-` strips leading tabs from the terminator
  bool started;         // the body has been consumed; the end is next
} Scanner;

void *tree_sitter_bash_external_scanner_create(void) {
  Scanner *s = calloc(1, sizeof(Scanner));
  return s;
}

void tree_sitter_bash_external_scanner_destroy(void *payload) { free(payload); }

unsigned tree_sitter_bash_external_scanner_serialize(void *payload, char *buffer) {
  Scanner *s = (Scanner *)payload;
  unsigned n = 0;
  buffer[n++] = (char)s->length;
  buffer[n++] = (char)s->allows_indent;
  buffer[n++] = (char)s->started;
  memcpy(buffer + n, s->delimiter, s->length);
  n += s->length;
  return n;
}

void tree_sitter_bash_external_scanner_deserialize(void *payload, const char *buffer,
                                                   unsigned length) {
  Scanner *s = (Scanner *)payload;
  memset(s, 0, sizeof(Scanner));
  if (length == 0) return;
  unsigned n = 0;
  s->length = (unsigned char)buffer[n++];
  s->allows_indent = buffer[n++];
  s->started = buffer[n++];
  if (s->length > MAX_DELIM) s->length = MAX_DELIM;
  memcpy(s->delimiter, buffer + n, s->length);
}

static void advance(TSLexer *lexer) { lexer->advance(lexer, false); }
static void skip(TSLexer *lexer) { lexer->advance(lexer, true); }

static bool is_word_char(int32_t c) {
  return iswalnum(c) || c == '_' || c == '-' || c == '.';
}

// `<<EOF`, `<<-EOF`, `<<"EOF"`, `<<'EOF'`. Only the NAME is captured; the
// quoting decides whether the body expands, which the grammar does not
// model yet.
static bool scan_heredoc_start(Scanner *s, TSLexer *lexer) {
  while (lexer->lookahead == ' ' || lexer->lookahead == '\t') skip(lexer);
  int32_t quote = 0;
  if (lexer->lookahead == '"' || lexer->lookahead == '\'') {
    quote = lexer->lookahead;
    advance(lexer);
  }
  s->length = 0;
  while (s->length < MAX_DELIM - 1 &&
         (quote ? (lexer->lookahead != quote && lexer->lookahead != 0)
                : is_word_char(lexer->lookahead))) {
    s->delimiter[s->length++] = (char)lexer->lookahead;
    advance(lexer);
  }
  if (quote) {
    if (lexer->lookahead != quote) return false;
    advance(lexer);
  }
  if (s->length == 0) return false;
  s->started = false;
  lexer->result_symbol = HEREDOC_START;
  return true;
}

// Everything up to the line that is exactly the delimiter.
static bool scan_heredoc_body(Scanner *s, TSLexer *lexer) {
  if (s->length == 0) return false;
  bool any = false;
  for (;;) {
    // At the start of a line: is this the terminator?
    lexer->mark_end(lexer);
    if (s->allows_indent) {
      while (lexer->lookahead == '\t') advance(lexer);
    }
    unsigned i = 0;
    while (i < s->length && lexer->lookahead == (int32_t)s->delimiter[i]) {
      advance(lexer);
      i++;
    }
    if (i == s->length && (lexer->lookahead == '\n' || lexer->lookahead == 0)) {
      if (!any) return false;      // the body is empty; let HEREDOC_END run
      lexer->result_symbol = HEREDOC_BODY;
      return true;
    }
    // Not the terminator: consume the rest of the line.
    while (lexer->lookahead != '\n' && lexer->lookahead != 0) advance(lexer);
    if (lexer->lookahead == 0) {
      lexer->mark_end(lexer);
      if (!any) return false;
      lexer->result_symbol = HEREDOC_BODY;
      return true;
    }
    advance(lexer);
    any = true;
  }
}

static bool scan_heredoc_end(Scanner *s, TSLexer *lexer) {
  if (s->length == 0) return false;
  if (s->allows_indent) {
    while (lexer->lookahead == '\t') skip(lexer);
  }
  for (unsigned i = 0; i < s->length; i++) {
    if (lexer->lookahead != (int32_t)s->delimiter[i]) return false;
    advance(lexer);
  }
  if (lexer->lookahead != '\n' && lexer->lookahead != 0) return false;
  lexer->mark_end(lexer);
  s->length = 0;
  lexer->result_symbol = HEREDOC_END;
  return true;
}

// `NAME=` and `NAME+=` at the head of a command, consuming only the NAME.
//
// This is the one thing the parse table cannot do and the scanner can. At
// `echo hi` and `x=1` the first token looks identical to a tokenizer: both
// are a run of name characters, and `variable_name` and `word` both match.
// The lexer has to pick one, and whichever it picks the other reading is
// gone. Shell itself resolves this by looking PAST the name for an `=`,
// which is a lookahead the grammar has no way to express — so the scanner
// does it, and emits this token only when the `=` is really there.
static bool scan_assignment_name(TSLexer *lexer) {
  if (!(iswalpha(lexer->lookahead) || lexer->lookahead == '_')) return false;
  while (iswalnum(lexer->lookahead) || lexer->lookahead == '_') advance(lexer);
  lexer->mark_end(lexer);
  // An array subscript may sit between the name and the `=`: `a[0]=1`.
  if (lexer->lookahead == '[') return false;
  if (lexer->lookahead == '+') advance(lexer);
  if (lexer->lookahead != '=') return false;
  lexer->result_symbol = ASSIGNMENT_NAME;
  return true;
}

bool tree_sitter_bash_external_scanner_scan(void *payload, TSLexer *lexer,
                                            const bool *valid) {
  Scanner *s = (Scanner *)payload;

  // In error recovery tree-sitter marks EVERY external token valid, which
  // is fatal for a zero-width one: CONCAT consumes nothing, so a parser
  // that keeps being offered it never advances and the whole sweep hangs
  // on one file. Decline the scanner there and let ordinary recovery run.
  if (valid[ERROR_SENTINEL]) return false;

  // CONCAT is zero-width and judged on the RAW lookahead -- it says only
  // that the previous token ended exactly where this one begins -- so it
  // must be decided before ANY block that skips whitespace runs. It fell
  // after such a block once, saw the post-skip character, and glued two
  // arguments a space separated.
  if (valid[CONCAT] && !valid[HEREDOC_BODY]) {
    int32_t c = lexer->lookahead;
    bool joins = c != ' ' && c != '\t' && c != '\n' && c != '\r' && c != 0 &&
                 c != ';' && c != '&' && c != '|' && c != ')' && c != '(' &&
                 c != '<' && c != '>';
    if (joins) {
      lexer->result_symbol = CONCAT;
      lexer->mark_end(lexer);
      return true;
    }
    // Not a join: fall through rather than return, because a redirect's
    // file descriptor may legitimately start after this very whitespace.
  }

  // A digit run is a file descriptor ONLY when a redirect operator abuts
  // it: `echo 2> f` redirects fd 2, `echo 2 > f` passes an argument. Both
  // `number` and this token match the same characters, the internal lexer
  // must pick one per state, and whichever it picks the other reading is
  // gone -- the same shape as ASSIGNMENT_NAME below, resolved the same
  // way: look past the digits, emit only when the operator is really
  // there.
  if (valid[FILE_DESCRIPTOR] && !valid[HEREDOC_BODY]) {
    while (lexer->lookahead == ' ' || lexer->lookahead == '\t') skip(lexer);
    if (lexer->lookahead >= '0' && lexer->lookahead <= '9') {
      while (lexer->lookahead >= '0' && lexer->lookahead <= '9') advance(lexer);
      lexer->mark_end(lexer);
      if (lexer->lookahead == '<' || lexer->lookahead == '>') {
        lexer->result_symbol = FILE_DESCRIPTOR;
        return true;
      }
      // Digits not followed by an operator can be nothing else we scan
      // for; let the internal lexer have them back.
      return false;
    }
  }

  if (valid[ASSIGNMENT_NAME] && !valid[HEREDOC_BODY]) {
    while (lexer->lookahead == ' ' || lexer->lookahead == '\t') skip(lexer);
    if (scan_assignment_name(lexer)) return true;
  }


  if (valid[HEREDOC_END] && s->length > 0 && s->started) {
    if (scan_heredoc_end(s, lexer)) return true;
  }
  if (valid[HEREDOC_BODY] && s->length > 0) {
    if (scan_heredoc_body(s, lexer)) {
      s->started = true;
      return true;
    }
    s->started = true;
  }
  if (valid[HEREDOC_END] && s->length > 0) {
    if (scan_heredoc_end(s, lexer)) return true;
  }
  if (valid[HEREDOC_START]) {
    return scan_heredoc_start(s, lexer);
  }
  return false;
}
