use super::value::Value;
use crate::ast::Type;
use std::rc::Rc;

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
pub(super) fn value_type_name(v: &Value) -> String {
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
pub(super) fn infer_type(v: &Value) -> Result<Type, String> {
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
pub(super) fn contains_float(values: &[Value]) -> bool {
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
pub(super) fn promote_ints_to_floats(values: &mut [Value]) {
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
