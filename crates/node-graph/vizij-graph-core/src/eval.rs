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
        NodeType::Power => Value::Float(as_float(&ivals[0]).powf(as_float(&ivals[1]))),
        NodeType::Log => {
            let base = if ivals.len() > 1 { as_float(&ivals[1]) } else { std::f64::consts::E };
            Value::Float(as_float(&ivals[0]).log(base))
        }
        NodeType::Sin => Value::Float(as_float(&ivals[0]).sin()),
        NodeType::Cos => Value::Float(as_float(&ivals[0]).cos()),
        NodeType::Tan => Value::Float(as_float(&ivals[0]).tan()),

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

        NodeType::GreaterThan => Value::Bool(as_float(&ivals[0]) > as_float(&ivals[1])),
        NodeType::LessThan => Value::Bool(as_float(&ivals[0]) < as_float(&ivals[1])),
        NodeType::Equal => Value::Bool((as_float(&ivals[0]) - as_float(&ivals[1])).abs() < 1e-9),
        NodeType::NotEqual => Value::Bool((as_float(&ivals[0]) - as_float(&ivals[1])).abs() > 1e-9),
        NodeType::If => {
            if ivals.len() < 3 { Value::default() } else {
                if as_bool(&ivals[0]) { ivals[1] } else { ivals[2] }
            }
        }

        NodeType::Clamp => {
            let x = as_float(&ivals[0]);
            let min = p.min;
            let max = p.max;
            Value::Float(x.clamp(min, max))
        }

        NodeType::Remap => {
            let x = as_float(&ivals[0]);
            let in_min = p.in_min.unwrap_or(0.0);
            let in_max = p.in_max.unwrap_or(1.0);
            let out_min = p.out_min.unwrap_or(0.0);
            let out_max = p.out_max.unwrap_or(1.0);
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
            let v = ivals.get(0).cloned().unwrap_or(Value::Vec3([0.0,0.0,0.0]));
            let [x,y,z] = as_vec3(&v);
            // Convention: publish X/Y/Z onto synthetic node ids id.x / id.y / id.z
            // but since our runtime stores one value per node id, we return Vec3 and let hosts split if needed.
            Value::Vec3([x,y,z])
        }
        NodeType::Vec3Add => {
            let a = as_vec3(&ivals[0]); let b = as_vec3(&ivals[1]);
            Value::Vec3([a[0]+b[0], a[1]+b[1], a[2]+b[2]])
        }
        NodeType::Vec3Subtract => {
            let a = as_vec3(&ivals[0]); let b = as_vec3(&ivals[1]);
            Value::Vec3([a[0]-b[0], a[1]-b[1], a[2]-b[2]])
        }
        NodeType::Vec3Multiply => {
            let a = as_vec3(&ivals[0]); let b = as_vec3(&ivals[1]);
            Value::Vec3([a[0]*b[0], a[1]*b[1], a[2]*b[2]])
        }
        NodeType::Vec3Scale => {
            let s = as_float(&ivals[0]); let v = as_vec3(&ivals[1]);
            Value::Vec3([s*v[0], s*v[1], s*v[2]])
        }
        NodeType::Vec3Normalize => {
            let v = as_vec3(&ivals[0]);
            let len = (v[0]*v[0] + v[1]*v[1] + v[2]*v[2]).sqrt();
            if len > 0.0 { Value::Vec3([v[0]/len, v[1]/len, v[2]/len]) } else { Value::Vec3([0.0,0.0,0.0]) }
        }
        NodeType::Vec3Dot => {
            let a = as_vec3(&ivals[0]); let b = as_vec3(&ivals[1]);
            Value::Float(a[0]*b[0] + a[1]*b[1] + a[2]*b[2])
        }
        NodeType::Vec3Cross => {
            let a = as_vec3(&ivals[0]); let b = as_vec3(&ivals[1]);
            Value::Vec3([
                a[1]*b[2] - a[2]*b[1],
                a[2]*b[0] - a[0]*b[2],
                a[0]*b[1] - a[1]*b[0],
            ])
        }
        NodeType::Vec3Length => {
            let v = as_vec3(&ivals[0]);
            Value::Float((v[0]*v[0] + v[1]*v[1] + v[2]*v[2]).sqrt())
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
