#pragma once

#include "../ast/ast.h"

// larping Rust's Result<Ok(()), Err> again
typedef enum { EVAL_OK, EVAL_ERROR } EvalStatus;

typedef enum {
  EVAL_ERR_DIV_BY_ZERO,
} EvalErrKind;

// this evaluator will be position-unaware
// if it fails, it fails. you won't know where and rarely why
typedef struct {
  EvalStatus status;
  union {
    double value;
    EvalErrKind error;
  };
} EvalResut;

EvalResut evaluate(ASTNode *nodes);
