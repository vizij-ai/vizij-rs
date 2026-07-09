#![allow(dead_code)]
//! Accumulation of per-target POD contributions and blending into final
//! wire-form [`Value`]s.
//!
//! Contributions arrive as sampled [`TrackValue`]s; the accumulator keeps
//! weighted component sums in plain arrays and encodes each blended result
//! through the vocabulary constructors once, in [`AccumulatorWithDerivatives::finalize`].

use std::collections::HashMap;

use crate::interp::functions::nlerp_quat;
use crate::value::{TrackValue, Transform, Value};
use vizij_api_core::value as vocab;

/// Numeric collection flavor: a `Vector` re-encodes as `ArrayF32`, an
/// `Array` as an all-scalar `ArrayValue`.
#[derive(Clone, Debug)]
enum CollectionKind {
    Vector(usize),
    Array(usize),
}

impl CollectionKind {
    fn len(&self) -> usize {
        match *self {
            CollectionKind::Vector(len) | CollectionKind::Array(len) => len,
        }
    }

    fn matches(&self, other: &CollectionKind) -> bool {
        std::mem::discriminant(self) == std::mem::discriminant(other) && self.len() == other.len()
    }

    fn build_value(&self, data: Vec<f32>) -> Value {
        match *self {
            CollectionKind::Vector(_) => vocab::vector(data),
            CollectionKind::Array(len) => {
                debug_assert_eq!(data.len(), len);
                vocab::array(data.into_iter().map(vocab::float).collect())
            }
        }
    }
}

fn numeric_collection_from_value(value: &TrackValue) -> Option<(CollectionKind, Vec<f32>)> {
    match value {
        TrackValue::Vector(values) => Some((CollectionKind::Vector(values.len()), values.clone())),
        TrackValue::NumericArray(values) => {
            Some((CollectionKind::Array(values.len()), values.clone()))
        }
        _ => None,
    }
}

/// Accumulator entry storing weighted sums per value kind.
/// For vectors/colors: store component-wise sum and total weight.
/// For quaternions: store weighted sum vector of (x,y,z,w) then normalize at finalize.
/// For transforms: separate TRS accumulators.
#[derive(Clone, Debug)]
enum AccumEntry {
    Scalar {
        sum: f32,
        w: f32,
    },
    Vec2 {
        sum: [f32; 2],
        w: f32,
    },
    Vec3 {
        sum: [f32; 3],
        w: f32,
    },
    Vec4 {
        sum: [f32; 4],
        w: f32,
    },
    Quat {
        sum: [f32; 4],
        w: f32,
    },
    Color {
        sum: [f32; 4],
        w: f32,
    },
    Transform {
        t_sum: [f32; 3],
        r_sum: [f32; 4], // weighted sum; normalized at blend
        s_sum: [f32; 3],
        w: f32,
    },
    Collection {
        kind: CollectionKind,
        sum: Vec<f32>,
        w: f32,
    },
    /// Step-only kinds (Bool/Text/other): prefer last assignment, no blending
    Step(TrackValue),
}

impl AccumEntry {
    fn add_value(&mut self, v: &TrackValue, w: f32) {
        match (self, v) {
            (AccumEntry::Scalar { sum, w: ww }, TrackValue::Float(x)) => {
                *sum += x * w;
                *ww += w;
            }
            (AccumEntry::Vec2 { sum, w: ww }, TrackValue::Vec2(a)) => {
                sum[0] += a[0] * w;
                sum[1] += a[1] * w;
                *ww += w;
            }
            (AccumEntry::Vec3 { sum, w: ww }, TrackValue::Vec3(a)) => {
                sum[0] += a[0] * w;
                sum[1] += a[1] * w;
                sum[2] += a[2] * w;
                *ww += w;
            }
            (AccumEntry::Vec4 { sum, w: ww }, TrackValue::Vec4(a)) => {
                sum[0] += a[0] * w;
                sum[1] += a[1] * w;
                sum[2] += a[2] * w;
                sum[3] += a[3] * w;
                *ww += w;
            }
            (AccumEntry::Quat { sum, w: ww }, TrackValue::Quat(q)) => {
                sum[0] += q[0] * w;
                sum[1] += q[1] * w;
                sum[2] += q[2] * w;
                sum[3] += q[3] * w;
                *ww += w;
            }
            (AccumEntry::Color { sum, w: ww }, TrackValue::ColorRgba(a)) => {
                sum[0] += a[0] * w;
                sum[1] += a[1] * w;
                sum[2] += a[2] * w;
                sum[3] += a[3] * w;
                *ww += w;
            }
            (
                AccumEntry::Transform {
                    t_sum,
                    r_sum,
                    s_sum,
                    w: ww,
                },
                TrackValue::Transform(t),
            ) => {
                t_sum[0] += t.translation[0] * w;
                t_sum[1] += t.translation[1] * w;
                t_sum[2] += t.translation[2] * w;

                r_sum[0] += t.rotation[0] * w;
                r_sum[1] += t.rotation[1] * w;
                r_sum[2] += t.rotation[2] * w;
                r_sum[3] += t.rotation[3] * w;

                s_sum[0] += t.scale[0] * w;
                s_sum[1] += t.scale[1] * w;
                s_sum[2] += t.scale[2] * w;
                *ww += w;
            }
            (
                AccumEntry::Step(last),
                TrackValue::Bool(_)
                | TrackValue::Text(_)
                | TrackValue::NumericArray(_)
                | TrackValue::Step(_),
            ) => {
                *last = v.clone(); // prefer last/most-recent assignment
            }
            (entry @ AccumEntry::Collection { .. }, other) => {
                if let Some((incoming_kind, data)) = numeric_collection_from_value(other) {
                    let matches_kind = match entry {
                        AccumEntry::Collection { kind, .. } => kind.matches(&incoming_kind),
                        _ => false,
                    };
                    if matches_kind {
                        if let AccumEntry::Collection { sum, w: ww, .. } = entry {
                            debug_assert_eq!(sum.len(), data.len());
                            for (acc, component) in sum.iter_mut().zip(data.iter()) {
                                *acc += component * w;
                            }
                            *ww += w;
                        }
                    } else {
                        *entry = AccumEntry::Step(other.clone());
                    }
                } else {
                    *entry = AccumEntry::Step(other.clone());
                }
            }
            _ => {
                // Mismatched kind; ignore additional values to keep fail-soft behavior.
            }
        }
    }

    fn from_value(v: &TrackValue, w: f32) -> Self {
        match v {
            TrackValue::Float(x) => AccumEntry::Scalar { sum: *x * w, w },
            TrackValue::Vec2(a) => AccumEntry::Vec2 {
                sum: [a[0] * w, a[1] * w],
                w,
            },
            TrackValue::Vec3(a) => AccumEntry::Vec3 {
                sum: [a[0] * w, a[1] * w, a[2] * w],
                w,
            },
            TrackValue::Vec4(a) => AccumEntry::Vec4 {
                sum: [a[0] * w, a[1] * w, a[2] * w, a[3] * w],
                w,
            },
            TrackValue::Quat(q) => AccumEntry::Quat {
                sum: [q[0] * w, q[1] * w, q[2] * w, q[3] * w],
                w,
            },
            TrackValue::ColorRgba(c) => AccumEntry::Color {
                sum: [c[0] * w, c[1] * w, c[2] * w, c[3] * w],
                w,
            },
            TrackValue::Transform(t) => AccumEntry::Transform {
                t_sum: [
                    t.translation[0] * w,
                    t.translation[1] * w,
                    t.translation[2] * w,
                ],
                r_sum: [
                    t.rotation[0] * w,
                    t.rotation[1] * w,
                    t.rotation[2] * w,
                    t.rotation[3] * w,
                ],
                s_sum: [t.scale[0] * w, t.scale[1] * w, t.scale[2] * w],
                w,
            },
            TrackValue::Vector(_) | TrackValue::NumericArray(_) => {
                let (kind, mut data) =
                    numeric_collection_from_value(v).expect("numeric collection kinds");
                for entry in data.iter_mut() {
                    *entry *= w;
                }
                AccumEntry::Collection { kind, sum: data, w }
            }
            TrackValue::Bool(_) | TrackValue::Text(_) | TrackValue::Step(_) => {
                AccumEntry::Step(v.clone())
            }
        }
    }

    fn finalize(self) -> Option<Value> {
        match self {
            AccumEntry::Scalar { sum, w } => {
                if w > 0.0 {
                    Some(vocab::float(sum / w))
                } else {
                    None
                }
            }
            AccumEntry::Vec2 { sum, w } => {
                if w > 0.0 {
                    Some(vocab::vec2([sum[0] / w, sum[1] / w]))
                } else {
                    None
                }
            }
            AccumEntry::Vec3 { sum, w } => {
                if w > 0.0 {
                    Some(vocab::vec3([sum[0] / w, sum[1] / w, sum[2] / w]))
                } else {
                    None
                }
            }
            AccumEntry::Vec4 { sum, w } => {
                if w > 0.0 {
                    Some(vocab::vec4([
                        sum[0] / w,
                        sum[1] / w,
                        sum[2] / w,
                        sum[3] / w,
                    ]))
                } else {
                    None
                }
            }
            AccumEntry::Quat { sum, w } => {
                if w > 0.0 {
                    let q = [sum[0] / w, sum[1] / w, sum[2] / w, sum[3] / w];
                    // Normalize; for robustness, NLERP with identity if needed.
                    let blended = nlerp_quat(q, q, 0.0);
                    Some(vocab::quat(blended))
                } else {
                    None
                }
            }
            AccumEntry::Color { sum, w } => {
                if w > 0.0 {
                    Some(vocab::color_rgba([
                        sum[0] / w,
                        sum[1] / w,
                        sum[2] / w,
                        sum[3] / w,
                    ]))
                } else {
                    None
                }
            }
            AccumEntry::Transform {
                t_sum,
                r_sum,
                s_sum,
                w,
            } => {
                if w > 0.0 {
                    let t = [t_sum[0] / w, t_sum[1] / w, t_sum[2] / w];
                    let s = [s_sum[0] / w, s_sum[1] / w, s_sum[2] / w];
                    let r = [r_sum[0] / w, r_sum[1] / w, r_sum[2] / w, r_sum[3] / w];
                    let r_norm = nlerp_quat(r, r, 0.0);
                    Some(vocab::transform(Transform {
                        translation: t,
                        rotation: r_norm,
                        scale: s,
                    }))
                } else {
                    None
                }
            }
            AccumEntry::Collection { kind, sum, w } => {
                if w > 0.0 {
                    let averaged = sum.into_iter().map(|c| c / w).collect();
                    Some(kind.build_value(averaged))
                } else {
                    None
                }
            }
            AccumEntry::Step(v) => Some(v.into()),
        }
    }
}

/// Accumulates per-handle contributions across instances, tracking both values and optional
/// derivatives so the engine can emit aligned `(Value, Option<Value>)` pairs.
#[derive(Default)]
pub struct AccumulatorWithDerivatives {
    values: HashMap<String, AccumEntry>,
    derivatives: HashMap<String, AccumEntry>,
}

impl AccumulatorWithDerivatives {
    pub fn new() -> Self {
        Self::default()
    }

    /// Add a weighted contribution to the accumulator.
    pub fn add(
        &mut self,
        handle: &str,
        value: &TrackValue,
        derivative: Option<&TrackValue>,
        weight: f32,
    ) {
        if weight <= 0.0 {
            return;
        }

        self.values
            .entry(handle.to_string())
            .and_modify(|entry| entry.add_value(value, weight))
            .or_insert_with(|| AccumEntry::from_value(value, weight));

        if let Some(deriv) = derivative {
            self.derivatives
                .entry(handle.to_string())
                .and_modify(|entry| entry.add_value(deriv, weight))
                .or_insert_with(|| AccumEntry::from_value(deriv, weight));
        }
    }

    /// Finalize accumulated values into canonical `(value, derivative)` pairs keyed by handle.
    pub fn finalize(self) -> HashMap<String, (Value, Option<Value>)> {
        let Self {
            values,
            mut derivatives,
        } = self;

        let mut out = HashMap::with_capacity(values.len());
        for (handle, entry) in values.into_iter() {
            if let Some(value) = entry.finalize() {
                let derivative = derivatives.remove(&handle).and_then(AccumEntry::finalize);
                out.insert(handle, (value, derivative));
            }
        }
        out
    }
}
