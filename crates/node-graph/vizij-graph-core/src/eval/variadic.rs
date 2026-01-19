//! Helpers for dealing with variadic node inputs.

use std::cmp::Ordering;

use vizij_api_core::Value;

use super::eval_node::InputSlots;
use super::numeric::binary_numeric;
use super::PortValue;

/// Split a variadic input key into its prefix and optional positional suffix.
///
/// Keys like `operand_3` return `("operand", Some(3))`.
pub fn parse_variadic_key(key: &str) -> (&str, Option<usize>) {
    if let Some((prefix, tail)) = key.rsplit_once('_') {
        if let Ok(idx) = tail.parse::<usize>() {
            return (prefix, Some(idx));
        }
    }
    (key, None)
}

/// Sort variadic keys lexicographically by prefix then index.
///
/// This keeps `operand_0`, `operand_1`, ... stable for evaluation.
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

/// Collect operand_* ports in stable slot order.
pub fn collect_operand_ports<'a>(inputs: &'a InputSlots<'a>) -> Vec<&'a PortValue> {
    inputs.variadic("operand").iter().collect()
}

/// Fold a variadic collection of values with the provided numeric operator.
///
/// Returns `empty_fallback` when the slice is empty.
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
