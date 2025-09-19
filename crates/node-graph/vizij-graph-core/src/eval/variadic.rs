//! Helpers for dealing with variadic node inputs.

use std::cmp::Ordering;

use vizij_api_core::Value;

use super::numeric::binary_numeric;

/// Split a variadic input key into its prefix and optional positional suffix.
pub fn parse_variadic_key(key: &str) -> (&str, Option<usize>) {
    if let Some((prefix, tail)) = key.rsplit_once('_') {
        if let Ok(idx) = tail.parse::<usize>() {
            return (prefix, Some(idx));
        }
    }
    (key, None)
}

/// Sort variadic keys lexicographically by prefix then index.
pub fn compare_variadic_keys(a: &str, b: &str) -> Ordering {
    let (prefix_a, idx_a) = parse_variadic_key(a);
    let (prefix_b, idx_b) = parse_variadic_key(b);

    match prefix_a.cmp(prefix_b) {
        Ordering::Equal => match (idx_a, idx_b) {
            (Some(ia), Some(ib)) => ia.cmp(&ib),
            (Some(_), None) => Ordering::Less,
            (None, Some(_)) => Ordering::Greater,
            (None, None) => a.cmp(b),
        },
        other => other,
    }
}

/// Fold a variadic collection of values with the provided numeric operator.
pub fn fold_numeric_variadic<F>(values: &[Value], op: F, empty_fallback: Value) -> Value
where
    F: Fn(f32, f32) -> f32 + Copy,
{
    if values.is_empty() {
        return empty_fallback;
    }
    let mut iter = values.iter();
    let mut acc = iter
        .next()
        .cloned()
        .unwrap_or_else(|| Value::Float(f32::NAN));
    for v in iter {
        acc = binary_numeric(&acc, v, op);
    }
    acc
}
