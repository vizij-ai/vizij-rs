//! Numeric helper utilities shared across node evaluators.

use crate::graph_value::{GraphValue, VizijKind};

use super::value_layout::{align_flattened, flatten_numeric};

/// Apply `op` pairwise to two numeric values, broadcasting scalars when possible.
pub fn binary_numeric<V, F>(lhs: &V, rhs: &V, op: F) -> V
where
    V: GraphValue,
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
        (None, None) => V::float(f32::NAN),
    }
}

/// Apply `op` to every component of `input`.
pub fn unary_numeric<V, F>(input: &V, op: F) -> V
where
    V: GraphValue,
    F: Fn(f32) -> f32 + Copy,
{
    match flatten_numeric(input) {
        Some(flat) => {
            let data: Vec<f32> = flat.data.iter().map(|x| op(*x)).collect();
            flat.layout.reconstruct(&data)
        }
        None => V::float(f32::NAN),
    }
}

/// Coerce a value to a single `f32`.
pub fn as_float<V: GraphValue>(v: &V) -> f32 {
    v.to_float()
}

/// Coerce a value to a boolean, treating non-zero numeric entries as `true`.
pub fn as_bool<V: GraphValue>(v: &V) -> bool {
    match v.kind() {
        VizijKind::Bool => v.as_bool().unwrap_or(false),
        VizijKind::Text => v.as_text().is_some_and(|s| !s.is_empty()),
        _ => v.to_vector().iter().any(|x| *x != 0.0),
    }
}
