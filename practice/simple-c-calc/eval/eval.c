#include "eval.h"

EvalResut evaluate(ASTNode *node) {
  // the base case for our evaluator is if it encounters an integer literal
  if (node->kind == AST_NUMBER) {
    return (EvalResut){.status = EVAL_OK, .value = node->data.number};
  }

  // we evaluate the left operand
  EvalResut left = evaluate(node->data.binary.left);
  // check for errors
  if (left.status == EVAL_ERROR)
    return left;

  // same thing for the right operand
  EvalResut right = evaluate(node->data.binary.right);
  if (right.status == EVAL_ERROR)
    return right;

  double l = left.value;
  double r = right.value;

  // the easy part:
  switch (node->data.binary.op) {
  case AST_ADD:
    return (EvalResut){.status = EVAL_OK, .value = l + r};
  case AST_SUB:
    return (EvalResut){.status = EVAL_OK, .value = l - r};
  case AST_MUL:
    return (EvalResut){.status = EVAL_OK, .value = l * r};
  case AST_DIV:
    if (r == 0)
      return (EvalResut){.status = EVAL_ERROR, .error = EVAL_ERR_DIV_BY_ZERO};
    return (EvalResut){.status = EVAL_OK, .value = l / r};
  }
}
