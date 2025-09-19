//! Numeric helper utilities shared across node evaluators.

use vizij_api_core::{coercion, Value};

use super::value_layout::{align_flattened, flatten_numeric};

/// Apply `op` pairwise to two numeric values, broadcasting scalars when possible.
pub fn binary_numeric<F>(lhs: &Value, rhs: &Value, op: F) -> Value
where
    F: Fn(f32, f32) -> f32 + Copy,
{
    match (flatten_numeric(lhs), flatten_numeric(rhs)) {
        (Some(a), Some(b)) => match align_flattened(&a, &b) {
            Ok((layout, da, db)) => {
                let data: Vec<f32> = da.iter().zip(db.iter()).map(|(x, y)| op(*x, *y)).collect();
                layout.reconstruct(&data)
            }
            Err(layout) => layout.fill_with(f32::NAN),
        },
        (Some(a), None) => a.layout.fill_with(f32::NAN),
        (None, Some(b)) => b.layout.fill_with(f32::NAN),
        (None, None) => Value::Float(f32::NAN),
    }
}

/// Apply `op` to every component of `input`.
pub fn unary_numeric<F>(input: &Value, op: F) -> Value
where
    F: Fn(f32) -> f32 + Copy,
{
    match flatten_numeric(input) {
        Some(flat) => {
            let data: Vec<f32> = flat.data.iter().map(|x| op(*x)).collect();
            flat.layout.reconstruct(&data)
        }
        None => Value::Float(f32::NAN),
    }
}

/// Coerce a [`Value`] to a single `f32`.
pub fn as_float(v: &Value) -> f32 {
    coercion::to_float(v)
}

/// Coerce a [`Value`] to a boolean, treating non-zero numeric entries as `true`.
pub fn as_bool(v: &Value) -> bool {
    match v {
        Value::Bool(b) => *b,
        Value::Text(s) => !s.is_empty(),
        _ => coercion::to_vector(v).iter().any(|x| *x != 0.0),
    }
}
