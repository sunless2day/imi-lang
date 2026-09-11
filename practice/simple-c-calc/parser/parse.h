#pragma once

#include "../ast/ast.h"
#include "../lexer/lexer.h"

// it's necessary to keep track of the position we're at all times
typedef struct {
  TokenList tokens;
  size_t pos;
} Parser;

// this function parses expressions and binds loosest
ASTNode *parse_expression(Parser *parser);

// gets called by parse_expression to parse terms (e.g. `1 + 3`)
ASTNode *parse_term(Parser *parser);

// binds tightest but also calls parse_expression if parenthesis are found
ASTNode *parse_factor(Parser *parser);
