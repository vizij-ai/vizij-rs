//! Numeric helper utilities shared across node evaluators.

use vizij_api_core::{coercion, Value};

use super::value_layout::{align_flattened, flatten_numeric};

/// Apply `op` pairwise to two numeric values, broadcasting scalars when possible.
///
/// Non-numeric inputs or incompatible layouts yield a NaN-filled result using the closest
/// compatible layout.
///
/// # Examples
///
/// ```
/// use vizij_api_core::Value;
/// use vizij_graph_core::eval::binary_numeric;
///
/// let lhs = Value::Vec3([1.0, 2.0, 3.0]);
/// let rhs = Value::Float(1.0);
/// let out = binary_numeric(&lhs, &rhs, |a, b| a + b);
/// assert_eq!(out, Value::Vec3([2.0, 3.0, 4.0]));
/// ```
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

/// Apply `op` to every component of `input`, preserving the input layout.
///
/// Non-numeric inputs yield a scalar NaN.
///
/// # Examples
///
/// ```
/// use vizij_api_core::Value;
/// use vizij_graph_core::eval::unary_numeric;
///
/// let input = Value::Vec2([3.0, 4.0]);
/// let out = unary_numeric(&input, |x| x * 2.0);
/// assert_eq!(out, Value::Vec2([6.0, 8.0]));
/// ```
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
///
/// This uses the same coercion logic as [`vizij_api_core::coercion::to_float`].
///
/// # Examples
///
/// ```
/// use vizij_api_core::Value;
/// use vizij_graph_core::eval::as_float;
///
/// let value = Value::Vec3([2.0, 0.0, 0.0]);
/// assert_eq!(as_float(&value), 2.0);
/// ```
pub fn as_float(v: &Value) -> f32 {
    coercion::to_float(v)
}

/// Coerce a [`Value`] to a boolean.
///
/// Text values are `true` when non-empty and numeric values are `true` when any component is
/// non-zero.
///
/// # Examples
///
/// ```
/// use vizij_api_core::Value;
/// use vizij_graph_core::eval::as_bool;
///
/// assert!(as_bool(&Value::Text("ok".into())));
/// assert!(!as_bool(&Value::Float(0.0)));
/// ```
pub fn as_bool(v: &Value) -> bool {
    match v {
        Value::Bool(b) => *b,
        Value::Text(s) => !s.is_empty(),
        _ => coercion::to_vector(v).iter().any(|x| *x != 0.0),
    }
}
