use crate::types::*;
use hashbrown::HashMap;

#[derive(Debug, Clone, Default)]
pub struct GraphRuntime {
    pub t: f64,
    pub outputs: HashMap<NodeId, Value>,
}

fn as_float(v: &Value) -> f64 {
    match *v {
        Value::Float(f) => f,
        Value::Bool(b) => if b { 1.0 } else { 0.0 },
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
        Value::Bool(b) => if b { [1.0,1.0,1.0] } else { [0.0,0.0,0.0] },
    }
}

fn read_inputs(rt: &GraphRuntime, inputs: &[NodeId]) -> Vec<Value> {
    inputs.iter().map(|id| rt.outputs.get(id).cloned().unwrap_or_default()).collect()
}

pub fn eval_node(rt: &mut GraphRuntime, spec: &NodeSpec) {
    let ivals = read_inputs(rt, &spec.inputs);
    let t = rt.t;
    let p = &spec.params;

    let v = match spec.kind {
        NodeType::Constant => p.value.unwrap_or_default(),
        NodeType::Slider => Value::Float(p.value.map(|v| as_float(&v)).unwrap_or(0.0)),
        NodeType::Add => Value::Float(ivals.iter().map(as_float).sum()),
        NodeType::Subtract => {
            if ivals.is_empty() { Value::Float(0.0) } else {
                let first = as_float(&ivals[0]);
                let rest: f64 = ivals[1..].iter().map(as_float).sum();
                Value::Float(first - rest)
            }
        }
        NodeType::Multiply => Value::Float(ivals.iter().map(as_float).product()),
        NodeType::Divide => {
            if ivals.len() < 2 { Value::Float(f64::NAN) } else {
                let mut it = ivals.iter().map(as_float);
                let init = it.next().unwrap();
                let res = it.fold(init, |acc, x| acc / x);
                Value::Float(res)
            }
        }
        NodeType::Power => {
            let base = ivals.get(0).map(as_float).unwrap_or(0.0);
            let exp = ivals.get(1).map(as_float).unwrap_or(0.0);
            Value::Float(base.powf(exp))
        }
        NodeType::Log => {
            let val = ivals.get(0).map(as_float).unwrap_or(1.0);
            let base = ivals.get(1).map(as_float).unwrap_or(std::f64::consts::E);
            Value::Float(val.log(base))
        }
        NodeType::Sin => Value::Float(ivals.get(0).map(as_float).unwrap_or(0.0).sin()),
        NodeType::Cos => Value::Float(ivals.get(0).map(as_float).unwrap_or(0.0).cos()),
        NodeType::Tan => Value::Float(ivals.get(0).map(as_float).unwrap_or(0.0).tan()),

        NodeType::Time => Value::Float(t),
        NodeType::Oscillator => {
            let f = ivals.get(0).map(as_float).unwrap_or(p.frequency.unwrap_or(1.0));
            let phase = ivals.get(1).map(as_float).unwrap_or(p.phase.unwrap_or(0.0));
            Value::Float((std::f64::consts::TAU * f * t + phase).sin())
        }

        NodeType::And => Value::Bool(ivals.iter().all(|v| as_bool(v))),
        NodeType::Or => Value::Bool(ivals.iter().any(|v| as_bool(v))),
        NodeType::Not => Value::Bool(!as_bool(ivals.get(0).unwrap_or(&Value::Bool(false)))),
        NodeType::Xor => {
            if ivals.len() < 2 { Value::Bool(false) } else { Value::Bool(as_bool(&ivals[0]) ^ as_bool(&ivals[1])) }
        }

        NodeType::GreaterThan => {
            let a = as_float(ivals.get(0).unwrap_or(&Value::default()));
            let b = as_float(ivals.get(1).unwrap_or(&Value::default()));
            Value::Bool(a > b)
        }
        NodeType::LessThan => {
            let a = as_float(ivals.get(0).unwrap_or(&Value::default()));
            let b = as_float(ivals.get(1).unwrap_or(&Value::default()));
            Value::Bool(a < b)
        }
        NodeType::Equal => {
            let a = as_float(ivals.get(0).unwrap_or(&Value::default()));
            let b = as_float(ivals.get(1).unwrap_or(&Value::default()));
            Value::Bool((a - b).abs() < 1e-9)
        }
        NodeType::NotEqual => {
            let a = as_float(ivals.get(0).unwrap_or(&Value::default()));
            let b = as_float(ivals.get(1).unwrap_or(&Value::default()));
            Value::Bool((a - b).abs() > 1e-9)
        }
        NodeType::If => {
            if ivals.len() < 3 { Value::default() } else {
                if as_bool(&ivals[0]) { ivals[1] } else { ivals[2] }
            }
        }

        NodeType::Clamp => {
            let x = as_float(ivals.get(0).unwrap_or(&Value::default()));
            let max = ivals.get(1).map(as_float).unwrap_or(p.max);
            let min = ivals.get(2).map(as_float).unwrap_or(p.min);
            Value::Float(x.clamp(min, max))
        }

        NodeType::Remap => {
            let x = as_float(ivals.get(0).unwrap_or(&Value::default()));
            let in_min = ivals.get(1).map(as_float).or(p.in_min).unwrap_or(0.0);
            let in_max = ivals.get(2).map(as_float).or(p.in_max).unwrap_or(1.0);
            let out_min = ivals.get(3).map(as_float).or(p.out_min).unwrap_or(0.0);
            let out_max = ivals.get(4).map(as_float).or(p.out_max).unwrap_or(1.0);
            let t = ((x - in_min) / (in_max - in_min)).clamp(0.0, 1.0);
            Value::Float(out_min + t * (out_max - out_min))
        }

        NodeType::Vec3 => {
            // Prefer inputs (x,y,z), fallback to params (x,y,z), fallback to Value::value if Vec3
            let x = ivals.get(0).map(as_float).or(p.x).unwrap_or(0.0);
            let y = ivals.get(1).map(as_float).or(p.y).unwrap_or(0.0);
            let z = ivals.get(2).map(as_float).or(p.z).unwrap_or(0.0);
            Value::Vec3([x,y,z])
        }
        NodeType::Vec3Split => {
            let v = as_vec3(ivals.get(0).unwrap_or(&Value::default()));
            let index = p.index.unwrap_or(0.0) as usize;
            if index < 3 {
                Value::Float(v[index])
            } else {
                Value::Float(std::f64::NAN)
            }
        }
        NodeType::Vec3Add => {
            let a = as_vec3(ivals.get(0).unwrap_or(&Value::default()));
            let b = as_vec3(ivals.get(1).unwrap_or(&Value::default()));
            Value::Vec3([a[0] + b[0], a[1] + b[1], a[2] + b[2]])
        }
        NodeType::Vec3Subtract => {
            let a = as_vec3(ivals.get(0).unwrap_or(&Value::default()));
            let b = as_vec3(ivals.get(1).unwrap_or(&Value::default()));
            Value::Vec3([a[0] - b[0], a[1] - b[1], a[2] - b[2]])
        }
        NodeType::Vec3Multiply => {
            let a = as_vec3(ivals.get(0).unwrap_or(&Value::default()));
            let b = as_vec3(ivals.get(1).unwrap_or(&Value::default()));
            Value::Vec3([a[0] * b[0], a[1] * b[1], a[2] * b[2]])
        }
        NodeType::Vec3Scale => {
            let s = as_float(ivals.get(0).unwrap_or(&Value::default()));
            let v = as_vec3(ivals.get(1).unwrap_or(&Value::default()));
            Value::Vec3([s * v[0], s * v[1], s * v[2]])
        }
        NodeType::Vec3Normalize => {
            let v = as_vec3(ivals.get(0).unwrap_or(&Value::default()));
            let len = (v[0] * v[0] + v[1] * v[1] + v[2] * v[2]).sqrt();
            if len > 0.0 {
                Value::Vec3([v[0] / len, v[1] / len, v[2] / len])
            } else {
                Value::Vec3([0.0, 0.0, 0.0])
            }
        }
        NodeType::Vec3Dot => {
            let a = as_vec3(ivals.get(0).unwrap_or(&Value::default()));
            let b = as_vec3(ivals.get(1).unwrap_or(&Value::default()));
            Value::Float(a[0] * b[0] + a[1] * b[1] + a[2] * b[2])
        }
        NodeType::Vec3Cross => {
            let a = as_vec3(ivals.get(0).unwrap_or(&Value::default()));
            let b = as_vec3(ivals.get(1).unwrap_or(&Value::default()));
            Value::Vec3([
                a[1] * b[2] - a[2] * b[1],
                a[2] * b[0] - a[0] * b[2],
                a[0] * b[1] - a[1] * b[0],
            ])
        }
        NodeType::Vec3Length => {
            let v = as_vec3(ivals.get(0).unwrap_or(&Value::default()));
            Value::Float((v[0] * v[0] + v[1] * v[1] + v[2] * v[2]).sqrt())
        }

        NodeType::InverseKinematics => {
            // 3-bone IK for a target pose (x, y, theta)
            let l1 = ivals.get(0).map(as_float).or(p.bone1).unwrap_or(1.0);
            let l2 = ivals.get(1).map(as_float).or(p.bone2).unwrap_or(1.0);
            let l3 = ivals.get(2).map(as_float).or(p.bone3).unwrap_or(0.5);
            let theta = ivals.get(3).map(as_float).unwrap_or(0.0);
            let x = ivals.get(4).map(as_float).unwrap_or(0.0);
            let y = ivals.get(5).map(as_float).unwrap_or(0.0);

            // Position of the wrist (end of bone 2)
            let wx = x - l3 * theta.cos();
            let wy = y - l3 * theta.sin();

            let dist_sq = wx * wx + wy * wy;

            // Check reachability for the first two bones
            if dist_sq > (l1 + l2) * (l1 + l2) || dist_sq < (l1 - l2) * (l1 - l2) {
                Value::Vec3([std::f64::NAN, std::f64::NAN, std::f64::NAN]) // Indicate failure
            } else {
                let dist = dist_sq.sqrt();
                let cos_angle2 = (dist_sq - l1 * l1 - l2 * l2) / (2.0 * l1 * l2);
                let angle2 = cos_angle2.acos(); // Joint 2 angle

                let angle1 = wy.atan2(wx) - (l2 * angle2.sin()).atan2(l1 + l2 * angle2.cos()); // Joint 1 angle

                let angle3 = theta - angle1 - angle2; // Joint 3 angle

                Value::Vec3([angle1, angle2, angle3])
            }
        }

        NodeType::Output => ivals.get(0).cloned().unwrap_or_default(),
    };

    rt.outputs.insert(spec.id.clone(), v);
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

