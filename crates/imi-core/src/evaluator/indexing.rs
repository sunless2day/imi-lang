use super::{
    Interpreter,
    types::{coerce_value_to_type, type_name, value_matches_type, value_type_name},
    value::Value,
};
use crate::{
    ast::{AssignOp, Expr, ExprNode, StmtNode, Type},
    environment::Environment,
    error::LangError,
};
use std::rc::Rc;

/// this is the destination of an index.
/// this can either produce an array element or a sub-string from a string (a single character)
enum MutPlace<'a> {
    ArrayElem(&'a mut Vec<Value>, Type),
    StrChar(&'a mut String),
}

impl Interpreter {
    /// indexes into an array or string.
    ///
    /// before `Value::Array` used Rc, there was a special fast path for a plain variable
    /// so indexing wouldn't deep-clone the whole array just to read one element. that
    /// optimization is obsolete now: cloning a Value only bumps the Rc, so we can evaluate
    /// the target normally and preserve the natural target-before-index evaluation order
    pub(super) fn eval_index(
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
    pub(super) fn eval_method_call(
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

    pub(super) fn exec_index_assign(
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
    pub(super) fn apply_assign_op(
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
}

/// shared by both branches of eval_index (the plain-variable fast path and the general fallback):
/// arrays return a clone of the element.
/// strings return a single character `str`, since imi has no separate char type. (and I'm not planning to introduce one either)
/// this is the same representation MutPlace::StrChar writes into on the assignment side,
/// so `s[0]` and `s[0] = "x"` agree on what a "character" is
pub(super) fn index_into(
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
