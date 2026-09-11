#include "parse.h"
#include <stdio.h>
#include <stdlib.h>

// if the kind of token we are looking at happens to be a + or -,
// we enter a loop where we create a new node out of the existing left/right
// nodes
ASTNode *parse_expression(Parser *parser) {
  // parse_term has higher presedence,
  // so it gets called as soon as we enter this function
  ASTNode *left = parse_term(parser);

  if (!left)
    return NULL;

  while (parser->tokens.tokens[parser->pos].kind == TOKEN_PLUS ||
         parser->tokens.tokens[parser->pos].kind == TOKEN_MINUS) {
    BinOp op = parser->tokens.tokens[parser->pos++].kind == TOKEN_PLUS
                   ? AST_ADD
                   : AST_SUB;

    ASTNode *right = parse_term(parser);

    if (!right)
      return NULL;

    // initializes the new node with the size of ASTNode
    ASTNode *new_node = malloc(sizeof *new_node);

    if (!new_node)
      return NULL;

    *new_node =
        (ASTNode){.kind = AST_BINARY,
                  .data = {.binary = {.left = left, .op = op, .right = right}}};

    left = new_node;
  }
  return left;
}

// same behavior as parse_expression, no need to explain much here
ASTNode *parse_term(Parser *parser) {
  ASTNode *left = parse_factor(parser);

  if (!left)
    return NULL;

  while (parser->tokens.tokens[parser->pos].kind == TOKEN_STAR ||
         parser->tokens.tokens[parser->pos].kind == TOKEN_SLASH) {
    BinOp op = parser->tokens.tokens[parser->pos++].kind == TOKEN_STAR
                   ? AST_MUL
                   : AST_DIV;

    ASTNode *right = parse_factor(parser);

    if (!right)
      return NULL;

    ASTNode *new_node = malloc(sizeof *new_node);

    if (!new_node)
      return NULL;

    *new_node =
        (ASTNode){.kind = AST_BINARY,
                  .data = {.binary = {.left = left, .op = op, .right = right}}};

    left = new_node;
  }

  return left;
}

ASTNode *parse_factor(Parser *parser) {
  Token current_token = parser->tokens.tokens[parser->pos];

  if (current_token.kind == TOKEN_NUMBER) {
    parser->pos++; // we consume the number token

    ASTNode *node = malloc(sizeof *node);

    if (!node)
      return NULL;

    *node =
        (ASTNode){.kind = AST_NUMBER, .data = {.number = current_token.value}};

    return node;
  }

  if (current_token.kind == TOKEN_LPAREN) {
    parser->pos++; // consumes '('

    ASTNode *node = parse_expression(parser);

    if (!node)
      return NULL;

    if (parser->tokens.tokens[parser->pos].kind == TOKEN_RPAREN) {
      parser->pos++; // consumes ')'
      return node;
    } else {
      fprintf(stderr, "Parser error: expected closing parenthesis\n");
      return NULL;
    }
  }

  fprintf(stderr, "Parser error: expected a number or token, got %d\n",
          current_token.kind);
  return NULL;
}
