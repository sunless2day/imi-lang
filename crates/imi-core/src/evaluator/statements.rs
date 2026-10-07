use super::{
    Interpreter, Outcome,
    types::{coerce_value_to_type, infer_type, type_name, value_matches_type},
    value::Value,
};
use crate::{
    ast::{ExprNode, Stmt, StmtNode, Type},
    environment::Environment,
    error::LangError,
};

impl Interpreter {
    /// executes statements in a fresh scope, stopping at (and propagating) the first
    /// non-Normal Outcome. the scope is popped whether the block finished or broke out
    pub(super) fn exec_block(
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
    pub(super) fn exec_stmt(
        &mut self,
        stmt: &StmtNode,
        env: &mut Environment,
    ) -> Result<Outcome, LangError> {
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
}
