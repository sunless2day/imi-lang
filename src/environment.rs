//! this module is the runtime's storage, I suppose (beatrice reference)
//! the evaluator will need to store things like variable states and function declarations somewhere...
//! this is the place!

use crate::error::LangError;
use crate::{
    ast::Type,
    evaluator::{Value, coerce_value_to_type, type_name, value_matches_type},
};

// yes, we will be using hashmaps for this
// (O(1) lookups and we never need the bindings in order, so they're the right tool)
use std::collections::HashMap;

pub struct Binding {
    pub value: Value,
    pub ty: Type,
    /// true if declared with `var`, false if declared with `let`
    /// (this is how my interpreter distincts a let/var declaration)
    pub mutable: bool,
    /// sometimes an immutable variable can be declared and not initialized right away,
    /// the interpreter must know if it was initialized or not so something like:
    /// ```
    ///  let x: int;
    ///
    ///  x = 15;
    /// ```
    /// won't raise a mutability error.
    /// the interpreter must know if a variable was initialized or not
    /// in order to safely "mutate" the state of an uninitialized variable
    pub is_initialized: bool,
}

/// each scope is a hashmap, and multiple scopes is simply just a vector of them
///
/// storing scopes in this particular fashion allows inner scopes to access variables of outer scopes
/// each new scope gets pushed to the array and lies on top of the former scope
pub struct Environment {
    scopes: Vec<HashMap<String, Binding>>,
}

impl Environment {
    pub fn new() -> Self {
        Environment {
            scopes: vec![HashMap::new()],
        }
    }

    /// whenever a new scope is entered, it is pushed to the array
    pub fn push_scope(&mut self) {
        self.scopes.push(HashMap::new());
    }

    /// likewise, a scope is popped when you exit one
    /// (the global scope is never popped, which is what keeps `scopes` from ever being empty,
    /// and what makes the unwrap in declare() safe)
    pub fn pop_scope(&mut self) {
        if self.scopes.len() > 1 {
            self.scopes.pop();
        }
    }

    pub fn declare(&mut self, name: String, value: Value, ty: Type, mutable: bool) {
        let is_initialized = !matches!(value, Value::Uninitialized);

        // it is "safe" to use the method unwrap here because the `.pop_scope()` method never pops the first (aka global) scope even if called.
        // it ensures we never end up with an empty vector
        // which means that `last_mut()` will never yield a `None` variant
        self.scopes.last_mut().unwrap().insert(
            name,
            Binding {
                value,
                ty,
                mutable,
                is_initialized,
            },
        );
    }

    /// simply searches through the vector of hashmaps
    /// and fetches the requested identifier if found
    pub fn get(&self, name: &str) -> Option<&Value> {
        self.scopes
            .iter()
            .rev()
            .find_map(|s| s.get(name).map(|b| &b.value))
    }

    /// same thing as the one above but returns a mutable reference instead
    pub fn get_mut(&mut self, name: &str) -> Option<&mut Binding> {
        self.scopes.iter_mut().rev().find_map(|s| s.get_mut(name))
    }

    /// this function takes a name and a value (and also a line and column in case it needs to report
    /// an error) to assign the value to the variable with that name. if the variable exists in any
    /// reachable scope, is mutable and the value to-be-assigned is of the same type, then the
    /// assignment is performed and its state is set to `initialized`. if the variable is immutable
    /// then it checks if it was already initialized and if not it assigns the value to the variable
    /// and sets its initialization state to true. if the variable wasn't found or it was
    /// an initialized immutable then it returns an error
    pub fn assign(
        &mut self,
        name: &str,
        value: Value,
        line: usize,
        col: usize,
    ) -> Result<(), LangError> {
        match self.get_mut(name) {
            Some(binding) if binding.mutable => {
                // int->float is imi's only implicit conversion, and assignment gets the
                // same treatment as declaration, push and index assignment
                // (without this, `var x: float = 1;` works but a later `x = 2;` errors)
                let value = coerce_value_to_type(value, &binding.ty);
                if !value_matches_type(&value, &binding.ty) {
                    return Err(LangError::new(
                        line,
                        col,
                        format!(
                            "cannot assign {} to '{}'. It was declared as {}.",
                            type_name(&value),
                            name,
                            binding.ty
                        ),
                    ));
                }
                binding.value = value;
                binding.is_initialized = true;
                Ok(())
            }

            Some(binding) if !binding.is_initialized => {
                // same coercion rule as the arm above
                let value = coerce_value_to_type(value, &binding.ty);
                if !value_matches_type(&value, &binding.ty) {
                    return Err(LangError::new(
                        line,
                        col,
                        format!(
                            "cannot assign {} to '{}'. It was declared as {}.",
                            type_name(&value),
                            name,
                            binding.ty
                        ),
                    ));
                }
                binding.value = value;
                binding.is_initialized = true;
                Ok(())
            }
            Some(_) => Err(LangError::new(
                line,
                col,
                format!(
                    "Cannot assign to '{}'. It was declared with 'let' and is already initialized.",
                    name
                ),
            )),
            None => Err(LangError::new(
                line,
                col,
                format!("Cannot assign to undeclared variable '{}'.", name),
            )),
        }
    }
}
