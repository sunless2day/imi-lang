use super::{
    Interpreter,
    types::{contains_float, infer_type, promote_ints_to_floats, type_name},
    value::{Value, as_f64},
};
use crate::{
    LangError,
    ast::{BinaryOp, Expr, ExprNode, Literal, UnaryOp},
    environment::Environment,
};
use std::rc::Rc;

impl Interpreter {
    pub(super) fn eval(
        &mut self,
        expr: &ExprNode,
        env: &mut Environment,
    ) -> Result<Value, LangError> {
        match &expr.node {
            Expr::Literal(lit) => self.eval_literal(lit, env),
            Expr::Variable(name) => match env.get(name) {
                Some(Value::Uninitialized) => Err(expr.error(format!(
                    "Variable '{}' used before being assigned a value.",
                    name
                ))),
                Some(v) => Ok(v.clone()),
                None => Err(expr.error(format!("Undeclared variable '{}'.", name))),
            },
            Expr::Unary(op, operand) => self.eval_unary(op, operand, env),
            Expr::Binary(l, op, r) => self.eval_binary(l, op, r, env),
            Expr::Call(name, args) => self.eval_call(name, args, env, expr),
            Expr::Index(target, index) => self.eval_index(target, index, env),
            Expr::MethodCall(target, method, args) => {
                self.eval_method_call(target, method, args, env)
            }
        }
    }

    fn eval_literal(&mut self, lit: &Literal, env: &mut Environment) -> Result<Value, LangError> {
        match lit {
            Literal::Int(n) => Ok(Value::Int(*n)),
            Literal::Float(f) => Ok(Value::Float(*f)),
            // str is a heap-allocated type, so we wrap it around Rc::new()
            Literal::Str(s) => Ok(Value::Str(Rc::new(s.clone()))),
            Literal::Bool(b) => Ok(Value::Bool(*b)),
            Literal::Array(elements) => {
                let mut values = Vec::with_capacity(elements.len());
                for e in elements {
                    values.push(self.eval(e, env)?);
                }

                // promotion runs BEFORE the same-type check on purpose:
                // this is what lets `[1, 2.0]` coerce to array[float] and pass, while `[1, "a"]` still errors.
                // swapping these two blocks would silently break mixedi nt/float arrays
                if contains_float(&values) {
                    promote_ints_to_floats(&mut values);
                }

                if let Some(first) = values.first() {
                    let first_type = infer_type(first).map_err(|e| elements[0].error(e))?;
                    for (i, item) in values.iter().enumerate().skip(1) {
                        let item_type = infer_type(item).map_err(|e| elements[i].error(e))?;
                        if item_type != first_type {
                            return Err(elements[i].error(format!(
                                "Array elements must have the same type, found {} and {}.",
                                first_type, item_type
                            )));
                        }
                    }
                }
                // array is also a heap-allocated type, same thing as str
                Ok(Value::Array(Rc::new(values)))
            }
        }
    }

    fn eval_unary(
        &mut self,
        op: &UnaryOp,
        operand: &ExprNode,
        env: &mut Environment,
    ) -> Result<Value, LangError> {
        let val = self.eval(operand, env)?;
        match (op, val) {
            (UnaryOp::Neg, Value::Int(n)) => n.checked_neg().map(Value::Int).ok_or_else(|| {
                operand.error(format!(
                    "integer overflow: -({}) cannot be represented as int",
                    n
                ))
            }),
            (UnaryOp::Neg, Value::Float(f)) => Ok(Value::Float(-f)),
            (UnaryOp::Neg, other) => Err(operand.error(format!(
                "Cannot negate a value of type {}.",
                type_name(&other)
            ))),
            (UnaryOp::Not, Value::Bool(b)) => Ok(Value::Bool(!b)),
            (UnaryOp::Not, other) => {
                Err(operand.error(format!("'not' requires bool, found {}.", type_name(&other))))
            }
        }
    }

    fn eval_binary(
        &mut self,
        l: &ExprNode,
        op: &BinaryOp,
        r: &ExprNode,
        env: &mut Environment,
    ) -> Result<Value, LangError> {
        // short-circuiting: we stop performing unnecessary computation if the end result is predictable.
        // the right operand is only evaluated when the left one isn't definitive.
        // but the left operand is always evaluated (it has to be)
        // `true or false` short-circuits because the left operand already makes the whole expression return true,
        // so computing the right operand won't make a difference.
        // `true and false`: since the left operand `true` can't determine the value of the whole expression,
        // the right operand must be evaluated
        if matches!(op, BinaryOp::And) {
            return match self.eval(l, env)? {
                Value::Bool(false) => Ok(Value::Bool(false)),
                Value::Bool(true) => match self.eval(r, env)? {
                    Value::Bool(b) => Ok(Value::Bool(b)),
                    other => {
                        Err(r.error(format!("'and' requires bool, found {}.", type_name(&other))))
                    }
                },
                other => Err(l.error(format!("'and' requires bool, found {}.", type_name(&other)))),
            };
        }
        if matches!(op, BinaryOp::Or) {
            return match self.eval(l, env)? {
                Value::Bool(true) => Ok(Value::Bool(true)),
                Value::Bool(false) => match self.eval(r, env)? {
                    Value::Bool(b) => Ok(Value::Bool(b)),
                    other => {
                        Err(r.error(format!("'or' requires bool, found {}.", type_name(&other))))
                    }
                },
                other => Err(l.error(format!("'or' requires bool, found {}.", type_name(&other)))),
            };
        }

        let left = self.eval(l, env)?;
        let right = self.eval(r, env)?;

        use BinaryOp::*;
        match op {
            Add | Sub | Mul | Div | Pow | Mod => self
                .eval_arithmetic(op, left, right)
                .map_err(|e| l.error(e)),
            Eq | NotEq | Lt | Gt | LtEq | GtEq => self
                .eval_comparison(op, left, right)
                .map_err(|e| l.error(e)),
            And | Or => Err(l.error(
                "Internal evaluator error: logical operator reached ordinary binary evaluation.",
            )),
        }
    }

    /// just an arithmetic helper: takes values, returns a value or a plain error String.
    /// integer arithmetic stays checked all the way through, including negation/division edge cases,
    /// so bad imi arithmetic becomes a LangError instead of a Rust panic.
    pub(super) fn eval_arithmetic(
        &self,
        op: &BinaryOp,
        left: Value,
        right: Value,
    ) -> Result<Value, String> {
        use BinaryOp::*;

        // str + str concatenates str + anything-else is rejected
        // (no implicit int-to-str, I implemented `format` for that)
        if let Add = op {
            if let (Value::Str(a), Value::Str(b)) = (&left, &right) {
                return Ok(Value::Str(Rc::new(format!("{}{}", a, b))));
            }
            if matches!(left, Value::Str(_)) || matches!(right, Value::Str(_)) {
                return Err(format!(
                    "Cannot add {} and {}.",
                    type_name(&left),
                    type_name(&right)
                ));
            }
        }

        match (&left, &right) {
            // int/int stays int. every operation here is guarded because i64 has a
            // slightly evil edge case beyond the obvious +/* overflow: i64::MIN / -1
            // cannot be represented. Rust also panics on i64::MIN % -1 even though the
            // mathematical remainder is simply 0, so the integer helper handles that too.
            (Value::Int(a), Value::Int(b)) => self.eval_int_arithmetic(op, *a, *b).map(Value::Int),

            // if either operand is a float, both take the float path.
            // this is imi's normal mixed numeric behavior.
            (Value::Int(_), Value::Float(_))
            | (Value::Float(_), Value::Int(_))
            | (Value::Float(_), Value::Float(_)) => {
                let a = as_f64(&left)?;
                let b = as_f64(&right)?;

                let result = match op {
                    Add => a + b,
                    Sub => a - b,
                    Mul => a * b,
                    Div => {
                        if b == 0.0 {
                            return Err("Division by zero is not allowed.".to_string());
                        }
                        a / b
                    }
                    Mod => {
                        if b == 0.0 {
                            return Err("Modulo by zero is not allowed.".to_string());
                        }
                        a % b
                    }
                    Pow => a.powf(b),
                    _ => {
                        return Err("Internal evaluator error: non-arithmetic operator reached arithmetic evaluation."
                            .to_string());
                    }
                };

                Ok(Value::Float(result))
            }

            _ => Err(format!(
                "Cannot apply arithmetic to {} and {}.",
                type_name(&left),
                type_name(&right)
            )),
        }
    }

    /// integer-only arithmetic lives here so every i64 operation follows the same
    /// checked-overflow policy instead of some operations accidentally using Rust's
    /// panic/wrap behavior.
    fn eval_int_arithmetic(&self, op: &BinaryOp, a: i64, b: i64) -> Result<i64, String> {
        use BinaryOp::*;

        let overflow = || {
            format!(
                "integer overflow: {} {:?} {} cannot be represented as int.",
                a, op, b
            )
        };

        match op {
            Add => a.checked_add(b).ok_or_else(|| overflow()),
            Sub => a.checked_sub(b).ok_or_else(|| overflow()),
            Mul => a.checked_mul(b).ok_or_else(|| overflow()),
            Div => {
                if b == 0 {
                    return Err("Division by zero is not allowed.".to_string());
                }
                a.checked_div(b).ok_or_else(|| overflow())
            }
            Mod => {
                if b == 0 {
                    return Err("Modulo by zero is not allowed.".to_string());
                }
                // Rust treats MIN % -1 as an overflow panic because the paired division overflows,
                // but imi only needs the remainder here, which is exactly 0.
                if a == i64::MIN && b == -1 {
                    return Ok(0);
                }
                Ok(a % b)
            }
            Pow => {
                if b < 0 {
                    return Err(
                        "Integer exponent must be non-negative. Use floats for negative exponents."
                            .to_string(),
                    );
                }

                // checked_pow wants u32. converting explicitly avoids exponent truncation.
                let exp = u32::try_from(b).map_err(|_| {
                    format!("integer exponent {} is too large (max {}).", b, u32::MAX)
                })?;
                a.checked_pow(exp).ok_or_else(|| overflow())
            }
            _ => unreachable!("yo idk bro, check line 588"),
        }
    }

    /// comparison helper, same deal as eval_arithmetic: no spans in here,
    /// the caller attaches them.
    fn eval_comparison(&self, op: &BinaryOp, left: Value, right: Value) -> Result<Value, String> {
        use BinaryOp::*;

        let left_is_str = matches!(left, Value::Str(_));
        let right_is_str = matches!(right, Value::Str(_));
        let left_is_num = matches!(left, Value::Int(_) | Value::Float(_));
        let right_is_num = matches!(right, Value::Int(_) | Value::Float(_));

        // equality between a str and a number is simply false (not an errors),
        // but ordering them is an error. "5" < 3 genuinely makes no sense
        if (left_is_str && right_is_num) || (left_is_num && right_is_str) {
            return match op {
                Eq => Ok(Value::Bool(false)),
                NotEq => Ok(Value::Bool(true)),
                _ => Err("Cannot order a string and a number. Ordering is not supported between numbers and strings.".to_string()),
            };
        }

        let result = match (&left, &right) {
            (Value::Int(a), Value::Int(b)) => match op {
                Eq => a == b,
                NotEq => a != b,
                Lt => a < b,
                Gt => a > b,
                LtEq => a <= b,
                GtEq => a >= b,
                _ => unreachable!("something went wrong when ordering integers"),
            },

            // mixed int/float comparisons intentionally go through f64 because a float
            // is already involved in the operation.
            (Value::Int(_), Value::Float(_))
            | (Value::Float(_), Value::Int(_))
            | (Value::Float(_), Value::Float(_)) => {
                let a = as_f64(&left)?;
                let b = as_f64(&right)?;
                match op {
                    Eq => a == b,
                    NotEq => a != b,
                    Lt => a < b,
                    Gt => a > b,
                    LtEq => a <= b,
                    GtEq => a >= b,
                    _ => unreachable!("something went wrong when ordering integers and floats"),
                }
            }

            (Value::Str(a), Value::Str(b)) => match op {
                Eq => a == b,
                NotEq => a != b,
                Lt | Gt | LtEq | GtEq => return Err("Cannot order a string with another string. Strings only support equality comparisons.".to_string()),
                _ => unreachable!("something went wrong when ordering strings"),
            },

            (Value::Bool(a), Value::Bool(b)) => match op {
                Eq => a == b,
                NotEq => a != b,
                _ => return Err("Ordering is not defined for bool.".to_string()),
            },

            _ => {
                return Err(format!(
                    "cannot compare {} and {}",
                    type_name(&left),
                    type_name(&right)
                ));
            }
        };

        Ok(Value::Bool(result))
    }
}
