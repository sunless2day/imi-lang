use super::{
    BUILTINS, Interpreter, MAX_CALL_DEPTH, Outcome,
    types::{coerce_value_to_type, value_matches_type, value_type_name},
    value::Value,
};
use crate::{ast::ExprNode, environment::Environment, error::LangError};

impl Interpreter {
    /// cool new helper below:
    /// evaluates call arguments strictly left to right.
    /// both ordinary function calls and array methods use this helper so they agree
    /// on evaluation order without copy-pasting the same loop.
    pub(super) fn eval_arguments(
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

    pub(super) fn eval_call(
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
}
