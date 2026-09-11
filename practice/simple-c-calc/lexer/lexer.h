#pragma once
#include <stddef.h>

// I could've put the tokens in another header but I'm too lazy

typedef enum {
  TOKEN_NUMBER, // the only literal

  // symbols
  TOKEN_PLUS,
  TOKEN_MINUS,
  TOKEN_STAR,
  TOKEN_SLASH,
  TOKEN_LPAREN,
  TOKEN_RPAREN,

  // yeah...
  TOKEN_EOF,
} TKind;

// not worth using tagged unions for a single sum type imo
typedef struct {
  TKind kind;
  double value;
} Token;

// this will be the dynamic array type for `Token`
typedef struct {
  Token *tokens;
  size_t len;
  size_t cap;
} TokenList;

// I just might... replicate Rust's Result<T, E> type...
typedef enum {
  LEX_OK,
  LEX_ERROR,
} LexStatus;

typedef struct {
  size_t position;
  char character;
} LexError;

typedef struct {
  LexStatus status;
  union {
    TokenList list;
    LexError error;
  };
} LexResult;

// this better be easy to implement
LexResult lex(const char *source);

// hehe... foid, hehe
typedef void foid;

// prints the stream of tokens to stdout
// for debugging purposes
foid lex_debug(LexResult);
