//! this is the runtime of my language.
//! the real engine doing everything while the program is running.
//! this module is large, indeed

//! new changes have been added.
//! I refactored the code a little and added some implementation-level optimizations.
//! heap-allocated values can now be passed by reference,
//! this is to avoid cloning heap data on every assignment or function call
//! but we still have to respect the previous semantics...
//! so I will try to implement this optimization
//! without changing the practical behavior of the language

// BTW
// documenting this module is still a pain...
// I tried to make relevant docs while omitting the mundane parts

use crate::{
    ast::{AssignOp, BinaryOp, Expr, ExprNode, Literal, Param, Stmt, StmtNode, Type, UnaryOp},
    environment::Environment,
    error::LangError,
};

use std::{
    collections::HashMap,
    io::{self, Write},
    rc::Rc,
    thread,
    time::{Duration, Instant},
};

/// every built-in function the evaluator implements.
/// the parser imports this to stop users from shadowing them,
/// so adding a builtin here automatically updates the parser's reserved list too
pub const BUILTINS: &[&str] = &[
    "print", "println", "format", "len", "strslice", "sleep", "type", "elapsed", "input", "parse",
    "exit", "fread", "fwrite",
];

/// how deep user recursion is allowed to go.
/// conservative on purpose: each call in imi costs a surprising amount of rust stack
/// (eval() itself recurses for every subexpression), and past this limit I'd rather
/// report a clean error than let the process die with a native stack overflow
const MAX_CALL_DEPTH: usize = 512;

/// every value the language can produce at runtime.
/// Void and Uninitialized look similar but are NOT the same thing:
/// - Void: the result of a function that returns nothing. it needs to exist so every call
///   has a value, but it's not meant to be stored or displayed
/// - Uninitialized: a declared-but-not-yet-assigned variable (`let x: int;`),
///   which is what lets us declare an immutable variable and later initialize it
#[derive(Debug, Clone, PartialEq)]
pub enum Value {
    Int(i64),
    Float(f64),
    Str(Rc<String>),
    Bool(bool),
    Array(Rc<Vec<Value>>),
    Void,
    Uninitialized,
}

/// how statements report control flow to whoever called them.
/// instead of unwinding the rust stack for break/continue/return, every statement
/// returns one of these as a plain value, and exec_stmt/exec_block propagate them
/// upward until the loop or function that owns them consumes them.
/// Return(None) is a bare `return;`, which becomes Value::Void at the call site
#[derive(Debug, Clone, PartialEq)]
enum Outcome {
    Normal,
    Break,
    Continue,
    Return(Option<Value>),
}

/// this is the destination of an index.
/// this can either produce an array element or a sub-string from a string (a single character)
enum MutPlace<'a> {
    ArrayElem(&'a mut Vec<Value>, Type),
    StrChar(&'a mut String),
}

/// functions will also go through this `Rc` optimization.
/// an entry is simply the function's parameters, optional return type and body.
/// using a struct here instead of a tuple makes the call path a little easier to read
/// without changing what a function actually stores.
struct FnEntry {
    params: Vec<Param>,
    ret_type: Option<Type>,
    body: Vec<StmtNode>,
}

/// functions belong to a different namespace than variables
/// this is particularly relevant since things like:
/// ```
/// fn foo() { return; }
///
/// let foo: int;
/// ```
/// are completely fine because the function `foo()` and variable `foo`
/// are in different namespaces and don't collide with each other
///
/// as you can see the hashmap simply consists of a `String` acting as the key
/// and the data mapped to that key are the parameters, return type and statements
/// of that function
///
/// now, we can wrap `FnEntry` in `Rc` so that every function call is a refcount bump instead of deep cloning the AST body.
/// should be easy to implement since function's are read-only after being written to for the first time
type FnTable = HashMap<String, Rc<FnEntry>>;

/// the interpreter state: the function table, the program timer, and the recursion depth
pub struct Interpreter {
    functions: FnTable,
    /// `Instant` is necessary for the function `elapsed()` to be supported
    start: Instant,
    /// current recursion depth, so runaway recursion dies as a LangError
    /// instead of a native stack overflow (which bypasses ALL of our error machinery)
    call_depth: usize,
}

impl Interpreter {
    /// in here we initialize the function table and start the timer
    /// we will call `self.start.elapsed()` whenever the interpreter
    /// encounters a function call to the in-built function `elapsed()`
    pub fn new() -> Self {
        Self {
            functions: HashMap::new(),
            start: Instant::now(),
            call_depth: 0,
        }
    }

    // this interpreter is multi-pass, and this is what each pass does:
    pub fn interpret(&mut self, program: &[StmtNode]) -> Result<(), LangError> {
        // pass 1: register every function declaration BEFORE running anything.
        // this is hoisting. it's what lets a function defined at the bottom of the
        // file be called from the top. the FnTable living on the interpreter (not
        // inside the environment) is also what makes recursion work for free
        for stmt in program {
            if let Stmt::FnDecl(name, params, ret, body) = &stmt.node {
                // same spirit as the parser's duplicate-parameter check: two
                // functions sharing a name would otherwise silently last-wins,
                // with the earlier declaration vanishing with no error at all
                if self.functions.contains_key(name) {
                    return Err(stmt.error(format!("Function '{}' is already declared.", name)));
                }
                self.functions.insert(
                    name.clone(),
                    Rc::new(FnEntry {
                        params: params.clone(),
                        ret_type: ret.clone(),
                        body: body.clone(),
                    }),
                );
            }
        }

        let mut env = Environment::new();

        // pass 2: run everything that isn't a function declaration.
        for stmt in program {
            if matches!(stmt.node, Stmt::FnDecl(..)) {
                continue;
            }
            self.exec_stmt(stmt, &mut env)?;
        }
        Ok(())
    }

    /// executes statements in a fresh scope, stopping at (and propagating) the first
    /// non-Normal Outcome. the scope is popped whether the block finished or broke out
    fn exec_block(
        &mut self,
        stmts: &[StmtNode],
        env: &mut Environment,
    ) -> Result<Outcome, LangError> {
        env.push_scope();

        let mut outcome = Outcome::Normal;
        let mut error = None;

        for stmt in stmts {
            match self.exec_stmt(stmt, env) {
                Ok(next) => {
                    outcome = next;
                    if !matches!(outcome, Outcome::Normal) {
                        break;
                    }
                }
                Err(err) => {
                    error = Some(err);
                    break;
                }
            }
        }

        // scope is popped even when a statement errors. right now a LangError aborts the whole
        // program anyway, but keeping the scope stack balanced makes this method more correct
        env.pop_scope();

        match error {
            Some(err) => Err(err),
            None => Ok(outcome),
        }
    }

    /// the statement dispatcher: each arm evaluates what it needs
    /// and reports its control flow back as an Outcome variant
    fn exec_stmt(&mut self, stmt: &StmtNode, env: &mut Environment) -> Result<Outcome, LangError> {
        let line = stmt.line;
        let col = stmt.col;
        match &stmt.node {
            Stmt::Let(name, ty_annotation, init) => {
                self.declare_checked(name, ty_annotation, init, false, stmt, env)?;
                Ok(Outcome::Normal)
            }
            Stmt::Var(name, ty_annotation, init) => {
                self.declare_checked(name, ty_annotation, init, true, stmt, env)?;
                Ok(Outcome::Normal)
            }
            Stmt::Assign(name, op, expr) => {
                let rhs = self.eval(expr, env)?;
                let new_value = self.apply_assign_op(env, name, op, rhs, stmt)?;
                env.assign(name, new_value, line, col)?;
                Ok(Outcome::Normal)
            }
            Stmt::IndexAssign(target, index, op, expr) => {
                self.exec_index_assign(target, index, op, expr, env)?;
                Ok(Outcome::Normal)
            }
            Stmt::Block(body) => self.exec_block(body, env),
            Stmt::If(cond, then_b, else_b) => {
                if self.eval_bool_condition(cond, env)? {
                    self.exec_block(then_b, env)
                } else if let Some(else_b) = else_b {
                    self.exec_block(else_b, env)
                } else {
                    Ok(Outcome::Normal)
                }
            }
            Stmt::While(cond, body) => {
                while self.eval_bool_condition(cond, env)? {
                    match self.exec_block(body, env)? {
                        Outcome::Break => break,
                        Outcome::Continue | Outcome::Normal => continue,
                        r @ Outcome::Return(_) => return Ok(r),
                    }
                }
                Ok(Outcome::Normal)
            }
            Stmt::Break => Ok(Outcome::Break),
            Stmt::Continue => Ok(Outcome::Continue),
            Stmt::Return(expr) => {
                let v = match expr {
                    Some(e) => Some(self.eval(e, env)?),
                    None => None,
                };
                Ok(Outcome::Return(v))
            }
            Stmt::ExprStmt(expr) => {
                self.eval(expr, env)?;
                Ok(Outcome::Normal)
            }
            // function declarations were all registered back in interpret()'s pass 1,
            // so executing one is a no-op
            Stmt::FnDecl(..) => Ok(Outcome::Normal),
        }
    }

    fn declare_checked(
        &mut self,
        name: &str,
        ty_annotation: &Option<Type>,
        init: &Option<ExprNode>,
        mutable: bool,
        stmt: &StmtNode,
        env: &mut Environment,
    ) -> Result<(), LangError> {
        let value = match init {
            Some(expr) => self.eval(expr, env)?,
            None => Value::Uninitialized,
        };

        let value = if let Some(t) = ty_annotation {
            coerce_value_to_type(value, t)
        } else {
            value
        };

        let ty = match (ty_annotation, &value) {
            (Some(t), v) if !matches!(v, Value::Uninitialized) => {
                if !value_matches_type(v, t) {
                    let actual_type = match infer_type(v) {
                        Ok(inferred) => format!("{}", inferred),
                        Err(e) => return Err(stmt.error(e)),
                    };
                    return Err(stmt.error(format!(
                        "'{}': declared as {} but initialized with {}.",
                        name, t, actual_type
                    )));
                }
                t.clone()
            }
            (Some(t), _) => t.clone(),
            (None, v) => infer_type(v).map_err(|e| stmt.error(e))?,
        };

        env.declare(name.to_string(), value, ty, mutable);
        Ok(())
    }

    fn eval_bool_condition(
        &mut self,
        expr: &ExprNode,
        env: &mut Environment,
    ) -> Result<bool, LangError> {
        // conditions must be strictly bool. no truthiness
        // this is to avoid javascript's bs conversions
        // `while 1 {}` is an error, on purpose
        match self.eval(expr, env)? {
            Value::Bool(b) => Ok(b),
            other => Err(expr.error(format!(
                "Condition must be of type bool, found {}.",
                type_name(&other)
            ))),
        }
    }

    fn eval(&mut self, expr: &ExprNode, env: &mut Environment) -> Result<Value, LangError> {
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
    fn eval_arithmetic(&self, op: &BinaryOp, left: Value, right: Value) -> Result<Value, String> {
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

    /// cool new helper below:
    /// evaluates call arguments strictly left to right.
    /// both ordinary function calls and array methods use this helper so they agree
    /// on evaluation order without copy-pasting the same loop.
    fn eval_arguments(
        &mut self,
        args: &[ExprNode],
        env: &mut Environment,
    ) -> Result<Vec<Value>, LangError> {
        let mut values = Vec::with_capacity(args.len());
        for arg in args {
            values.push(self.eval(arg, env)?);
        }
        Ok(values)
    }

    fn eval_call(
        &mut self,
        name: &str,
        args: &[ExprNode],
        env: &mut Environment,
        call_site: &ExprNode,
    ) -> Result<Value, LangError> {
        // all arguments are evaluated before dispatch, strictly left to right,
        // even for builtins that might not need them all
        let arg_values = self.eval_arguments(args, env)?;

        if BUILTINS.contains(&name) {
            self.eval_builtin(name, &arg_values, call_site)
        } else {
            self.call_user_function(name, arg_values, call_site)
        }
    }

    /// built-ins live inside an isolated method instead of sharing eval_call with user functions.
    /// keeping this as one method seemed like a necessary refactor for this huge mess of a module
    fn eval_builtin(
        &mut self,
        name: &str,
        arg_values: &[Value],
        call_site: &ExprNode,
    ) -> Result<Value, LangError> {
        // each arm does its own arity/type checking because imi has no static
        // function signatures to lean on at runtime.
        match name {
            "println" | "print" | "format" => {
                let s = self.do_format(arg_values, call_site)?;
                match name {
                    // println!/print! panic if stdout is closed (e.g. piping into `head`),
                    // so we write through io::stdout() and ignore the error instead,
                    // a closed stream is not worth crashing the whole interpreter over
                    "println" => {
                        let _ = writeln!(io::stdout(), "{}", s);
                    }
                    "print" => {
                        let _ = write!(io::stdout(), "{}", s);
                        io::stdout().flush().ok();
                    }
                    "format" => return Ok(Value::Str(Rc::new(s))),
                    _ => {
                        return Err(call_site.error(
                            "Internal evaluator error: invalid formatting builtin dispatch.",
                        ));
                    }
                }
                // functions like print and println have nothing relevant to return,
                // unlike format (that returns a formatted string).
                // we end up returning Void anyway to have something to return (see docs in Value)
                Ok(Value::Void)
            }
            "len" => match arg_values {
                [Value::Str(s)] => Ok(Value::Int(s.chars().count() as i64)),
                [Value::Array(items)] => Ok(Value::Int(items.len() as i64)),
                [v] => Err(call_site.error(format!(
                    "'len' expects a str or array, found {}.",
                    type_name(v)
                ))),
                _ => Err(call_site.error(format!(
                    "'len' expects 1 argument, got {}.",
                    arg_values.len()
                ))),
            },
            "strslice" => match arg_values {
                [Value::Str(s), Value::Int(start), Value::Int(end)] => {
                    let (start, end) = (*start, *end);
                    let chars: Vec<char> = s.chars().collect();
                    if start < 0 || end < start || end as usize > chars.len() {
                        return Err(call_site.error(format!(
                            "'strslice' range {}..{} out of bounds (str has {} characters).",
                            start,
                            end,
                            chars.len()
                        )));
                    }
                    // i'm pretty sure type casting an i64 to usize COULD cause trouble in very niche cases
                    // I trust the conditions above are enough to disallow any incoming bugs (fingers crossed)
                    Ok(Value::Str(Rc::new(
                        chars[start as usize..end as usize].iter().collect(),
                    )))
                }
                [Value::Str(_), a, b] => Err(call_site.error(format!(
                    "'strslice' expects (str, int, int), found ({}, {}).",
                    type_name(a),
                    type_name(b)
                ))),
                [v, _, _] => Err(call_site.error(format!(
                    "'strslice' expects a str as its first argument, found {}.",
                    type_name(v)
                ))),
                _ => Err(call_site.error(format!(
                    "'strslice' expects 3 arguments, got {}.",
                    arg_values.len()
                ))),
            },

            "sleep" => match arg_values {
                [v] => {
                    let secs = as_f64(v).map_err(|e| call_site.error(e))?;
                    // from_secs_f64 panics on negative, NaN and absurdly huge values,
                    // and I want every possible error (at the language-level) to be a LangError instead of Rust's runtime panics
                    if !(0.0..=Duration::MAX.as_secs_f64()).contains(&secs) {
                        return Err(
                            call_site.error(format!("'sleep' can't sleep for {} seconds.", secs))
                        );
                    }
                    thread::sleep(Duration::from_secs_f64(secs));
                    Ok(Value::Void)
                }
                _ => Err(call_site.error(format!(
                    "'sleep' expects 1 argument, got {}.",
                    arg_values.len()
                ))),
            },
            "type" => match arg_values {
                [v] => Ok(Value::Str(Rc::new(value_type_name(v)))),
                _ => Err(call_site.error(format!(
                    "'type' expects 1 argument, got {}.",
                    arg_values.len()
                ))),
            },
            "elapsed" => match arg_values {
                [] => Ok(Value::Float(self.start.elapsed().as_secs_f64())),
                _ => Err(call_site.error(format!(
                    "'elapsed' expects 0 arguments, got {}.",
                    arg_values.len()
                ))),
            },
            "input" => match arg_values {
                [Value::Str(prompt)] => {
                    // unlike print!, write! gives us the I/O error instead of panicking,
                    // so input() can keep the evaluator's "runtime failures become LangError" rule.
                    write!(io::stdout(), "{}", prompt)
                        .map_err(|e| call_site.error(format!("Failed to write prompt: {}", e)))?;
                    io::stdout()
                        .flush()
                        .map_err(|e| call_site.error(format!("Failed to flush stdout: {}", e)))?;

                    let mut input = String::new();
                    io::stdin()
                        .read_line(&mut input)
                        .map_err(|e| call_site.error(format!("Failed to read input: {}", e)))?;

                    // strips the trailing newline, and '\r' too so CRLF on windows doesn't sneak into the value
                    Ok(Value::Str(Rc::new(
                        input.trim_end_matches(['\n', '\r']).to_string(),
                    )))
                }
                [v] => {
                    Err(call_site.error(format!("'input' expects a 'str', got {}.", type_name(v))))
                }
                _ => Err(call_site.error(format!(
                    "'input' expects 1 argument, got {}.",
                    arg_values.len()
                ))),
            },
            "exit" => {
                let code = match arg_values {
                    [] => 0,
                    [Value::Int(n)] => {
                        if *n < 0 || *n > 255 {
                            return Err(call_site.error(format!(
                                "'exit' expects a code between 0 and 255, got {}.",
                                n
                            )));
                        }
                        *n as i32
                    }
                    [v] => {
                        return Err(call_site
                            .error(format!("'exit' expects an 'int', got {}.", type_name(v))));
                    }
                    _ => {
                        return Err(call_site.error(format!(
                            "'exit' expects 0 or 1 arguments, got {}.",
                            arg_values.len()
                        )));
                    }
                };
                // yeah... this just kills the process. no LangError, no panic, just death.
                // I make an exception from the "every error should be a LangError" rule
                // for this function because it quite literally is a process killer
                io::stdout().flush().ok();
                std::process::exit(code);
            }
            "parse" => match arg_values {
                [Value::Str(s)] => {
                    // never fails by design:
                    // anything it can't parse comes back as the original string.
                    // whether that's forgiving or a footgun, it IS the spec
                    // (and yes, I wrote it)
                    if let Ok(i) = s.parse::<i64>() {
                        Ok(Value::Int(i))
                    } else if s.contains('.') {
                        if let Ok(f) = s.parse::<f64>() {
                            Ok(Value::Float(f))
                        } else {
                            Ok(Value::Str(s.clone()))
                        }
                    } else if s.as_str() == "true" {
                        Ok(Value::Bool(true))
                    } else if s.as_str() == "false" {
                        Ok(Value::Bool(false))
                    } else {
                        Ok(Value::Str(s.clone()))
                    }
                }
                [v] => {
                    Err(call_site.error(format!("'parse' expects a 'str', got {}.", type_name(v))))
                }
                _ => Err(call_site.error(format!(
                    "'parse' expects 1 argument, got {}",
                    arg_values.len()
                ))),
            },
            "fread" => match arg_values {
                // no special error handling beyond what std::fs::read_to_string offers us,
                // it already returns a message containing the reason of why the function failed
                // (file not found, permission denied, not valid UTF-8. etc...)
                [Value::Str(path)] => match std::fs::read_to_string(path.as_str()) {
                    Ok(contents) => Ok(Value::Str(Rc::new(contents))),
                    Err(e) => {
                        Err(call_site.error(format!("'fread' failed to read '{}': {}", path, e)))
                    }
                },
                [v] => {
                    Err(call_site.error(format!("'fread' expects a str, found {}.", type_name(v))))
                }
                _ => Err(call_site.error(format!(
                    "'fread' expects 1 argument, got {}.",
                    arg_values.len()
                ))),
            },
            "fwrite" => match arg_values {
                // "o" overwrites the file (creating it if it's missing), "a" appends to it
                // (also creating it if it's missing). no default mode on purpose, silently
                // overwriting a file is precisely the thing that requires an explicit option
                // same error handling as fread
                [Value::Str(path), Value::Str(contents), Value::Str(mode)] => {
                    let result = match mode.as_str() {
                        "o" => std::fs::write(path.as_str(), contents.as_bytes()),
                        "a" => std::fs::OpenOptions::new()
                            .create(true)
                            .append(true)
                            .open(path.as_str())
                            .and_then(|mut f| f.write_all(contents.as_bytes())),
                        _ => {
                            return Err(call_site.error(format!(
                                "'fwrite' expects 'a' or 'o' as its mode, found '{}'.",
                                mode
                            )));
                        }
                    };
                    match result {
                        Ok(()) => Ok(Value::Void),
                        Err(e) => Err(call_site.error(format!(
                            "'fwrite' failed to write '{}' to '{}': {}",
                            contents, path, e
                        ))),
                    }
                }
                [Value::Str(_), Value::Str(_), v] => Err(call_site.error(format!(
                    "'fwrite' expects (str, str, str), found (str, str, {}).",
                    type_name(v)
                ))),
                [Value::Str(_), v, _] => Err(call_site.error(format!(
                    "'fwrite' expects (str, str, str), found (str, {}, ...).",
                    type_name(v)
                ))),
                [v, _, _] => Err(call_site.error(format!(
                    "'fwrite' expects a str as its first argument, got {}.",
                    type_name(v)
                ))),
                // damn, seems like I forgot to change the error message
                _ => Err(call_site.error(format!(
                    "'fwrite' expects 3 arguments, got {}.",
                    arg_values.len()
                ))),
            },
            _ => Err(call_site.error(format!(
                "Internal evaluator error: '{}' is not a registered builtin.",
                name
            ))),
        }
    }

    /// entry point for user-defined function calls:
    /// enforces the recursion limit, then delegates.
    /// this method acts as a guard for the method `call_function_body`
    /// the reason we don't propagate the error with the `?` operator
    /// when calling `call_function_body()` is to avoid leaking depth.
    /// we must subtract 1 from the `call.depth` field, and error propagation would ruin that.
    fn call_user_function(
        &mut self,
        name: &str,
        arg_values: Vec<Value>,
        call_site: &ExprNode,
    ) -> Result<Value, LangError> {
        if self.call_depth >= MAX_CALL_DEPTH {
            return Err(call_site
                .error(format!(
                    "Stack overflow: recursion exceeded {} nested calls.",
                    MAX_CALL_DEPTH
                ))
                .with_suggestion("Check that your recursive functions have a base case."));
        }
        self.call_depth += 1;
        let result = self.call_function_body(name, arg_values, call_site);
        self.call_depth -= 1;
        result
    }

    /// this is the method that `call_user_function` was wrapping.
    /// we can't leak depth here since the wrapping method already handles that for us
    fn call_function_body(
        &mut self,
        name: &str,
        arg_values: Vec<Value>,
        call_site: &ExprNode,
    ) -> Result<Value, LangError> {
        // no more clone! well... you still see `Rc::clone()`, but that's different from a standard clone.
        // `Rc::clone()` just clones the pointer and bumps the refcounter, but never the data.
        // this is a huge performance gain
        let entry =
            self.functions.get(name).cloned().ok_or_else(|| {
                call_site.error(format!("Call to undeclared function '{}'.", name))
            })?;

        let entry = entry.as_ref();
        if entry.params.len() != arg_values.len() {
            return Err(call_site.error(format!(
                "'{}' expected {} argument(s), got {}.",
                name,
                entry.params.len(),
                arg_values.len()
            )));
        }
        // a brand new environment: functions canNOT see global variables, only
        // their parameters. (recursion still works because the FnTable lives on
        // the Interpreter, not inside any Environment)
        let mut call_env = Environment::new();
        for (param, value) in entry.params.iter().zip(arg_values.into_iter()) {
            // int -> float coercion at the parameter boundary, had to add this atp
            let value = coerce_value_to_type(value, &param.ty);
            if !value_matches_type(&value, &param.ty) {
                return Err(call_site.error(format!(
                    "'{}' expected {} for parameter '{}', got {}.",
                    name,
                    param.ty,
                    param.name,
                    value_type_name(&value)
                )));
            }
            // params are born mutable (no let/var distinction inside a signature)
            call_env.declare(param.name.clone(), value, param.ty.clone(), true);
        }
        match self.exec_block(&entry.body, &mut call_env)? {
            Outcome::Return(Some(v)) => {
                let Some(t) = &entry.ret_type else {
                    return Err(call_site.error(format!(
                        "'{}': returned a value but has no declared return type.",
                        name
                    )));
                };
                // int -> float just like above
                let v = coerce_value_to_type(v, t);
                if !value_matches_type(&v, t) {
                    return Err(call_site.error(format!(
                        "'{}': return type mismatch. expected {}, got {}.",
                        name,
                        t,
                        value_type_name(&v)
                    )));
                }
                Ok(v)
            }
            Outcome::Return(None) => {
                if entry.ret_type.is_some() {
                    return Err(call_site.error(format!(
                        "'{}': declared a return type but returned no value.",
                        name
                    )));
                }
                Ok(Value::Void)
            }
            // falling off the end of a function with a declared return type is a hard error
            // imi has no implicit "return void" on fall-through
            Outcome::Normal => {
                if entry.ret_type.is_some() {
                    return Err(call_site.error(format!(
                        "'{}': missing return. Execution reached the end of the function without returning a value.",
                        name
                    )));
                }
                Ok(Value::Void)
            }
            Outcome::Break | Outcome::Continue => Err(call_site.error(
                "'break'/'continue' escaped a loop without being caught. This means the parser bugged out because it should have rejected it.",
            )),
        }
    }

    /// indexes into an array or string.
    ///
    /// before `Value::Array` used Rc, there was a special fast path for a plain variable
    /// so indexing wouldn't deep-clone the whole array just to read one element. that
    /// optimization is obsolete now: cloning a Value only bumps the Rc, so we can evaluate
    /// the target normally and preserve the natural target-before-index evaluation order
    fn eval_index(
        &mut self,
        target: &ExprNode,
        index: &ExprNode,
        env: &mut Environment,
    ) -> Result<Value, LangError> {
        let value = self.eval(target, env)?;
        let idx = self.eval(index, env)?;
        index_into(&value, &idx, target, index)
    }

    /// half of the lvalue machinery: flattens an index chain like `matrix[i][j]`
    /// into (root variable name, [i, j]), evaluating the index expressions as
    /// they're collected, left to right.
    /// this exists because you can't mutate through an evaluated COPY of an array,
    /// exec_index_assign and eval_method_call need a mutable PATH back to the root
    /// binding, which is what get_indexable_binding_mut reconstructs from this
    fn collect_indices(
        &mut self,
        target: &ExprNode,
        env: &mut Environment,
        indices: &mut Vec<i64>,
    ) -> Result<String, LangError> {
        match &target.node {
            Expr::Variable(name) => Ok(name.clone()),
            Expr::Index(root, idx_expr) => {
                let name = self.collect_indices(root, env, indices)?;
                let idx_val = self.eval(idx_expr, env)?;
                let Value::Int(i) = &idx_val else {
                    return Err(target.error(format!(
                        "Array index must be int, found {}.",
                        type_name(&idx_val)
                    )));
                };
                indices.push(*i);
                Ok(name)
            }
            _ => Err(target.error(
                "Method calls and index assignments are only supported on variables or nested array indices.",
            )),
        }
    }

    /// the other half of the lvalue machinery: walks the type tree and the value
    /// tree in lockstep to reach a `&mut Vec<Value>` of the innermost array, plus
    /// its declared element type (needed for coercion on push/assign).
    ///
    /// two kind of places exist:
    /// - an element of a `var` declared (and maybe nested) array -> MutPlace::ArrayElem,
    /// along with the declared element type (needed for int->float coercion)
    ///
    /// -a character of a `var` declared string -> MutPlace::StrChar,
    /// strings join arrays here so `s[0] = "x"` works exactly array element assignment.
    ///
    /// the indices in `indices` navigate array levels EXCLUSIVELY, yes, EXCLUSIVELY.
    /// which means that the final index (the one landing on a string, when there is one)
    /// is applied by the caller. so reaching Type::Str in the loop means someone is indexing
    /// INTO a character, and a character is a singular value, not a place
    ///
    /// the checks after the loop repeat the ones inside it on purpose: that's the
    /// ZERO-indices case (`a.push(x)` on the root array itself), not copy-paste.
    /// without it, method calls directly on a plain variable wouldn't work at all
    fn get_indexable_binding_mut<'a>(
        &self,
        target: &ExprNode,
        root_name: &str,
        indices: &[i64],
        env: &'a mut Environment,
    ) -> Result<MutPlace<'a>, LangError> {
        let binding = env
            .get_mut(root_name)
            .ok_or_else(|| target.error(format!("Undeclared variable '{}'.", root_name)))?;

        // shape before mutability: if it isn't an array (or now also a string) at all, "use var" would be
        // bad advice (switching to var doesn't make an int indexable), so this has
        // to be reported as a type error rather than a mutability error
        if !matches!(binding.ty, Type::Array(_) | Type::Str) {
            return Err(target.error(format!(
                "'{}' is not an array (declared as {}).",
                root_name, binding.ty
            )));
        }

        if !binding.mutable {
            return Err(target.error(format!(
                "Cannot mutate '{}'. It was declared with 'let'. Use var to make it mutable.",
                root_name
            )));
        }

        let mut current_val = &mut binding.value;
        let mut current_ty = binding.ty.clone();

        for &i in indices {
            let inner_ty = match current_ty.clone() {
                Type::Array(inner) => *inner,
                Type::Str => {
                    return Err(target.error(
                        "Cannot index into a character. Only the last index may target a str",
                    ));
                }
                _ => return Err(target.error("Cannot index into a non-array.")),
            };

            let Value::Array(items) = current_val else {
                return Err(target.error("Target is not an array."));
            };
            // clone-on-write: only actually clones this container if it's
            // currently shared (reference count > 1).
            // a sibling branch nobody is touching never gets cloned,
            // only the spine down to the mutated branch
            let items = Rc::make_mut(items);

            if i < 0 || i as usize >= items.len() {
                return Err(target.error(format!(
                    "Index {} out of bounds (array has {} elements).",
                    i,
                    items.len()
                )));
            }

            current_val = &mut items[i as usize];
            current_ty = inner_ty;
        }

        match current_ty {
            Type::Array(inner) => {
                let Value::Array(items) = current_val else {
                    return Err(target.error("Target is not an array."));
                };
                Ok(MutPlace::ArrayElem(Rc::make_mut(items), *inner))
            }
            Type::Str => {
                let Value::Str(s) = current_val else {
                    return Err(target.error("Target is not a string."));
                };
                Ok(MutPlace::StrChar(Rc::make_mut(s)))
            }
            // landing on a primitive here means the source tried to keep indexing or
            // call an array method after the path had already reached a scalar value
            //
            // basically, indexing a primitive is bad bad and it triggers a runtime error
            _ => Err(target.error("Cannot mutate or call an array method on a non-array value.")),
        }
    }

    /// array methods (push/pop/remove). note the order of operations:
    /// ALL arguments are evaluated BEFORE the mutable borrow of the target binding is taken.
    /// that's load-bearing, it's why `a.push(a.pop())` works, the borrows never overlap
    fn eval_method_call(
        &mut self,
        target: &ExprNode,
        method: &str,
        args: &[ExprNode],
        env: &mut Environment,
    ) -> Result<Value, LangError> {
        let mut target_indices = Vec::new();
        let root_name = self.collect_indices(target, env, &mut target_indices)?;

        let arg_values = self.eval_arguments(args, env)?;

        let (items, inner_ty) =
            match self.get_indexable_binding_mut(target, &root_name, &target_indices, env)? {
                MutPlace::ArrayElem(items, inner_ty) => (items, inner_ty),
                MutPlace::StrChar(_) => {
                    return Err(target.error(
                        "Strings have no methods. Method calls are only supported on arrays.",
                    ));
                }
            };

        match method {
            "push" => {
                let [v] = arg_values.as_slice() else {
                    return Err(target.error(format!(
                        "'push' expects exactly 1 argument, got {}.",
                        arg_values.len()
                    )));
                };
                let mut v = v.clone();

                // int -> float coercion applies here too, same as everywhere else
                v = coerce_value_to_type(v, &inner_ty);

                if !value_matches_type(&v, &inner_ty) {
                    return Err(target.error(format!(
                        "Cannot push {} to an array of {}.",
                        value_type_name(&v),
                        inner_ty
                    )));
                }

                items.push(v);
                Ok(Value::Void)
            }
            "pop" => {
                if !arg_values.is_empty() {
                    return Err(target.error(format!(
                        "'pop' expects 0 arguments, got {}.",
                        arg_values.len()
                    )));
                }
                items
                    .pop()
                    .ok_or_else(|| target.error("Cannot pop from an empty array."))
            }
            "remove" => {
                if arg_values.len() != 1 {
                    return Err(target.error(format!(
                        "'remove' expects exactly 1 argument, got {}.",
                        arg_values.len()
                    )));
                }
                let idx = match arg_values.get(0) {
                    Some(Value::Int(i)) => *i,
                    _ => return Err(target.error("'remove' expects 1 int argument.")),
                };
                if idx < 0 || idx as usize >= items.len() {
                    return Err(target.error(format!(
                        "Index {} out of bounds (array has {} elements).",
                        idx,
                        items.len()
                    )));
                }
                Ok(items.remove(idx as usize))
            }
            other => Err(target.error(format!("Array has no method '{}'.", other))),
        }
    }

    fn exec_index_assign(
        &mut self,
        target: &ExprNode,
        index: &ExprNode,
        op: &AssignOp,
        expr: &ExprNode,
        env: &mut Environment,
    ) -> Result<(), LangError> {
        let mut target_indices = Vec::new();
        let root_name = self.collect_indices(target, env, &mut target_indices)?;

        let final_idx = self.eval(index, env)?;
        let new_val = self.eval(expr, env)?;

        let Value::Int(final_i) = &final_idx else {
            return Err(target.error(format!(
                "Array index must be int, found {}.",
                type_name(&final_idx)
            )));
        };

        let final_i = *final_i;

        match self.get_indexable_binding_mut(target, &root_name, &target_indices, env)? {
            MutPlace::ArrayElem(items, inner_ty) => {
                if final_i < 0 || final_i as usize >= items.len() {
                    return Err(index.error(format!(
                        "Index {} out of bounds (array has {} elements).",
                        final_i,
                        items.len()
                    )));
                }

                let idx_usize = final_i as usize;

                let final_val = match op.to_binary_op() {
                    None => new_val,
                    Some(bin_op) => {
                        let current = items[idx_usize].clone();
                        self.eval_arithmetic(&bin_op, current, new_val)
                            .map_err(|e| expr.error(e))?
                    }
                };

                let final_val = coerce_value_to_type(final_val, &inner_ty);

                if !value_matches_type(&final_val, &inner_ty) {
                    return Err(expr.error(format!(
                        "Cannot assign {} to an array of {}.",
                        value_type_name(&final_val),
                        inner_ty
                    )));
                }
                items[idx_usize] = final_val;
                Ok(())
            }

            MutPlace::StrChar(s) => {
                // I genuinely don't know what to do with compound assignment for single characters in the spec,
                // so I just denied them outright
                if !matches!(op, AssignOp::Assign) {
                    return Err(
                        index.error("Compound assignment is not supported on a single character.")
                    );
                }

                let Value::Str(ch) = &new_val else {
                    return Err(expr.error(format!(
                        "Cannot assign {} to a str index. Expected a str.",
                        type_name(&new_val)
                    )));
                };

                // a str index holds exactly ONE character,
                // so the assigned stings must be exactly one character long.
                // this is why we perform an explicit length check
                let mut replacement = ch.chars();

                match (replacement.next(), replacement.next()) {
                    (Some(c), None) => {
                        // indices count unicode scalar values, not bytes (identical to imi's in-built `len()`),
                        // and this is why I'm using a Vec<char> instead of byte-slicing
                        let mut chars: Vec<char> = s.chars().collect();
                        if final_i < 0 || final_i as usize >= chars.len() {
                            return Err(index.error(format!(
                                "Index {} out of bounds (str has {} characters).",
                                final_i,
                                chars.len()
                            )));
                        }
                        chars[final_i as usize] = c;
                        *s = chars.into_iter().collect();
                        Ok(())
                    }
                    _ => Err(expr.error(format!(
                        "Cannot assign '{}' to a str index: expected exactly one character.",
                        ch
                    ))),
                }
            }
        }
    }

    /// compound assignment for plain variables (`x += 2`), same reuse of
    /// eval_arithmetic as the index version. plain `=` skips all of this
    fn apply_assign_op(
        &self,
        env: &Environment,
        name: &str,
        op: &AssignOp,
        rhs: Value,
        stmt: &StmtNode,
    ) -> Result<Value, LangError> {
        // plain `=` has no binary operation to apply
        let Some(bin_op) = op.to_binary_op() else {
            return Ok(rhs);
        };

        // we fetch the current value
        let current = env
            .get(name)
            .cloned()
            .ok_or_else(|| stmt.error(format!("Undeclared variable '{}'", name)))?;

        // and finally evaluate the expression
        self.eval_arithmetic(&bin_op, current, rhs)
            .map_err(|e| stmt.error(e))
    }

    /// positional formatting: every `{}` in the template consumes the next
    /// argument in order. `{{` and `}}` are escapes for literal braces.
    /// note that placeholder COUNT is only checked here, at runtime
    /// the parser's "first argument must be a string literal" rule guarantees we
    /// have a template, but it can't count the {}s against the arguments
    fn do_format(&self, args: &[Value], call_site: &ExprNode) -> Result<String, LangError> {
        let Some(Value::Str(template)) = args.first() else {
            // safe (but fragile) since the parser ensures the first argument is a string
            unreachable!("bro, I swear you told me this was safe. damned be the parser");
        };

        let mut result = String::new();
        let mut arg_iter = args[1..].iter();
        let mut chars = template.chars().peekable();

        while let Some(c) = chars.next() {
            if c == '{' && chars.peek() == Some(&'{') {
                chars.next();
                result.push('{'); // '{{' is an escaped '{'
            } else if c == '{' && chars.peek() == Some(&'}') {
                chars.next();
                match arg_iter.next() {
                    Some(v) => result.push_str(&display_value(v).map_err(|e| call_site.error(e))?),
                    None => {
                        return Err(call_site.error(format!(
                            "Not enough arguments for format string '{}'.",
                            template
                        )));
                    }
                }
            } else if c == '}' && chars.peek() == Some(&'}') {
                chars.next();
                result.push('}'); // '}}' for symmetry
            } else {
                result.push(c);
            }
        }
        if arg_iter.next().is_some() {
            return Err(call_site.error(format!(
                "Too many arguments for format string '{}'.",
                template
            )));
        }
        Ok(result)
    }
}

/// shared by both branches of eval_index (the plain-variable fast path and the general fallback): 
/// arrays return a clone of the element.
/// strings return a single character `str`, since imi has no separate char type. (and I'm not planning to introduce one either)
/// this is the same representation MutPlace::StrChar writes into on the assignment side,
/// so `s[0]` and `s[0] = "x"` agree on what a "character" is
fn index_into(
    value: &Value,
    idx: &Value,
    target: &ExprNode,
    index: &ExprNode,
) -> Result<Value, LangError> {
    let Value::Int(i) = idx else {
        return Err(target.error(format!(
            "Cannot index {} with {}.",
            type_name(value),
            type_name(idx)
        )));
    };
    let i = *i;

    match value {
        Value::Array(items) => {
            if i < 0 || i as usize >= items.len() {
                return Err(index.error(format!(
                    "Index {} out of bounds (array has {} elements).",
                    i,
                    items.len()
                )));
            }
            Ok(items[i as usize].clone())
        }
        Value::Str(s) => {
            let chars: Vec<char> = s.chars().collect();
            if i < 0 || i as usize >= chars.len() {
                return Err(index.error(format!(
                    "Index {} out of bounds (str has {} characters).",
                    i,
                    chars.len()
                )));
            }
            Ok(Value::Str(Rc::new(chars[i as usize].to_string())))
        }
        other => Err(target.error(format!(
            "Cannot index {} with {}.",
            type_name(other),
            type_name(idx)
        ))),
    }
}

fn as_f64(v: &Value) -> Result<f64, String> {
    match v {
        Value::Int(n) => Ok(*n as f64),
        Value::Float(f) => Ok(*f),
        other => Err(format!("Expected a number, found {}.", type_name(other))),
    }
}

/// the flat name of a value's type, any array just says "array".
/// keep in sync with value_type_name below, which reports the element type too
pub fn type_name(v: &Value) -> &'static str {
    match v {
        Value::Int(_) => "int",
        Value::Float(_) => "float",
        Value::Str(_) => "str",
        Value::Bool(_) => "bool",
        Value::Array(_) => "array",
        Value::Void => "void",
        Value::Uninitialized => "uninitialized",
    }
}

/// like type_name, but arrays report their element type, for example: "array[int]".
/// two functions instead of one because not every caller has an infallible
/// array type to report (an empty array can't infer its element type)
fn value_type_name(v: &Value) -> String {
    match v {
        Value::Int(_) => "int".to_string(),
        Value::Float(_) => "float".to_string(),
        Value::Str(_) => "str".to_string(),
        Value::Bool(_) => "bool".to_string(),
        Value::Void => "void".to_string(),
        Value::Uninitialized => "uninitialized".to_string(),
        Value::Array(_) => match infer_type(v) {
            Ok(t) => t.to_string(),
            Err(_) => "array".to_string(),
        },
    }
}

/// how values render inside format strings / println.
/// floats go through rust's Display, which drops the trailing ".0"
/// println("{}", 2.0) prints `2`, not `2.0`. python-brained users will notice
fn display_value(v: &Value) -> Result<String, String> {
    match v {
        Value::Int(n) => Ok(n.to_string()),
        Value::Float(f) => Ok(format!("{}", f)),
        Value::Str(s) => Ok(s.to_string()),
        Value::Bool(b) => Ok(b.to_string()),
        Value::Array(items) => {
            let mut inner: Vec<String> = Vec::with_capacity(items.len());
            for it in items.iter() {
                inner.push(display_value(it)?);
            }
            Ok(format!("[{}]", inner.join(", ")))
        }
        Value::Void => Err(
            "Attempted to display a void value. this call doesn't produce a usable value."
                .to_string(),
        ),
        Value::Uninitialized => Err("Attempted to display an uninitialized variable.".to_string()),
    }
}

/// exact type matching, never coerces any type.
/// int -> float happens through coerce_value_to_type, never in here
pub fn value_matches_type(v: &Value, t: &Type) -> bool {
    match (v, t) {
        (Value::Int(_), Type::Int) => true,
        (Value::Float(_), Type::Float) => true,
        (Value::Str(_), Type::Str) => true,
        (Value::Bool(_), Type::Bool) => true,
        (Value::Array(items), Type::Array(inner)) => {
            // an empty array matches ANY array type (all() on empty is true).
            // deliberate: `var a: array[int] = [];` must be legal (see infer_type)
            items.iter().all(|item| value_matches_type(item, inner))
        }
        _ => false,
    }
}

/// takes a value as input and returns its matching type.
/// this function is helpful for declarations like `let x = 5;`
/// where `x` was declared without a type.
/// imi infers the type for x from the value it was initialized with
///
/// it can fail tho: an empty array has no element type to infer (that's what the
/// error up there is for), and neither void nor an uninitialized value has a type
fn infer_type(v: &Value) -> Result<Type, String> {
    match v {
        Value::Int(_) => Ok(Type::Int),
        Value::Float(_) => Ok(Type::Float),
        Value::Str(_) => Ok(Type::Str),
        Value::Bool(_) => Ok(Type::Bool),
        Value::Array(items) => {
            let Some(first) = items.first() else {
                return Err("Cannot infer the element type of an empty array literal. Give it an explicit type, e.g. 'var x: array[int] = [];'"
                    .to_string());
            };

            let element_type = infer_type(first)?;

            for item in &items[1..] {
                let item_type = infer_type(item)?;

                if item_type != element_type {
                    return Err(format!(
                        "Array elements must have the same type, found {} and {}.",
                        element_type, item_type
                    ));
                }
            }

            Ok(Type::Array(Box::new(element_type)))
        }
        Value::Void => Err(
            "Cannot assign a void value to a variable. This call doesn't produce a usable value."
                .to_string(),
        ),
        Value::Uninitialized => Err("Cannot infer the type of an uninitialized value.".to_string()),
    }
}

/// recursive because arrays can nest, a float buried in [[1, 2.0]] still
/// promotes the whole tree
fn contains_float(values: &[Value]) -> bool {
    values.iter().any(|v| match v {
        Value::Float(_) => true,
        Value::Array(inner) => contains_float(inner),
        _ => false,
    })
}

/// I believe I explained this above in some match arm somewhere,
/// but I will do it again in other words to be precise
///
/// when promoting ints to floats, we need exclusive access to the array
/// if no one else is owning the data, we simply modify it, fine.
/// if there are more owners, we clone the data so we can have exclusive access to it,
/// leaving the original value unchanged. this is essentially what `Rc::make_mut(this)` does
///
/// this is relevant because coercing (which mutates) the items in an array owned by anyone other
/// than the expression breaks the semantics of the language. therefore, it is important that if we're going
/// to modify the array in-place, it can't have more owners than the expression itself
fn promote_ints_to_floats(values: &mut [Value]) {
    for v in values.iter_mut() {
        match v {
            Value::Int(n) => *v = Value::Float(*n as f64),
            Value::Array(inner) => {
                let inner: &mut Vec<Value> = Rc::make_mut(inner);
                promote_ints_to_floats(inner);
            }
            _ => {}
        }
    }
}

/// imi's ONLY implicit conversion: int -> float, applied recursively through
/// arrays. everything else is exact. nothing ever converts float -> int.
///
/// importantly, this function does NOT clone an Rc-backed array just because it was
/// asked to "coerce" it. if the value already matches the target type, the Rc is kept
/// as-is. copy-on-write only kicks in along branches where an int actually has to become
/// a float, which is the whole point of using Rc in the first place.
pub fn coerce_value_to_type(mut v: Value, t: &Type) -> Value {
    coerce_value_to_type_in_place(&mut v, t);
    v
}

/// returns true only when applying the target type would actually change a value.
/// this lets the in-place coercion below avoid Rc::make_mut on already-correct arrays.
fn value_needs_coercion(v: &Value, t: &Type) -> bool {
    match (v, t) {
        (Value::Int(_), Type::Float) => true,
        (Value::Array(items), Type::Array(inner_ty)) => items
            .iter()
            .any(|item| value_needs_coercion(item, inner_ty)),
        _ => false,
    }
}

/// performs the actual conversion while respecting copy-on-write.
/// if an array is shared, Rc::make_mut clones only when a descendant really needs
/// coercion. nested arrays repeat the same rule so only the changed spine is copied.
fn coerce_value_to_type_in_place(v: &mut Value, t: &Type) {
    match t {
        Type::Float => {
            if let Value::Int(n) = v {
                let n = *n;
                *v = Value::Float(n as f64);
            }
        }
        Type::Array(inner_ty) => {
            let Value::Array(items) = v else {
                return;
            };

            if !items
                .iter()
                .any(|item| value_needs_coercion(item, inner_ty))
            {
                return;
            }

            for item in Rc::make_mut(items).iter_mut() {
                coerce_value_to_type_in_place(item, inner_ty);
            }
        }
        Type::Int | Type::Str | Type::Bool => {}
    }
}
