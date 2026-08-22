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
  HEREDOC_START_DASH,
  HEREDOC_BODY,
  HEREDOC_END,
  HEREDOC_NEWLINE,
  CONCAT,
  ASSIGNMENT_NAME,
  FILE_DESCRIPTOR,
  BACKTICK_OPEN,
  BACKTICK_CLOSE,
  DOLLAR_LITERAL,
  BRACE_EXPR_START,
  ERROR_SENTINEL,
};

#define MAX_DELIM 64
#define MAX_HEREDOCS 15

typedef struct {
  char delimiter[MAX_DELIM];
  unsigned length;
  bool allows_indent;   // `<<-` strips leading tabs from the terminator
  bool started;         // the body has been consumed; the end is next
} Heredoc;

typedef struct {
  Heredoc heredocs[MAX_HEREDOCS];
  unsigned heredoc_count;
  bool needs_heredoc_newline; // another queued body follows an end marker
  bool in_backtick;     // between the opening backtick and its close
} Scanner;

static Heredoc *current_heredoc(Scanner *s) {
  return s->heredoc_count > 0 ? &s->heredocs[0] : NULL;
}

static void pop_heredoc(Scanner *s) {
  if (s->heredoc_count == 0) return;
  s->heredoc_count--;
  if (s->heredoc_count > 0) {
    memmove(&s->heredocs[0], &s->heredocs[1],
            s->heredoc_count * sizeof(Heredoc));
  }
  memset(&s->heredocs[s->heredoc_count], 0, sizeof(Heredoc));
  s->needs_heredoc_newline = s->heredoc_count > 0;
}

void *tree_sitter_bash_external_scanner_create(void) {
  Scanner *s = calloc(1, sizeof(Scanner));
  return s;
}

void tree_sitter_bash_external_scanner_destroy(void *payload) { free(payload); }

unsigned tree_sitter_bash_external_scanner_serialize(void *payload, char *buffer) {
  Scanner *s = (Scanner *)payload;
  unsigned n = 0;
  buffer[n++] = (char)s->heredoc_count;
  buffer[n++] = (char)s->needs_heredoc_newline;
  buffer[n++] = (char)s->in_backtick;
  for (unsigned i = 0; i < s->heredoc_count; i++) {
    Heredoc *h = &s->heredocs[i];
    buffer[n++] = (char)h->length;
    buffer[n++] = (char)h->allows_indent;
    buffer[n++] = (char)h->started;
    memcpy(buffer + n, h->delimiter, h->length);
    n += h->length;
  }
  return n;
}

void tree_sitter_bash_external_scanner_deserialize(void *payload, const char *buffer,
                                                   unsigned length) {
  Scanner *s = (Scanner *)payload;
  memset(s, 0, sizeof(Scanner));
  if (length < 3) return;
  unsigned n = 0;
  s->heredoc_count = (unsigned char)buffer[n++];
  s->needs_heredoc_newline = buffer[n++];
  s->in_backtick = buffer[n++];
  if (s->heredoc_count > MAX_HEREDOCS) s->heredoc_count = MAX_HEREDOCS;
  for (unsigned i = 0; i < s->heredoc_count; i++) {
    if (n + 3 > length) {
      s->heredoc_count = i;
      return;
    }
    Heredoc *h = &s->heredocs[i];
    h->length = (unsigned char)buffer[n++];
    h->allows_indent = buffer[n++];
    h->started = buffer[n++];
    if (h->length >= MAX_DELIM) h->length = MAX_DELIM - 1;
    if (n + h->length > length) {
      s->heredoc_count = i;
      return;
    }
    memcpy(h->delimiter, buffer + n, h->length);
    n += h->length;
  }
}

static void advance(TSLexer *lexer) { lexer->advance(lexer, false); }
static void skip(TSLexer *lexer) { lexer->advance(lexer, true); }

static bool is_word_char(int32_t c) {
  return iswalnum(c) || c == '_' || c == '-' || c == '.';
}

// `<<EOF`, `<<-EOF`, `<<"EOF"`, `<<'EOF'`. Only the NAME is captured; the
// quoting decides whether the body expands, which the grammar does not
// model yet.
static bool scan_heredoc_start(Scanner *s, TSLexer *lexer, bool dash) {
  if (s->heredoc_count >= MAX_HEREDOCS) return false;
  Heredoc *h = &s->heredocs[s->heredoc_count];
  memset(h, 0, sizeof(Heredoc));
  while (lexer->lookahead == ' ' || lexer->lookahead == '\t') skip(lexer);
  int32_t quote = 0;
  if (lexer->lookahead == '"' || lexer->lookahead == '\'') {
    quote = lexer->lookahead;
    advance(lexer);
  }
  while (h->length < MAX_DELIM - 1 &&
         (quote ? (lexer->lookahead != quote && lexer->lookahead != 0)
                : is_word_char(lexer->lookahead))) {
    h->delimiter[h->length++] = (char)lexer->lookahead;
    advance(lexer);
  }
  if (quote) {
    if (lexer->lookahead != quote) return false;
    advance(lexer);
  }
  if (h->length == 0) return false;
  h->allows_indent = dash;
  s->heredoc_count++;
  lexer->result_symbol = dash ? HEREDOC_START_DASH : HEREDOC_START;
  return true;
}

// Everything up to the line that is exactly the delimiter.
static bool scan_heredoc_body(Heredoc *h, TSLexer *lexer) {
  if (h == NULL || h->length == 0) return false;
  bool any = false;
  for (;;) {
    // At the start of a line: is this the terminator?
    lexer->mark_end(lexer);
    if (h->allows_indent) {
      while (lexer->lookahead == '\t') advance(lexer);
    }
    unsigned i = 0;
    while (i < h->length && lexer->lookahead == (int32_t)h->delimiter[i]) {
      advance(lexer);
      i++;
    }
    if (i == h->length && (lexer->lookahead == '\n' || lexer->lookahead == 0)) {
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

static bool scan_heredoc_end(Heredoc *h, TSLexer *lexer) {
  if (h == NULL || h->length == 0) return false;
  if (h->allows_indent) {
    while (lexer->lookahead == '\t') skip(lexer);
  }
  for (unsigned i = 0; i < h->length; i++) {
    if (lexer->lookahead != (int32_t)h->delimiter[i]) return false;
    advance(lexer);
  }
  if (lexer->lookahead != '\n' && lexer->lookahead != 0) return false;
  lexer->mark_end(lexer);
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
  // The token is still just the NAME -- mark_end already fenced it -- but
  // the lookahead must walk the brackets to see whether an `=` really
  // follows, or `a[0]=1` lexes as one word and becomes a command named
  // `a[0]=1`: the wrong tree with no error, invisible to the sweep and
  // found by the mvdan/sh span oracle (issue #143).
  if (lexer->lookahead == '[') {
    int depth = 0;
    while (lexer->lookahead != 0 && lexer->lookahead != '\n') {
      if (lexer->lookahead == '[') depth++;
      else if (lexer->lookahead == ']') {
        depth--;
        if (depth == 0) { advance(lexer); break; }
      }
      advance(lexer);
    }
    if (depth != 0) return false;
  }
  if (lexer->lookahead == '+') advance(lexer);
  if (lexer->lookahead != '=') return false;
  // `==` inside `[[ a == b ]]` must not read as an assignment to `a=`.
  advance(lexer);
  if (lexer->lookahead == '=') return false;
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

  // This token is valid only after a COMPLETE statement that registered a
  // heredoc. Consuming the newline here creates a parser state in which the
  // body is legal; before it, redirects, arguments and pipeline tails remain
  // ordinary syntax on the opening line.
  if (valid[HEREDOC_NEWLINE] && s->heredoc_count > 0) {
    if (lexer->lookahead == '\r') {
      advance(lexer);
      if (lexer->lookahead != '\n') return false;
    }
    if (lexer->lookahead == '\n') {
      advance(lexer);
      lexer->mark_end(lexer);
      lexer->result_symbol = HEREDOC_NEWLINE;
      return true;
    }
  }

  // CONCAT is zero-width and judged on the RAW lookahead -- it says only
  // that the previous token ended exactly where this one begins -- so it
  // must be decided before ANY block that skips whitespace runs. It fell
  // after such a block once, saw the post-skip character, and glued two
  // arguments a space separated.
  if (valid[CONCAT] && !valid[HEREDOC_BODY]) {
    int32_t c = lexer->lookahead;
    // `}` cannot join: the word class excludes it, so nothing BEFORE a
    // closing brace continues past it -- and with `}` counted as joining,
    // every `${x:-/tmp}` and `${y//a/b}` ended in a zero-width CONCAT
    // demanding a word that could not exist: the 5,041-file
    // `concatenation > MISSING word` cluster. `{` stays joining, because
    // `a{b,c}d` really is one word and the brace_expression is its middle.
    // Concatenation ACROSS a closing brace -- `${a}tail` -- is unaffected:
    // that CONCAT decision happens after the `}` token, where the
    // lookahead is the continuation itself.
    bool joins = c != ' ' && c != '\t' && c != '\n' && c != '\r' && c != 0 &&
                 c != ';' && c != '&' && c != '|' && c != ')' && c != '(' &&
                 c != '<' && c != '>' && c != '}' &&
                 // A backtick joins a concatenation OUTSIDE a substitution
                 // (`a\`date\`b` is one word) and CLOSES one inside --
                 // the parity bit is the difference.
                 (c != '`' || !s->in_backtick);
    if (joins) {
      lexer->result_symbol = CONCAT;
      lexer->mark_end(lexer);
      return true;
    }
    // Not a join: fall through rather than return, because a redirect's
    // file descriptor may legitimately start after this very whitespace.
  }

  // Zero-width gate for brace expansion. bash expands `{a,b}` only when
  // an UNQUOTED comma sits inside the matching braces with no whitespace
  // anywhere -- `{ :; }` is a compound statement, `{x}` a literal word.
  // The grammar cannot look ahead to the close, and the first attempt at
  // a nested rule died in the LALR state both readings share: the element
  // token out-lexed the word that the compound reading needed (#168).
  // Deciding HERE, before the `{` is even shifted, keeps the two worlds in
  // separate states and the conflict never forms.
  if (valid[BRACE_EXPR_START]) {
    // Self-skips whitespace: the external scanner runs BEFORE the extras
    // skip and is not called again after it, so a gate that only checks
    // the raw lookahead never sees a `{` that follows a space.
    while (lexer->lookahead == ' ' || lexer->lookahead == '\t') skip(lexer);
    if (lexer->lookahead != '{') goto not_brace;
    lexer->mark_end(lexer);
    int depth = 0;
    bool comma = false;
    unsigned n = 0;
    int32_t c = lexer->lookahead;
    while (c != 0 && n < 4096) {
      if (c == '{') depth++;
      else if (c == '}') {
        depth--;
        if (depth == 0) break;
      } else if (c == ',' && depth >= 1) comma = true;
      else if (c == ' ' || c == '\t' || c == '\n' || c == '\r') { depth = -1; break; }
      advance(lexer);
      c = lexer->lookahead;
      n++;
    }
    if (depth == 0 && comma) {
      lexer->result_symbol = BRACE_EXPR_START;
      return true;
    }
    return false;
  }
not_brace:;

  // A digit run is a file descriptor ONLY when a redirect operator abuts
  // it: `echo 2> f` redirects fd 2, `echo 2 > f` passes an argument. Both
  // `number` and this token match the same characters, the internal lexer
  // must pick one per state, and whichever it picks the other reading is
  // gone -- the same shape as ASSIGNMENT_NAME below, resolved the same
  // way: look past the digits, emit only when the operator is really
  // there.
  // A `$` that no expansion can follow is literal string content:
  // `"$"`, `"v$/x"`. One character of lookahead settles it -- everything
  // that CAN start an expansion after `$` is listed, and anything else
  // (a quote, a slash, a space) means the dollar is just a dollar.
  if (valid[DOLLAR_LITERAL] && lexer->lookahead == '$') {
    advance(lexer);
    lexer->mark_end(lexer);
    int32_t c = lexer->lookahead;
    bool expandable = iswalnum(c) || c == '_' || c == '{' || c == '(' ||
                      c == '!' || c == '#' || c == '?' || c == '@' ||
                      c == '*' || c == '-' || c == '$' || c == '\'';
    if (!expandable) {
      lexer->result_symbol = DOLLAR_LITERAL;
      return true;
    }
    return false;
  }

  // Backticks close on the FIRST unescaped backtick -- bash's own rule,
  // and the reason the old-style substitution cannot nest. A parity bit is
  // all it takes, and it is lexer state, which is exactly what the grammar
  // cannot express: at the closing backtick the parser would otherwise
  // happily open a nested substitution and run to EOF.
  if ((valid[BACKTICK_OPEN] || valid[BACKTICK_CLOSE])) {
    int32_t c = lexer->lookahead;
    while (c == ' ' || c == '\t') { skip(lexer); c = lexer->lookahead; }
    if (c == '`') {
      if (!s->in_backtick && valid[BACKTICK_OPEN]) {
        advance(lexer);
        lexer->mark_end(lexer);
        s->in_backtick = true;
        lexer->result_symbol = BACKTICK_OPEN;
        return true;
      }
      if (s->in_backtick && valid[BACKTICK_CLOSE]) {
        advance(lexer);
        lexer->mark_end(lexer);
        s->in_backtick = false;
        lexer->result_symbol = BACKTICK_CLOSE;
        return true;
      }
      return false;
    }
  }

  if (valid[FILE_DESCRIPTOR] && !valid[HEREDOC_BODY]) {
    while (lexer->lookahead == ' ' || lexer->lookahead == '\t') skip(lexer);
    // `exec {lock_fd}> file`: bash allocates a descriptor into the named
    // variable. Same adjacency contract as the numeric form, same peek --
    // and as a grammar token this ate `echo {x}`, where nothing follows.
    if (lexer->lookahead == '{') {
      advance(lexer);
      if (!(iswalpha(lexer->lookahead) || lexer->lookahead == '_')) return false;
      while (iswalnum(lexer->lookahead) || lexer->lookahead == '_') advance(lexer);
      if (lexer->lookahead != '}') return false;
      advance(lexer);
      lexer->mark_end(lexer);
      if (lexer->lookahead == '<' || lexer->lookahead == '>') {
        lexer->result_symbol = FILE_DESCRIPTOR;
        return true;
      }
      return false;
    }
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


  Heredoc *h = current_heredoc(s);
  if (h != NULL && s->needs_heredoc_newline) {
    if (lexer->lookahead == '\r') {
      skip(lexer);
      if (lexer->lookahead != '\n') return false;
    }
    if (lexer->lookahead != '\n') return false;
    skip(lexer);
    s->needs_heredoc_newline = false;
  }
  if (valid[HEREDOC_END] && h != NULL && h->started) {
    if (scan_heredoc_end(h, lexer)) {
      pop_heredoc(s);
      return true;
    }
  }
  if (valid[HEREDOC_BODY] && h != NULL) {
    if (scan_heredoc_body(h, lexer)) {
      h->started = true;
      return true;
    }
    h->started = true;
  }
  if (valid[HEREDOC_END] && h != NULL) {
    if (scan_heredoc_end(h, lexer)) {
      pop_heredoc(s);
      return true;
    }
  }
  if (valid[HEREDOC_START] || valid[HEREDOC_START_DASH]) {
    return scan_heredoc_start(s, lexer, valid[HEREDOC_START_DASH]);
  }
  return false;
}
