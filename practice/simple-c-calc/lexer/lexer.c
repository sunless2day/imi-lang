#include "./lexer.h"
#include <ctype.h>
#include <errno.h>
#include <stdbool.h>
#include <stddef.h>
#include <stdio.h>
#include <stdlib.h>

// checks if the array needs to increase its capacity (when len == cap) and does
// so then pushes the given token to the end of the array (simply checks the
// current length to find the last token)
// returns true for success and false for failure
static bool push_token(TokenList *list, Token token) {
  if (list->len == list->cap) {
    // if cap is 0, it sets it to 16, otherwise it doubles it
    size_t new_cap = list->cap == 0 ? 16 : list->cap * 2;

    Token *temp = realloc(list->tokens, new_cap * sizeof *list->tokens);

    if (!temp) {
      perror("Lexer memory reallocation failed");
      return false;
    }
    list->tokens = temp;
    list->cap = new_cap;
  }

  // post increment only increments the length AFTER assigning the token
  list->tokens[list->len++] = token;

  return true;
}

LexResult lex(const char *source) {
  TokenList tokens = {.tokens = NULL, .len = 0, .cap = 0};

  size_t i = 0;

  while (source[i] != '\0') {
    // we cast to `unsigned char` to safely handle extended ascii chars
    if (isspace((unsigned char)source[i])) {
      i++;
      continue;
    }

    bool push_success = true;
    bool matched = true;

    // matching single character symbols
    switch (source[i]) {
    case '+':
      push_success =
          push_token(&tokens, (Token){.kind = TOKEN_PLUS, .value = 0});
      break;
    case '-':
      push_success =
          push_token(&tokens, (Token){.kind = TOKEN_MINUS, .value = 0});
      break;
    case '*':
      push_success =
          push_token(&tokens, (Token){.kind = TOKEN_STAR, .value = 0});
      break;
    case '/':
      push_success =
          push_token(&tokens, (Token){.kind = TOKEN_SLASH, .value = 0});
      break;
    case '(':
      push_success =
          push_token(&tokens, (Token){.kind = TOKEN_LPAREN, .value = 0});
      break;
    case ')':
      push_success =
          push_token(&tokens, (Token){.kind = TOKEN_RPAREN, .value = 0});
      break;
    default:
      matched = false;
      break;
    }

    if (matched) {
      if (!push_success)
        goto memory_error;
      i++;
      continue;
    }

    // same cast to prevent UBs
    if (isdigit((unsigned char)source[i])) {
      char *next_char;

      errno = 0;

      // the good thing with strtod is that it lets us know where the number
      // ends
      double val = strtod(&source[i], &next_char);

      // just in case strtod failed to consume the integer
      if (errno == ERANGE) {
        fprintf(stderr, "Lexer error: number overflow at position %zu\n", i);
        free(tokens.tokens);
        return (LexResult){.status = LEX_ERROR,
                           .error = {.position = i, .character = source[i]}};
      }

      if (!push_token(&tokens, (Token){.kind = TOKEN_NUMBER, .value = val}))
        goto memory_error;

      // we jump to the end of the number strtod consumed and keep iterating
      // from there
      i += (next_char - &source[i]);
      continue;
    }

    LexError error = {.position = i, .character = source[i]};
    fprintf(stderr, "Lexer error: unexpected token symbol '%c'\n", source[i]);
    free(tokens.tokens);
    return (LexResult){.status = LEX_ERROR, .error = error};
  }

  // very important to append the EOF token at the end
  if (!push_token(&tokens, (Token){.kind = TOKEN_EOF, .value = 0}))
    goto memory_error;

  return (LexResult){.status = LEX_OK, .list = tokens};

memory_error:
  free(tokens.tokens);
  return (LexResult){.status = LEX_ERROR,
                     .error = {.position = i, .character = source[i]}};
}

// hehe, it says foid
foid lex_debug(LexResult tokens) {
  for (size_t i = 0; i < tokens.list.len; i++) {
    if (tokens.list.tokens[i].kind == TOKEN_NUMBER) {
      printf("NUMBER(%lf)\n", tokens.list.tokens[i].value);
    } else if (tokens.list.tokens[i].kind == TOKEN_PLUS) {
      printf("PLUS\n");
    } else if (tokens.list.tokens[i].kind == TOKEN_MINUS) {
      printf("MINUS\n");
    } else if (tokens.list.tokens[i].kind == TOKEN_STAR) {
      printf("MULTIPLICATION\n");
    } else if (tokens.list.tokens[i].kind == TOKEN_SLASH) {
      printf("DIVISION\n");
    } else if (tokens.list.tokens[i].kind == TOKEN_LPAREN) {
      printf("OPENING PARENTHESIS\n");
    } else if (tokens.list.tokens[i].kind == TOKEN_RPAREN) {
      printf("CLOSING PARENTHESIS\n");
    } else {
      printf("END OF FILE\n");
    }
  }
}
