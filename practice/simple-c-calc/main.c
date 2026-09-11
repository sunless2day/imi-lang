#include "ast/ast.h"
#include "eval/eval.h"
#include "lexer/lexer.h"
#include "parser/parse.h"
#include <stdio.h>
#include <stdlib.h>

// the main function is very simple
// runs the lexer, parser and evaluator
// checks if any of the functions failed at any point
// yeah, pretty much that

int main(int argc, char **argv) {
  if (argc < 2) {
    fprintf(stderr, "usage: %s <math expression>\n", argv[0]);
    return 1;
  }

  LexResult tokens = lex(argv[1]);

  if (tokens.status == LEX_ERROR) {
    fprintf(stderr, "something went wrong at position: %zu, character: %c\n",
            tokens.error.position, tokens.error.character);
    return 1;
  }

  Parser parser = {.tokens = tokens.list, .pos = 0};
  ASTNode *ast = parse_expression(&parser);

  if (!ast) {
    fprintf(stderr, "the parser failed to do it's job\n");
    free(tokens.list.tokens);
    return 1;
  }

  if (parser.tokens.tokens[parser.pos].kind != TOKEN_EOF) {
    fprintf(stderr, "unexpected trailing input at token %zu\n",
            parser.pos);
    free(tokens.list.tokens);
    return 1;
  }

  EvalResut result = evaluate(ast);

  if (result.status == EVAL_ERROR) {
    fprintf(stderr, "evaluator error: division by zero is not allowed");
  } else {
    printf("%.2lf\n", result.value);
  }

  free(tokens.list.tokens);
  return result.status == EVAL_OK ? 0 : 1;
}
