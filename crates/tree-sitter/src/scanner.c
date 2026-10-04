// The block comments of IEC 61131-3 Table 3, `(* *)` and `/* */`, as one
// token each. They nest, `(* (* NESTED *) *)`, which no regular expression
// can count, so they are read here. Each style counts its own delimiters
// only: a `/*` inside a `(* *)` is text, and so is a `//`. A comment that
// is not closed before the end of the input is no token: the parser
// reports the `(*` where it starts and reads on, where the rest of the
// file used to become the comment, with the missing `*)` at its end.
//
// A pragma rk has no rule for, `{attribute 'hide'}`, is one token too, up
// to its `}`, so a `//` or a `}` inside a quoted string is text. The
// compiler reports it (E1510). A pragma whose first word is one of rk's
// is left to the grammar, which parses it, or refuses it where it cannot
// stand.

#include "tree_sitter/parser.h"

#include <stdbool.h>
#include <wctype.h>

#include <string.h>

enum TokenType {
  C_STYLE_COMMENT,
  PASCAL_STYLE_COMMENT,
  PRAGMA,
};

// The first words of the pragmas the grammar has a rule for.
static const char *const KNOWN_PRAGMAS[] = {
    "test", "once", "export", "extern", "wasm", "allow", "warn", "info",
};

static bool is_word_char(int32_t c) {
  return (c >= 'a' && c <= 'z') || (c >= 'A' && c <= 'Z') || (c >= '0' && c <= '9') || c == '_';
}

// After the `{`: a pragma rk has no rule for, read to its `}`. A known first
// word, or no `}` before the end of the input, is no token.
static bool scan_unknown_pragma(TSLexer *lexer) {
  lexer->advance(lexer, false);
  while (iswspace(lexer->lookahead)) {
    lexer->advance(lexer, false);
  }
  char word[16];
  unsigned length = 0;
  bool longer = false;
  while (is_word_char(lexer->lookahead)) {
    if (length < sizeof(word) - 1) {
      word[length++] = (char)lexer->lookahead;
    } else {
      longer = true;
    }
    lexer->advance(lexer, false);
  }
  word[length] = '\0';
  if (!longer) {
    for (unsigned i = 0; i < sizeof(KNOWN_PRAGMAS) / sizeof(KNOWN_PRAGMAS[0]); i++) {
      if (strcmp(word, KNOWN_PRAGMAS[i]) == 0) {
        return false;
      }
    }
  }
  while (!lexer->eof(lexer)) {
    int32_t c = lexer->lookahead;
    lexer->advance(lexer, false);
    if (c == '}') {
      lexer->result_symbol = PRAGMA;
      return true;
    }
    // A quoted string, with the `$` escapes of IEC 61131-3 Table 6.
    if (c == '\'' || c == '"') {
      while (!lexer->eof(lexer) && lexer->lookahead != c) {
        if (lexer->lookahead == '$') {
          lexer->advance(lexer, false);
        }
        lexer->advance(lexer, false);
      }
      if (lexer->eof(lexer)) {
        return false;
      }
      lexer->advance(lexer, false);
    }
  }
  return false;
}

void *tree_sitter_rk_external_scanner_create(void) { return NULL; }

void tree_sitter_rk_external_scanner_destroy(void *payload) { (void)payload; }

// No state: every comment is read in one call.
unsigned tree_sitter_rk_external_scanner_serialize(void *payload, char *buffer) {
  (void)payload;
  (void)buffer;
  return 0;
}

void tree_sitter_rk_external_scanner_deserialize(void *payload, const char *buffer,
                                                 unsigned length) {
  (void)payload;
  (void)buffer;
  (void)length;
}

bool tree_sitter_rk_external_scanner_scan(void *payload, TSLexer *lexer,
                                          const bool *valid_symbols) {
  (void)payload;
  if (!valid_symbols[C_STYLE_COMMENT] && !valid_symbols[PASCAL_STYLE_COMMENT] &&
      !valid_symbols[PRAGMA]) {
    return false;
  }

  while (iswspace(lexer->lookahead)) {
    lexer->advance(lexer, true);
  }

  if (lexer->lookahead == '{') {
    return valid_symbols[PRAGMA] && scan_unknown_pragma(lexer);
  }

  // `(` for `(* *)`, `/` for `/* */`: the character before the `*` that
  // opens, and the one after the `*` that closes.
  int32_t open;
  int32_t close;
  enum TokenType token;
  if (lexer->lookahead == '(') {
    open = '(';
    close = ')';
    token = PASCAL_STYLE_COMMENT;
  } else if (lexer->lookahead == '/') {
    open = '/';
    close = '/';
    token = C_STYLE_COMMENT;
  } else {
    return false;
  }
  if (!valid_symbols[token]) {
    return false;
  }
  lexer->advance(lexer, false);
  // A `(` or a `/` alone, or `//`: the grammar's own tokens.
  if (lexer->lookahead != '*') {
    return false;
  }
  lexer->advance(lexer, false);

  unsigned depth = 1;
  while (!lexer->eof(lexer)) {
    int32_t c = lexer->lookahead;
    lexer->advance(lexer, false);
    if (c == '*' && lexer->lookahead == close) {
      lexer->advance(lexer, false);
      if (--depth == 0) {
        lexer->result_symbol = token;
        return true;
      }
    } else if (c == open && lexer->lookahead == '*') {
      lexer->advance(lexer, false);
      depth++;
    }
  }
  return false;
}
