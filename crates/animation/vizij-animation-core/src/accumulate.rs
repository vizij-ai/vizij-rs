#![allow(dead_code)]
//! Accumulation of per-target contributions and blending into final Values.

use std::collections::HashMap;

use crate::interp::functions::nlerp_quat;
use crate::value::Value;

/// Accumulator entry storing weighted sums per Value kind.
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
    /// Step-only kinds (Bool/Text): prefer last assignment, no blending
    Step(Value),
}

impl AccumEntry {
    fn add_value(&mut self, v: &Value, w: f32) {
        match (self, v) {
            (AccumEntry::Scalar { sum, w: ww }, Value::Scalar(x)) => {
                *sum += x * w;
                *ww += w;
            }
            (AccumEntry::Vec2 { sum, w: ww }, Value::Vec2(a)) => {
                sum[0] += a[0] * w;
                sum[1] += a[1] * w;
                *ww += w;
            }
            (AccumEntry::Vec3 { sum, w: ww }, Value::Vec3(a)) => {
                sum[0] += a[0] * w;
                sum[1] += a[1] * w;
                sum[2] += a[2] * w;
                *ww += w;
            }
            (AccumEntry::Vec4 { sum, w: ww }, Value::Vec4(a)) => {
                sum[0] += a[0] * w;
                sum[1] += a[1] * w;
                sum[2] += a[2] * w;
                sum[3] += a[3] * w;
                *ww += w;
            }
            (AccumEntry::Quat { sum, w: ww }, Value::Quat(q)) => {
                sum[0] += q[0] * w;
                sum[1] += q[1] * w;
                sum[2] += q[2] * w;
                sum[3] += q[3] * w;
                *ww += w;
            }
            (AccumEntry::Color { sum, w: ww }, Value::Color(a)) => {
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
                Value::Transform {
                    translation,
                    rotation,
                    scale,
                },
            ) => {
                t_sum[0] += translation[0] * w;
                t_sum[1] += translation[1] * w;
                t_sum[2] += translation[2] * w;

                r_sum[0] += rotation[0] * w;
                r_sum[1] += rotation[1] * w;
                r_sum[2] += rotation[2] * w;
                r_sum[3] += rotation[3] * w;

                s_sum[0] += scale[0] * w;
                s_sum[1] += scale[1] * w;
                s_sum[2] += scale[2] * w;
                *ww += w;
            }
            (AccumEntry::Step(last), Value::Bool(_)) => {
                *last = v.clone(); // prefer last/most-recent assignment
            }
            (AccumEntry::Step(last), Value::Text(_)) => {
                *last = v.clone(); // prefer last/most-recent assignment
            }
            _ => {
                // Mismatched kind; ignore additional values to keep fail-soft behavior.
            }
        }
    }

    fn from_value(v: &Value, w: f32) -> Self {
        match v {
            Value::Scalar(x) => AccumEntry::Scalar { sum: *x * w, w },
            Value::Vec2(a) => AccumEntry::Vec2 {
                sum: [a[0] * w, a[1] * w],
                w,
            },
            Value::Vec3(a) => AccumEntry::Vec3 {
                sum: [a[0] * w, a[1] * w, a[2] * w],
                w,
            },
            Value::Vec4(a) => AccumEntry::Vec4 {
                sum: [a[0] * w, a[1] * w, a[2] * w, a[3] * w],
                w,
            },
            Value::Quat(q) => AccumEntry::Quat {
                sum: [q[0] * w, q[1] * w, q[2] * w, q[3] * w],
                w,
            },
            Value::Color(c) => AccumEntry::Color {
                sum: [c[0] * w, c[1] * w, c[2] * w, c[3] * w],
                w,
            },
            Value::Transform {
                translation,
                rotation,
                scale,
            } => AccumEntry::Transform {
                t_sum: [translation[0] * w, translation[1] * w, translation[2] * w],
                r_sum: [
                    rotation[0] * w,
                    rotation[1] * w,
                    rotation[2] * w,
                    rotation[3] * w,
                ],
                s_sum: [scale[0] * w, scale[1] * w, scale[2] * w],
                w,
            },
            Value::Text(s) => AccumEntry::Step(Value::Text(s.clone())),
            Value::Bool(b) => AccumEntry::Step(Value::Bool(*b)),
        }
    }

    fn finalize(self) -> Option<Value> {
        match self {
            AccumEntry::Scalar { sum, w } => {
                if w > 0.0 {
                    Some(Value::Scalar(sum / w))
                } else {
                    None
                }
            }
            AccumEntry::Vec2 { sum, w } => {
                if w > 0.0 {
                    Some(Value::Vec2([sum[0] / w, sum[1] / w]))
                } else {
                    None
                }
            }
            AccumEntry::Vec3 { sum, w } => {
                if w > 0.0 {
                    Some(Value::Vec3([sum[0] / w, sum[1] / w, sum[2] / w]))
                } else {
                    None
                }
            }
            AccumEntry::Vec4 { sum, w } => {
                if w > 0.0 {
                    Some(Value::Vec4([
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
                    Some(Value::Quat(blended))
                } else {
                    None
                }
            }
            AccumEntry::Color { sum, w } => {
                if w > 0.0 {
                    Some(Value::Color([
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
                    Some(Value::Transform {
                        translation: t,
                        rotation: r_norm,
                        scale: s,
                    })
                } else {
                    None
                }
            }
            AccumEntry::Step(v) => Some(v),
        }
    }
}

/// Accumulates per-handle contributions across instances.
#[derive(Default)]
pub struct Accumulator {
    map: HashMap<String, AccumEntry>,
}

impl Accumulator {
    pub fn new() -> Self {
        Self {
            map: HashMap::new(),
        }
    }

    pub fn add(&mut self, handle: &str, value: &Value, weight: f32) {
        if weight <= 0.0 {
            return;
        }
        self.map
            .entry(handle.to_string())
            .and_modify(|entry| entry.add_value(value, weight))
            .or_insert_with(|| AccumEntry::from_value(value, weight));
    }

    pub fn finalize(self) -> HashMap<String, Value> {
        let mut out = HashMap::new();
        for (k, entry) in self.map.into_iter() {
            if let Some(v) = entry.finalize() {
                out.insert(k, v);
            }
        }
        out
    }
}
