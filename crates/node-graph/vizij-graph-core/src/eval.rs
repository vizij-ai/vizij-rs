use crate::types::{GraphSpec, InputConnection, NodeId, NodeSpec, NodeType, Value};
use hashbrown::HashMap;

#[derive(Debug, Clone, Default)]
pub struct GraphRuntime {
    pub t: f64,
    pub outputs: HashMap<NodeId, HashMap<String, Value>>,
}

fn as_float(v: &Value) -> f64 {
    match *v {
        Value::Float(f) => f,
        Value::Bool(b) => {
            if b {
                1.0
            } else {
                0.0
            }
        }
        Value::Vec3(v) => v[0],
    }
}

fn as_bool(v: &Value) -> bool {
    match *v {
        Value::Float(f) => f != 0.0,
        Value::Bool(b) => b,
        Value::Vec3(v) => v[0] != 0.0 || v[1] != 0.0 || v[2] != 0.0,
    }
}

fn as_vec3(v: &Value) -> [f64; 3] {
    match *v {
        Value::Vec3(a) => a,
        Value::Float(f) => [f, f, f],
        Value::Bool(b) => {
            if b {
                [1.0, 1.0, 1.0]
            } else {
                [0.0, 0.0, 0.0]
            }
        }
    }
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
        NodeType::Constant => out_map!(p.value.unwrap_or_default()),
        NodeType::Slider => out_map!(Value::Float(p.value.map(|v| as_float(&v)).unwrap_or(0.0))),
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
        NodeType::Add => out_map!(Value::Float(ivals.values().map(as_float).sum())),
        NodeType::Subtract => {
            let first = as_float(&get_input("lhs"));
            let second = as_float(&get_input("rhs"));
            out_map!(Value::Float(first - second))
        }
        NodeType::Multiply => {
            let product = ivals.values().map(as_float).product();
            out_map!(Value::Float(product))
        }
        NodeType::Divide => {
            let lhs = as_float(&get_input("lhs"));
            let rhs = as_float(&get_input("rhs"));
            out_map!(Value::Float(if rhs != 0.0 { lhs / rhs } else { f64::NAN }))
        }
        NodeType::Power => {
            let base = as_float(&get_input("base"));
            let exp = as_float(&get_input("exp"));
            out_map!(Value::Float(base.powf(exp)))
        }
        NodeType::Log => {
            let val = as_float(&get_input("value"));
            let base = as_float(&get_input("base"));
            out_map!(Value::Float(val.log(base)))
        }
        NodeType::Sin => out_map!(Value::Float(as_float(&get_input("in")).sin())),
        NodeType::Cos => out_map!(Value::Float(as_float(&get_input("in")).cos())),
        NodeType::Tan => out_map!(Value::Float(as_float(&get_input("in")).tan())),

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

        NodeType::Vec3 => {
            let x = as_float(&get_input("x"));
            let y = as_float(&get_input("y"));
            let z = as_float(&get_input("z"));
            out_map!(Value::Vec3([x, y, z]))
        }
        NodeType::Vec3Split => {
            let v = as_vec3(&get_input("in"));
            let mut map = HashMap::new();
            map.insert("x".to_string(), Value::Float(v[0]));
            map.insert("y".to_string(), Value::Float(v[1]));
            map.insert("z".to_string(), Value::Float(v[2]));
            map
        }
        NodeType::Vec3Add => {
            let a = as_vec3(&get_input("a"));
            let b = as_vec3(&get_input("b"));
            out_map!(Value::Vec3([a[0] + b[0], a[1] + b[1], a[2] + b[2]]))
        }
        NodeType::Vec3Subtract => {
            let a = as_vec3(&get_input("a"));
            let b = as_vec3(&get_input("b"));
            out_map!(Value::Vec3([a[0] - b[0], a[1] - b[1], a[2] - b[2]]))
        }
        NodeType::Vec3Multiply => {
            let a = as_vec3(&get_input("a"));
            let b = as_vec3(&get_input("b"));
            out_map!(Value::Vec3([a[0] * b[0], a[1] * b[1], a[2] * b[2]]))
        }
        NodeType::Vec3Scale => {
            let s = as_float(&get_input("scalar"));
            let v = as_vec3(&get_input("v"));
            out_map!(Value::Vec3([s * v[0], s * v[1], s * v[2]]))
        }
        NodeType::Vec3Normalize => {
            let v = as_vec3(&get_input("in"));
            let len = (v[0] * v[0] + v[1] * v[1] + v[2] * v[2]).sqrt();
            out_map!(if len > 0.0 {
                Value::Vec3([v[0] / len, v[1] / len, v[2] / len])
            } else {
                Value::Vec3([0.0, 0.0, 0.0])
            })
        }
        NodeType::Vec3Dot => {
            let a = as_vec3(&get_input("a"));
            let b = as_vec3(&get_input("b"));
            out_map!(Value::Float(a[0] * b[0] + a[1] * b[1] + a[2] * b[2]))
        }
        NodeType::Vec3Cross => {
            let a = as_vec3(&get_input("a"));
            let b = as_vec3(&get_input("b"));
            out_map!(Value::Vec3([
                a[1] * b[2] - a[2] * b[1],
                a[2] * b[0] - a[0] * b[2],
                a[0] * b[1] - a[1] * b[0],
            ]))
        }
        NodeType::Vec3Length => {
            let v = as_vec3(&get_input("in"));
            out_map!(Value::Float(
                (v[0] * v[0] + v[1] * v[1] + v[2] * v[2]).sqrt()
            ))
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
                    Value::Vec3([std::f64::NAN, std::f64::NAN, std::f64::NAN])
                } else {
                    let dist = dist_sq.sqrt();
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
