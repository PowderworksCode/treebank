// treebank-python external scanner.
//
// Three jobs, all of them things a context-free grammar cannot do:
//
// 1. Logical lines: NEWLINE / INDENT / DEDENT off an indentation stack.
//    Newlines inside brackets need no special handling here — the parse
//    table simply never offers _newline there, the scanner declines, and
//    the whitespace extra eats the character. Bracket tracking comes free
//    from the parser state.
// 2. String boundaries: one STRING_START token carrying the prefix and
//    quote(s), STRING_CONTENT runs, STRING_END — with a stack of open
//    strings so f-string interpolations can nest strings (and, per
//    PEP 701, even reuse the same quote).
// 3. f-string content stops at `{` and `}` so the grammar can parse
//    interpolations; raw-string content swallows backslashes itself.
//
// The scanner never emits a verdict it cannot justify from its own state:
// in error recovery (every symbol valid) it does nothing at EOF, so it can
// never loop.

#include "tree_sitter/parser.h"

#include <string.h>

enum TokenType {
  NEWLINE,
  INDENT,
  DEDENT,
  STRING_START,
  STRING_CONTENT,
  STRING_END,
  LINE_START,
};

#define MAX_INDENTS 100
#define MAX_STRINGS 60

enum StringFlags {
  RAW = 1 << 0,
  FSTRING = 1 << 1,
  BYTES = 1 << 2,
  TRIPLE = 1 << 3,
};

typedef struct {
  uint8_t flags;
  char quote;
} OpenString;

typedef struct {
  // True when the last scanner-owned token was a line boundary
  // (NEWLINE/INDENT/DEDENT), i.e. the next real token starts a line.
  bool line_start_pending;
  uint32_t pending_dedents;
  uint32_t indent_count;
  uint16_t indents[MAX_INDENTS];
  uint32_t string_count;
  OpenString strings[MAX_STRINGS];
} Scanner;

// One scanner source, two parsers: `tree-sitter generate` names the
// external-scanner entry points after the GRAMMAR, so a shared scanner
// cannot spell them literally. The variant's src/scanner.c stub sets the
// prefix before including this file; python3 gets the default, which keeps
// `tree_sitter_python_external_scanner_*` exactly as it was.
#ifndef TREEBANK_SCANNER_PREFIX
#define TREEBANK_SCANNER_PREFIX tree_sitter_python
#endif
#define TB_CAT_(a, b) a##b
#define TB_CAT(a, b) TB_CAT_(a, b)
#define TB_SCANNER(name) TB_CAT(TREEBANK_SCANNER_PREFIX, name)

static inline void advance(TSLexer *lexer) { lexer->advance(lexer, false); }
static inline void skip(TSLexer *lexer) { lexer->advance(lexer, true); }

// ── indentation ─────────────────────────────────────────────────────────

// ── strings ─────────────────────────────────────────────────────────────

static bool scan_string_start(TSLexer *lexer, Scanner *s) {
  uint8_t flags = 0;
  // Up to two prefix letters, case-insensitive. Anything else is not a
  // string prefix and we decline (the internal lexer will read an
  // identifier instead). Which combinations are legal is per variant:
  // py3 takes r b u f br rb fr rf, py2 takes r b u br rb ur and no f.
  int letters = 0;
  bool seen[4] = {false, false, false, false}; // r b u f
  while (letters < 2) {
    int32_t c = lexer->lookahead;
    int idx = (c == 'r' || c == 'R') ? 0
            : (c == 'b' || c == 'B') ? 1
            : (c == 'u' || c == 'U') ? 2
            : (c == 'f' || c == 'F') ? 3
            : -1;
    if (idx < 0) break;
    if (seen[idx]) return false;
    seen[idx] = true;
    letters++;
    advance(lexer);
  }
#ifdef TREEBANK_PYTHON2
  if (seen[2] && (seen[1] || seen[3])) return false; // u mixes with nothing but r
#else
  // Python 3.3 restored the `u` prefix but NOT `ur`: in py3 a `u` stands
  // alone. The union grammar had to take the py2 rule, so `ur"x"` lexed as
  // a string in py3 code -- the one py2 form the parse table could not
  // reject on its own, because a string prefix is decided down here.
  if (seen[2] && (seen[0] || seen[1] || seen[3])) return false;
#endif
  if (seen[1] && seen[3]) return false;              // no fb
#ifdef TREEBANK_PYTHON2
  // PEP 498 is python 3.6. In py2 an `f` prefix is not a string prefix at
  // all, so declining here is what makes `f"{x}"` lex as the name `f`
  // followed by a string -- two tokens the grammar then rejects.
  if (seen[3]) return false;
#endif
  if (seen[0]) flags |= RAW;
  if (seen[1]) flags |= BYTES;
  if (seen[3]) flags |= FSTRING;

  int32_t quote = lexer->lookahead;
  if (quote != '\'' && quote != '"') return false;
  advance(lexer);
  lexer->mark_end(lexer);
  if (lexer->lookahead == quote) {
    advance(lexer);
    if (lexer->lookahead == quote) {
      advance(lexer);
      lexer->mark_end(lexer);
      flags |= TRIPLE;
    }
    // else: empty string — the two quotes are START then END; leave the
    // token as the single opening quote.
  }
  if (s->string_count >= MAX_STRINGS) return false;
  s->strings[s->string_count].flags = flags;
  s->strings[s->string_count].quote = (char)quote;
  s->string_count++;
  lexer->result_symbol = STRING_START;
  return true;
}

static bool scan_string_body(TSLexer *lexer, Scanner *s, const bool *valid) {
  OpenString *os = &s->strings[s->string_count - 1];
  bool raw = os->flags & RAW;
  bool fstring = os->flags & FSTRING;
  bool triple = os->flags & TRIPLE;
  int32_t quote = os->quote;
  bool has_content = false;

  for (;;) {
    int32_t c = lexer->lookahead;
    if (c == 0) {
      break; // unterminated; emit what we have or fail
    } else if (c == '\\') {
      if (!raw) break; // grammar's escape_sequence token takes over
      // Raw string: the backslash and whatever follows it (a quote, a
      // newline, anything) are content; the backslash still shields a
      // closing quote lexically. In a RAW F-string the braces stay live:
      // fr"\{x}" is a literal backslash followed by an interpolation.
      advance(lexer);
      if (fstring && (lexer->lookahead == '{' || lexer->lookahead == '}')) {
        has_content = true;
        continue;
      }
      if (lexer->lookahead) advance(lexer);
      has_content = true;
      continue;
    } else if (fstring && (c == '{' || c == '}')) {
      break; // interpolation or {{ }} escape; grammar decides
    } else if (c == '\n' && !triple) {
      break; // unterminated single-line string
    } else if (c == quote) {
      if (!triple) {
        if (has_content) break;
        if (valid[STRING_END]) {
          advance(lexer);
          lexer->mark_end(lexer);
          s->string_count--;
          lexer->result_symbol = STRING_END;
          return true;
        }
        return false;
      }
      // Triple: only three in a row end it.
      lexer->mark_end(lexer);
      advance(lexer);
      if (lexer->lookahead == quote) {
        advance(lexer);
        if (lexer->lookahead == quote) {
          if (has_content) {
            // Content run ends before the closing quotes.
            lexer->result_symbol = STRING_CONTENT;
            return valid[STRING_CONTENT];
          }
          advance(lexer);
          lexer->mark_end(lexer);
          s->string_count--;
          lexer->result_symbol = STRING_END;
          return valid[STRING_END];
        }
      }
      // One or two quotes: content.
      has_content = true;
      lexer->mark_end(lexer);
      continue;
    }
    advance(lexer);
    has_content = true;
  }

  if (has_content && valid[STRING_CONTENT]) {
    lexer->mark_end(lexer);
    lexer->result_symbol = STRING_CONTENT;
    return true;
  }
  return false;
}

// ── the entry points ────────────────────────────────────────────────────

/* Indentation of the next line that has CODE on it, skipping comment-only
 * and blank lines the way CPython's tokenizer does. Pure lookahead: every
 * character is consumed with skip(), and the caller has already marked the
 * token end, so nothing here widens the token. Returns 0 at EOF, which
 * closes every open block -- which is what EOF should do. */
#define NO_DEDENT UINT32_MAX

static uint32_t next_code_column(TSLexer *lexer) {
  for (;;) {
    /* Skip to the end of the current line. */
    while (lexer->lookahead != 0 && lexer->lookahead != '\n' &&
           lexer->lookahead != '\r') {
      skip(lexer);
    }
    if (lexer->lookahead == 0) return 0;
    if (lexer->lookahead == '\r') skip(lexer);
    if (lexer->lookahead == '\n') skip(lexer);

    /* Measure the next line's indentation. */
    while (lexer->lookahead == ' ' || lexer->lookahead == '\t' ||
           lexer->lookahead == '\f') {
      skip(lexer);
    }
    if (lexer->lookahead == 0) return 0;
    /* Blank or comment: not a line of code, keep looking. */
    if (lexer->lookahead == '\n' || lexer->lookahead == '\r' ||
        lexer->lookahead == '#') {
      continue;
    }
    /* A continuation clause means the compound statement is NOT over, so
     * nothing may be closed in front of the comment -- and a comment sitting
     * above an `else` reads as belonging to the body it follows anyway.
     * Report a column no dedent can trigger on. Same four keywords the
     * LINE_START peek uses, and for the same reason: at a block boundary the
     * token stream is shared across GLR forks, and closing early kills the
     * fork that is waiting for the clause. */
    uint32_t col = lexer->get_column(lexer);
    if (lexer->lookahead == 'e' || lexer->lookahead == 'f') {
      char word[9];
      int n = 0;
      while (n < 8 && lexer->lookahead >= 'a' && lexer->lookahead <= 'z') {
        word[n++] = (char)lexer->lookahead;
        skip(lexer);
      }
      word[n] = 0;
      if (strcmp(word, "elif") == 0 || strcmp(word, "else") == 0 ||
          strcmp(word, "except") == 0 || strcmp(word, "finally") == 0) {
        return NO_DEDENT;
      }
    }
    return col;
  }
}

bool TB_SCANNER(_external_scanner_scan)(void *payload, TSLexer *lexer,
                                              const bool *valid) {
  Scanner *s = (Scanner *)payload;

  bool error_recovery = valid[NEWLINE] && valid[INDENT] && valid[DEDENT] &&
                        valid[STRING_START] && valid[STRING_CONTENT] &&
                        valid[STRING_END] && valid[LINE_START];

  // Queued dedents first: one token per call.
  if (s->pending_dedents > 0 && valid[DEDENT]) {
    s->pending_dedents--;
    s->line_start_pending = true;
    lexer->result_symbol = DEDENT;
    return true;
  }

  // Inside a string, content/end outrank everything else — and nothing may
  // be skipped, because spaces are content.
  if (s->string_count > 0 && (valid[STRING_CONTENT] || valid[STRING_END]) &&
      !error_recovery) {
    return scan_string_body(lexer, s, valid);
  }
  if (s->string_count > 0 && error_recovery) {
    // The parser is recovering inside a string; drop the state so we do
    // not resume a string that no longer exists.
    s->string_count = 0;
  }

  // Anchor for zero-width tokens emitted before any skipping.
  lexer->mark_end(lexer);

  // Where this call began, and whether it has crossed a line boundary yet.
  // Together these say whether a newline we reach TERMINATES a line that had
  // content on it, or merely ends a BLANK one. CPython emits NEWLINE only
  // for the former; blank lines get NL, which the parser never sees.
  //
  // Deliberately local rather than scanner state: tree-sitter persists
  // scanner state only after a call that RETURNS A TOKEN, so a flag cleared
  // on the (overwhelmingly common) `return false` path is simply lost.
  uint32_t entry_column = lexer->get_column(lexer);
  bool line_crossed = false;

  // One unified skip pass. The external scanner is consulted exactly once
  // per token request, BEFORE the internal lexer touches extras — so every
  // kind of whitespace in front of a token we own must be handled here:
  // horizontal space, backslash continuations, and newlines in states
  // where the newline is not a message (inside brackets, blank lines
  // between the NEWLINE already emitted and the indent decision).
  for (;;) {
    int32_t c = lexer->lookahead;
    if (c == ' ' || c == '\t' || c == '\f') {
      skip(lexer);
    } else if (c == '\\') {
      skip(lexer);
      if (lexer->lookahead == '\r') skip(lexer);
      if (lexer->lookahead == '\n') skip(lexer);
    } else if (c == '\n' || c == '\r') {
      // A NEWLINE only where a line with CONTENT ends: this call started
      // past column 0 and has not already crossed a boundary. Otherwise the
      // line is blank and gets skipped.
      //
      // Emitting for a blank line killed the continuation fork after an
      // INLINE suite. `if True: pass` opens no indent level, so a blank or
      // comment line before the `else` never reached the INDENT/DEDENT
      // branch that consumes such lines — it fell to here, and the spurious
      // NEWLINE committed the shared GLR token stream before `else` could
      // attach. With a block suite the DEDENT path absorbed it, which is why
      // only the inline form was affected.
      if (valid[NEWLINE] && entry_column > 0 && !line_crossed) {
        if (lexer->lookahead == '\r') advance(lexer);
        if (lexer->lookahead == '\n') advance(lexer);
        lexer->mark_end(lexer);
        s->line_start_pending = true;
        lexer->result_symbol = NEWLINE;
        return true;
      }
      line_crossed = true;
      // Not a message here: a blank line before an indent decision, or a
      // newline inside brackets. Plain whitespace either way — but only
      // the former is a line boundary: inside brackets the logical line
      // continues, and `)` at the start of a physical line must not be
      // offered a LINE_START that would sever `) from e`.
      skip(lexer);
      if (valid[INDENT] || valid[DEDENT]) s->line_start_pending = true;
    } else {
      break;
    }
  }

  // A comment is a node, not skippable here. Decline; the internal lexer
  // lexes it as an extra and we are consulted again on the far side.
  //
  // But first close any block the comment is NOT inside. Declining
  // unconditionally put every trailing comment INSIDE the block above it,
  // because the DEDENT is only emitted on the far side -- so
  //
  //     if x:
  //       body()
  //
  //     # a comment about what comes next
  //     if y:
  //
  // gave the first `if_statement` a span running to the second one. Asking
  // for that statement's source text handed back a comment that is not part
  // of it, which matters most to anything that re-emits code.
  //
  // CPython's tokenizer ignores comment lines for indentation, and so do we:
  // the decision uses the indentation of the next line with actual CODE on
  // it, scanning past any run of comment and blank lines. A comment sitting
  // at the block's own indentation therefore stays inside it.
  if (lexer->lookahead == '#') {
    if (valid[DEDENT] && !error_recovery && s->line_start_pending &&
        s->indent_count > 0) {
      uint32_t col = next_code_column(lexer);
      if (col < s->indents[s->indent_count - 1]) {
        uint32_t pops = 0;
        while (s->indent_count > 0 && s->indents[s->indent_count - 1] > col) {
          s->indent_count--;
          pops++;
        }
        if (pops > 0) {
          s->pending_dedents = pops - 1;
          // No mark_end: this token is ZERO WIDTH and must not swallow the
          // comment it is closing in front of.
          lexer->result_symbol = DEDENT;
          return true;
        }
      }
    }
    return false;
  }

  // At EOF: close what remains — the final logical line, then the open
  // indent levels. Never during error recovery, where every symbol is
  // valid and a zero-width token would loop forever.
  if (lexer->lookahead == 0) {
    if (!error_recovery) {
      if (valid[DEDENT] && s->indent_count > 0) {
        s->indent_count--;
        s->line_start_pending = true;
        lexer->result_symbol = DEDENT;
        return true;
      }
      if (valid[NEWLINE]) {
        s->line_start_pending = true;
        lexer->result_symbol = NEWLINE;
        return true;
      }
    }
    return false;
  }

  // Indentation only means anything at the start of a line. Mid-line the
  // grammar may still OFFER dedent (a GLR fork that has already closed a
  // suite), and answering there would close blocks in the middle of an
  // expression — the guard is what keeps `""".strip()` inside its block.
  // These tokens CONSUME the blank lines they crossed (mark_end here, after
  // the skip pass): leaving a blank line unconsumed would let the next call
  // emit a NEWLINE for it, which the shared GLR token stream would force on
  // the fork that is waiting for an elif/else/except instead.
  if ((valid[INDENT] || valid[DEDENT]) && s->line_start_pending) {
    uint32_t col = lexer->get_column(lexer);
    uint32_t top = s->indent_count > 0 ? s->indents[s->indent_count - 1] : 0;
    if (col > top && valid[INDENT]) {
      if (s->indent_count < MAX_INDENTS) {
        s->indents[s->indent_count++] = (uint16_t)col;
      }
      lexer->mark_end(lexer);
      lexer->result_symbol = INDENT;
      return true;
    }
    if (col < top && valid[DEDENT]) {
      uint32_t pops = 0;
      while (s->indent_count > 0 && s->indents[s->indent_count - 1] > col) {
        s->indent_count--;
        pops++;
      }
      if (pops > 0) {
        s->pending_dedents = pops - 1;
        lexer->mark_end(lexer);
        lexer->result_symbol = DEDENT;
        return true;
      }
    }
    // Same level: no token; fall through, because the next token may still
    // be ours (a statement can begin with a string).
  }

  // A logical line can only start where the line genuinely starts: either
  // this call began at column 0, or the skip pass crossed a newline to get
  // here. Never before EOF, a blank line or a comment — emitting there
  // would commit the parser to a line that has no statement. And never
  // before a continuation keyword: at a block boundary the parser may be
  // choosing between an elif/else/except/finally clause and a new
  // statement, the token stream is shared across GLR forks, and emitting
  // LINE_START would kill the continuation fork. Those four are hard
  // keywords that can never begin a statement, so peeking them is safe.
  if (valid[LINE_START] && !error_recovery && s->line_start_pending &&
      lexer->lookahead != 0 && lexer->lookahead != '\n' &&
      lexer->lookahead != '\r' && lexer->lookahead != '#') {
    static const char *continuations[] = {"elif", "else", "except", "finally"};
    char word[9];
    int n = 0;
    while (n < 8 && ((lexer->lookahead >= 'a' && lexer->lookahead <= 'z'))) {
      word[n++] = (char)lexer->lookahead;
      advance(lexer);
    }
    word[n] = 0;
    bool ends_word = !(lexer->lookahead == '_' ||
                       (lexer->lookahead >= 'a' && lexer->lookahead <= 'z') ||
                       (lexer->lookahead >= 'A' && lexer->lookahead <= 'Z') ||
                       (lexer->lookahead >= '0' && lexer->lookahead <= '9'));
    if (ends_word) {
      for (unsigned k = 0; k < 4; k++) {
        if (strcmp(word, continuations[k]) == 0) return false;
      }
    }
    s->line_start_pending = false;
    lexer->result_symbol = LINE_START;
    return true;
  }

  if (valid[STRING_START] && !error_recovery) {
    int32_t c = lexer->lookahead;
    if (c == '\'' || c == '"' || c == 'r' || c == 'R' || c == 'b' ||
        c == 'B' || c == 'u' || c == 'U' || c == 'f' || c == 'F') {
      return scan_string_start(lexer, s);
    }
  }

  return false;
}

unsigned TB_SCANNER(_external_scanner_serialize)(void *payload,
                                                       char *buffer) {
  Scanner *s = (Scanner *)payload;
  unsigned i = 0;
  buffer[i++] = (char)s->line_start_pending;
  buffer[i++] = (char)s->pending_dedents;
  buffer[i++] = (char)s->indent_count;
  for (uint32_t k = 0; k < s->indent_count; k++) {
    memcpy(&buffer[i], &s->indents[k], sizeof(uint16_t));
    i += sizeof(uint16_t);
  }
  buffer[i++] = (char)s->string_count;
  for (uint32_t k = 0; k < s->string_count; k++) {
    buffer[i++] = (char)s->strings[k].flags;
    buffer[i++] = s->strings[k].quote;
  }
  return i;
}

void TB_SCANNER(_external_scanner_deserialize)(void *payload,
                                                     const char *buffer,
                                                     unsigned length) {
  Scanner *s = (Scanner *)payload;
  memset(s, 0, sizeof(Scanner));
  s->line_start_pending = true;
  if (length == 0) return;
  unsigned i = 0;
  s->line_start_pending = buffer[i++] != 0;
  s->pending_dedents = (uint8_t)buffer[i++];
  s->indent_count = (uint8_t)buffer[i++];
  for (uint32_t k = 0; k < s->indent_count; k++) {
    memcpy(&s->indents[k], &buffer[i], sizeof(uint16_t));
    i += sizeof(uint16_t);
  }
  s->string_count = (uint8_t)buffer[i++];
  for (uint32_t k = 0; k < s->string_count; k++) {
    s->strings[k].flags = (uint8_t)buffer[i++];
    s->strings[k].quote = buffer[i++];
  }
}

void *TB_SCANNER(_external_scanner_create)(void) {
  Scanner *s = calloc(1, sizeof(Scanner));
  s->line_start_pending = true; // the first line of a file starts a line
  return s;
}

void TB_SCANNER(_external_scanner_destroy)(void *payload) {
  free(payload);
}
