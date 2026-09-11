// I won't even bother explaining this header,
// it's literally what it reads as
#pragma once

typedef enum {
  AST_NUMBER,
  AST_BINARY,
} AstKind;

typedef enum { AST_ADD, AST_SUB, AST_MUL, AST_DIV } BinOp;

typedef struct ASTNode {
  AstKind kind;
  union {
    double number;

    struct {
      BinOp op;
      struct ASTNode *left;
      struct ASTNode *right;
    } binary;
  } data;
} ASTNode;
