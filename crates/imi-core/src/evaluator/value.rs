use super::types::type_name;
use std::rc::Rc;

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

pub(super) fn as_f64(v: &Value) -> Result<f64, String> {
    match v {
        Value::Int(n) => Ok(*n as f64),
        Value::Float(f) => Ok(*f),
        other => Err(format!("Expected a number, found {}.", type_name(other))),
    }
}

/// how values render inside format strings / println.
/// floats go through rust's Display, which drops the trailing ".0"
/// println("{}", 2.0) prints `2`, not `2.0`. python-brained users will notice
pub(super) fn display_value(v: &Value) -> Result<String, String> {
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
