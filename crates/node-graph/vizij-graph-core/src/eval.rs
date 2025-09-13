use crate::types::{GraphSpec, InputConnection, NodeId, NodeSpec, NodeType, Value};
use hashbrown::HashMap;

#[derive(Debug, Clone, Default)]
pub struct GraphRuntime {
    pub t: f64,
    pub outputs: HashMap<NodeId, HashMap<String, Value>>,
}

fn as_float(v: &Value) -> f64 {
    match v {
        Value::Float(f) => *f,
        Value::Bool(b) => {
            if *b {
                1.0
            } else {
                0.0
            }
        }
        Value::Vec3(v) => v[0],
        Value::Vector(a) => a.first().copied().unwrap_or(0.0),
    }
}

fn as_bool(v: &Value) -> bool {
    match v {
        Value::Float(f) => *f != 0.0,
        Value::Bool(b) => *b,
        Value::Vec3(v) => v[0] != 0.0 || v[1] != 0.0 || v[2] != 0.0,
        Value::Vector(a) => a.iter().any(|x| *x != 0.0),
    }
}

fn as_vector(v: &Value) -> Vec<f64> {
    match v {
        Value::Vector(a) => a.clone(),
        Value::Vec3(a) => vec![a[0], a[1], a[2]],
        Value::Float(f) => vec![*f],
        Value::Bool(b) => {
            if *b {
                vec![1.0]
            } else {
                vec![0.0]
            }
        }
    }
}

fn elementwise_bin_op<F>(a: &[f64], b: &[f64], f: F) -> Vec<f64>
where
    F: Fn(f64, f64) -> f64,
{
    if a.len() == b.len() {
        a.iter().zip(b.iter()).map(|(x, y)| f(*x, *y)).collect()
    } else if a.len() == 1 {
        let x = a[0];
        b.iter().map(|&y| f(x, y)).collect()
    } else if b.len() == 1 {
        let y = b[0];
        a.iter().map(|&x| f(x, y)).collect()
    } else {
        let len = a.len().max(b.len());
        vec![f64::NAN; len]
    }
}

fn length_squared(v: &[f64]) -> f64 {
    v.iter().map(|x| x * x).sum()
}

fn map_unary<F>(a: &[f64], f: F) -> Vec<f64>
where
    F: Fn(f64) -> f64,
{
    a.iter().copied().map(f).collect()
}

fn fold_variadic<'a, F>(mut acc: Vec<f64>, rest: impl Iterator<Item = &'a [f64]>, f: F) -> Vec<f64>
where
    F: Fn(f64, f64) -> f64 + Copy,
{
    for next in rest {
        acc = elementwise_bin_op(&acc, next, f);
    }
    acc
}

fn read_inputs(
    rt: &GraphRuntime,
    inputs: &HashMap<String, InputConnection>,
) -> HashMap<String, Value> {
    inputs
        .iter()
        .map(|(input_key, conn)| {
            let val = rt
                .outputs
                .get(&conn.node_id)
                .and_then(|outputs| outputs.get(&conn.output_key))
                .cloned()
                .unwrap_or_default();
            (input_key.clone(), val)
        })
        .collect()
}

macro_rules! out_map {
    ($key:expr, $val:expr) => {{
        let mut map = HashMap::new();
        map.insert($key.to_string(), $val);
        map
    }};
    ($val:expr) => {
        out_map!("out", $val)
    };
}

pub fn eval_node(rt: &mut GraphRuntime, spec: &NodeSpec) {
    let ivals = read_inputs(rt, &spec.inputs);
    let t = rt.t;
    let p = &spec.params;

    let get_input = |key: &str| ivals.get(key).cloned().unwrap_or_default();

    let outputs = match spec.kind {
        NodeType::Constant => out_map!(p.value.clone().unwrap_or_default()),
        NodeType::Slider => out_map!(Value::Float(p.value.as_ref().map(as_float).unwrap_or(0.0))),
        NodeType::MultiSlider => {
            let mut map = HashMap::new();
            let x = p.x.unwrap_or(0.0);
            let y = p.y.unwrap_or(0.0);
            let z = p.z.unwrap_or(0.0);
            map.insert("x".to_string(), Value::Float(x));
            map.insert("y".to_string(), Value::Float(y));
            map.insert("z".to_string(), Value::Float(z));
            map
        }
        NodeType::Add => {
            // Variadic add (vector-first with broadcasting)
            let ops: Vec<Vec<f64>> = ivals.values().map(as_vector).collect();
            if let Some((first, rest)) = ops.split_first() {
                let acc =
                    fold_variadic(first.clone(), rest.iter().map(|v| v.as_slice()), |x, y| {
                        x + y
                    });
                out_map!(Value::Vector(acc))
            } else {
                out_map!(Value::Vector(Vec::new()))
            }
        }
        NodeType::Subtract => {
            // Binary subtract (vector-first with broadcasting)
            let a = as_vector(&get_input("lhs"));
            let b = as_vector(&get_input("rhs"));
            let out = elementwise_bin_op(&a, &b, |x, y| x - y);
            out_map!(Value::Vector(out))
        }
        NodeType::Multiply => {
            // Variadic multiply (vector-first with broadcasting)
            let ops: Vec<Vec<f64>> = ivals.values().map(as_vector).collect();
            if let Some((first, rest)) = ops.split_first() {
                let acc =
                    fold_variadic(first.clone(), rest.iter().map(|v| v.as_slice()), |x, y| {
                        x * y
                    });
                out_map!(Value::Vector(acc))
            } else {
                out_map!(Value::Vector(Vec::new()))
            }
        }
        NodeType::Divide => {
            // Binary divide (vector-first with broadcasting)
            let a = as_vector(&get_input("lhs"));
            let b = as_vector(&get_input("rhs"));
            let out = elementwise_bin_op(&a, &b, |x, y| if y != 0.0 { x / y } else { f64::NAN });
            out_map!(Value::Vector(out))
        }
        NodeType::Power => {
            // Vectorized power with broadcasting
            let base = as_vector(&get_input("base"));
            let exp = as_vector(&get_input("exp"));
            let out = elementwise_bin_op(&base, &exp, |x, y| x.powf(y));
            out_map!(Value::Vector(out))
        }
        NodeType::Log => {
            // Vectorized log with broadcasting
            let val = as_vector(&get_input("value"));
            let base = as_vector(&get_input("base"));
            let out = elementwise_bin_op(&val, &base, |x, b| x.log(b));
            out_map!(Value::Vector(out))
        }
        NodeType::Sin => {
            let v = as_vector(&get_input("in"));
            let out = map_unary(&v, |x| x.sin());
            out_map!(Value::Vector(out))
        }
        NodeType::Cos => {
            let v = as_vector(&get_input("in"));
            let out = map_unary(&v, |x| x.cos());
            out_map!(Value::Vector(out))
        }
        NodeType::Tan => {
            let v = as_vector(&get_input("in"));
            let out = map_unary(&v, |x| x.tan());
            out_map!(Value::Vector(out))
        }

        NodeType::Time => out_map!(Value::Float(t)),
        NodeType::Oscillator => {
            let f = as_float(&get_input("frequency"));
            let phase = as_float(&get_input("phase"));
            out_map!(Value::Float((std::f64::consts::TAU * f * t + phase).sin()))
        }

        NodeType::And => out_map!(Value::Bool(
            as_bool(&get_input("lhs")) && as_bool(&get_input("rhs"))
        )),
        NodeType::Or => out_map!(Value::Bool(
            as_bool(&get_input("lhs")) || as_bool(&get_input("rhs"))
        )),
        NodeType::Not => out_map!(Value::Bool(!as_bool(&get_input("in")))),
        NodeType::Xor => out_map!(Value::Bool(
            as_bool(&get_input("lhs")) ^ as_bool(&get_input("rhs"))
        )),

        NodeType::GreaterThan => out_map!(Value::Bool(
            as_float(&get_input("lhs")) > as_float(&get_input("rhs"))
        )),
        NodeType::LessThan => out_map!(Value::Bool(
            as_float(&get_input("lhs")) < as_float(&get_input("rhs"))
        )),
        NodeType::Equal => out_map!(Value::Bool(
            (as_float(&get_input("lhs")) - as_float(&get_input("rhs"))).abs() < 1e-9
        )),
        NodeType::NotEqual => out_map!(Value::Bool(
            (as_float(&get_input("lhs")) - as_float(&get_input("rhs"))).abs() > 1e-9
        )),
        NodeType::If => {
            let cond = as_bool(&get_input("cond"));
            out_map!(if cond {
                get_input("then")
            } else {
                get_input("else")
            })
        }

        NodeType::Clamp => {
            let x = as_float(&get_input("in"));
            let min = as_float(&get_input("min"));
            let max = as_float(&get_input("max"));
            out_map!(Value::Float(x.clamp(min, max)))
        }

        NodeType::Remap => {
            let x = as_float(&get_input("in"));
            let in_min = as_float(&get_input("in_min"));
            let in_max = as_float(&get_input("in_max"));
            let out_min = as_float(&get_input("out_min"));
            let out_max = as_float(&get_input("out_max"));
            let t = ((x - in_min) / (in_max - in_min)).clamp(0.0, 1.0);
            out_map!(Value::Float(out_min + t * (out_max - out_min)))
        }

        NodeType::Vec3Cross => {
            let a = as_vector(&get_input("a"));
            let b = as_vector(&get_input("b"));
            if a.len() == 3 && b.len() == 3 {
                out_map!(Value::Vec3([
                    a[1] * b[2] - a[2] * b[1],
                    a[2] * b[0] - a[0] * b[2],
                    a[0] * b[1] - a[1] * b[0],
                ]))
            } else {
                out_map!(Value::Vec3([f64::NAN, f64::NAN, f64::NAN]))
            }
        }

        // Generic vector utilities
        NodeType::VectorConstant => {
            let vec = p.value.as_ref().map(as_vector).unwrap_or_default();
            out_map!(Value::Vector(vec))
        }
        NodeType::VectorAdd => {
            let a = as_vector(&get_input("a"));
            let b = as_vector(&get_input("b"));
            let out = elementwise_bin_op(&a, &b, |x, y| x + y);
            out_map!(Value::Vector(out))
        }
        NodeType::VectorSubtract => {
            let a = as_vector(&get_input("a"));
            let b = as_vector(&get_input("b"));
            let out = elementwise_bin_op(&a, &b, |x, y| x - y);
            out_map!(Value::Vector(out))
        }
        NodeType::VectorMultiply => {
            let a = as_vector(&get_input("a"));
            let b = as_vector(&get_input("b"));
            let out = elementwise_bin_op(&a, &b, |x, y| x * y);
            out_map!(Value::Vector(out))
        }
        NodeType::VectorScale => {
            let s = as_float(&get_input("scalar"));
            let v = as_vector(&get_input("v"));
            let out: Vec<f64> = v.iter().map(|x| s * x).collect();
            out_map!(Value::Vector(out))
        }
        NodeType::VectorNormalize => {
            let v = as_vector(&get_input("in"));
            let len = length_squared(&v).sqrt();
            let out = if len > 0.0 {
                v.iter().map(|x| x / len).collect()
            } else {
                vec![0.0; v.len()]
            };
            out_map!(Value::Vector(out))
        }
        NodeType::VectorDot => {
            let a = as_vector(&get_input("a"));
            let b = as_vector(&get_input("b"));
            let out = if a.len() == b.len() {
                a.iter().zip(b.iter()).map(|(x, y)| x * y).sum::<f64>()
            } else {
                f64::NAN
            };
            out_map!(Value::Float(out))
        }
        NodeType::VectorLength => {
            let v = as_vector(&get_input("in"));
            out_map!(Value::Float(length_squared(&v).sqrt()))
        }
        NodeType::VectorIndex => {
            let v = as_vector(&get_input("v"));
            let idx_f = as_float(&get_input("index"));
            let idx = idx_f.floor() as isize;
            let out = if idx >= 0 && (idx as usize) < v.len() {
                Value::Float(v[idx as usize])
            } else {
                Value::Float(f64::NAN)
            };
            out_map!(out)
        }

        // Placeholder implementations to satisfy exhaustive match (schema may not expose these yet)

        // Join: flatten all inputs (order of map iteration is arbitrary here)
        NodeType::Join => {
            let mut out: Vec<f64> = Vec::new();
            for v in ivals.values() {
                out.extend(as_vector(v));
            }
            out_map!(Value::Vector(out))
        }

        // Split: sizes param (floored). Emit variadic outputs: part1..partN.
        // If sum(sizes) != len(v), produce NaN vectors for each requested size.
        NodeType::Split => {
            let v = as_vector(&get_input("in"));
            let sizes = p.sizes.clone().unwrap_or_default();
            let sizes_usize: Vec<usize> =
                sizes.iter().map(|x| x.floor().max(0.0) as usize).collect();

            let mut map = HashMap::new();
            if sizes_usize.is_empty() {
                // No sizes specified: emit a single empty vector as 'part1'
                map.insert("part1".to_string(), Value::Vector(Vec::new()));
                map
            } else {
                let total: usize = sizes_usize.iter().sum();
                if total == v.len() {
                    let mut offset = 0usize;
                    for (i, sz) in sizes_usize.iter().copied().enumerate() {
                        let slice = v[offset..offset + sz].to_vec();
                        offset += sz;
                        map.insert(format!("part{}", i + 1), Value::Vector(slice));
                    }
                    map
                } else {
                    // Mismatch: return NaN vectors for each requested size
                    for (i, sz) in sizes_usize.iter().copied().enumerate() {
                        map.insert(format!("part{}", i + 1), Value::Vector(vec![f64::NAN; sz]));
                    }
                    map
                }
            }
        }

        // Reducers (vector -> scalar)
        NodeType::VectorMin => {
            let v = as_vector(&get_input("in"));
            let out = if v.is_empty() {
                f64::NAN
            } else {
                v.iter().copied().fold(f64::INFINITY, f64::min)
            };
            out_map!(Value::Float(out))
        }
        NodeType::VectorMax => {
            let v = as_vector(&get_input("in"));
            let out = if v.is_empty() {
                f64::NAN
            } else {
                v.iter().copied().fold(f64::NEG_INFINITY, f64::max)
            };
            out_map!(Value::Float(out))
        }
        NodeType::VectorMean => {
            let v = as_vector(&get_input("in"));
            let out = if v.is_empty() {
                f64::NAN
            } else {
                v.iter().sum::<f64>() / (v.len() as f64)
            };
            out_map!(Value::Float(out))
        }
        NodeType::VectorMedian => {
            let mut v = as_vector(&get_input("in"));
            let out = if v.is_empty() {
                f64::NAN
            } else {
                v.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
                let n = v.len();
                if n % 2 == 1 {
                    v[n / 2]
                } else {
                    (v[n / 2 - 1] + v[n / 2]) / 2.0
                }
            };
            out_map!(Value::Float(out))
        }
        NodeType::VectorMode => {
            let v = as_vector(&get_input("in"));
            let out = if v.is_empty() {
                f64::NAN
            } else {
                // Count frequencies; tie -> smallest numeric value
                let mut map: hashbrown::HashMap<i64, (f64, usize)> = hashbrown::HashMap::new();
                // quantize to i64 for frequency on floats; for more precision use epsilon-binning later
                for x in v {
                    if x.is_nan() {
                        continue;
                    }
                    let key = x.to_bits() as i64;
                    let entry = map.entry(key).or_insert((x, 0));
                    entry.1 += 1;
                }
                if map.is_empty() {
                    f64::NAN
                } else {
                    let mut best_val = f64::NAN;
                    let mut best_count = 0usize;
                    for (_k, (val, cnt)) in map.iter() {
                        if *cnt > best_count {
                            best_count = *cnt;
                            best_val = *val;
                        } else if *cnt == best_count {
                            // tie -> smallest numeric value
                            if val < &best_val {
                                best_val = *val;
                            }
                        }
                    }
                    best_val
                }
            };
            out_map!(Value::Float(out))
        }

        NodeType::InverseKinematics => {
            let l1 = as_float(&get_input("bone1"));
            let l2 = as_float(&get_input("bone2"));
            let l3 = as_float(&get_input("bone3"));
            let theta = as_float(&get_input("theta"));
            let x = as_float(&get_input("x"));
            let y = as_float(&get_input("y"));

            let wx = x - l3 * theta.cos();
            let wy = y - l3 * theta.sin();
            let dist_sq = wx * wx + wy * wy;

            out_map!(
                if dist_sq > (l1 + l2) * (l1 + l2) || dist_sq < (l1 - l2) * (l1 - l2) {
                    Value::Vec3([f64::NAN, f64::NAN, f64::NAN])
                } else {
                    // let dist = dist_sq.sqrt();
                    let cos_angle2 = (dist_sq - l1 * l1 - l2 * l2) / (2.0 * l1 * l2);
                    let angle2 = cos_angle2.acos();
                    let angle1 = wy.atan2(wx) - (l2 * angle2.sin()).atan2(l1 + l2 * angle2.cos());
                    let angle3 = theta - angle1 - angle2;
                    Value::Vec3([angle1, angle2, angle3])
                }
            )
        }

        NodeType::Output => out_map!(get_input("in")),
    };

    rt.outputs.insert(spec.id.clone(), outputs);
}

pub fn evaluate_all(rt: &mut GraphRuntime, spec: &GraphSpec) -> Result<(), String> {
    let order = crate::topo::topo_order(&spec.nodes)?;
    for id in order {
        if let Some(node) = spec.nodes.iter().find(|n| n.id == id) {
            eval_node(rt, node);
        }
    }
    Ok(())
}
