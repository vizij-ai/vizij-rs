//! Per-node evaluation logic for the Vizij graph runtime.

use crate::eval::graph_runtime::GraphRuntime;
use crate::eval::variadic::compare_variadic_keys;
use crate::types::{InputConnection, NodeParams, NodeSpec, NodeType};
use hashbrown::HashMap;
use vizij_api_core::{Value, WriteOp};

use super::numeric::{as_bool, as_float, binary_numeric, unary_numeric};
use super::shape_helpers::enforce_output_shapes;
use super::urdfik::{
    apply_joint_positions, fetch_joint_vector, hash_urdf_config, quat_from_value, solve_pose,
    solve_position, tip_pose, vector_from_value, IkKey,
};
use super::value_layout::{align_flattened, flatten_numeric, FlatValue, PortValue};
use super::variadic::fold_numeric_variadic;

type OutputMap = HashMap<String, PortValue>;

/// Build an output map containing a single port.
fn keyed_output(key: &str, value: Value) -> OutputMap {
    let mut map = HashMap::with_capacity(1);
    map.insert(key.to_string(), PortValue::new(value));
    map
}

/// Build an output map for the default `out` port.
fn single_output(value: Value) -> OutputMap {
    keyed_output("out", value)
}

/// Evaluate a single node, updating `rt` with new outputs and queued writes.
pub fn eval_node(rt: &mut GraphRuntime, spec: &NodeSpec) -> Result<(), String> {
    let inputs = read_inputs(rt, &spec.inputs);
    let mut outputs = evaluate_kind(rt, spec, &inputs)?;

    let pending_write = spec.params.path.as_ref().and_then(|path| {
        outputs
            .get("out")
            .map(|pv| (path.clone(), pv.value.clone()))
    });

    enforce_output_shapes(spec, &mut outputs)?;
    if let Some((path, value)) = pending_write {
        rt.writes.push(WriteOp::new(path, value));
    }
    rt.outputs.insert(spec.id.clone(), outputs);
    Ok(())
}

fn evaluate_kind(
    rt: &mut GraphRuntime,
    spec: &NodeSpec,
    inputs: &HashMap<String, PortValue>,
) -> Result<OutputMap, String> {
    let params = &spec.params;
    match &spec.kind {
        NodeType::Constant => Ok(eval_constant(params)),
        NodeType::Slider => Ok(eval_slider(params)),
        NodeType::MultiSlider => Ok(eval_multi_slider(params)),
        node_type @ (NodeType::Add
        | NodeType::Subtract
        | NodeType::Multiply
        | NodeType::Divide
        | NodeType::Power
        | NodeType::Log) => Ok(eval_arithmetic(node_type, inputs)),
        node_type @ (NodeType::Sin | NodeType::Cos | NodeType::Tan) => {
            Ok(eval_trig(node_type, inputs))
        }
        NodeType::Time => Ok(eval_time(rt)),
        NodeType::Oscillator => Ok(eval_oscillator(rt, inputs)),
        node_type @ (NodeType::Spring | NodeType::Damp | NodeType::Slew) => {
            Ok(eval_stateful(node_type, rt, spec, params, inputs))
        }
        node_type @ (NodeType::And | NodeType::Or | NodeType::Not | NodeType::Xor) => {
            Ok(eval_logic(node_type, inputs))
        }
        node_type @ (NodeType::GreaterThan
        | NodeType::LessThan
        | NodeType::Equal
        | NodeType::NotEqual) => Ok(eval_comparison(node_type, inputs)),
        NodeType::If => Ok(eval_if(inputs)),
        NodeType::Clamp => Ok(eval_clamp(inputs)),
        NodeType::Remap => Ok(eval_remap(inputs)),
        NodeType::Vec3Cross => Ok(eval_vec3_cross(inputs)),
        NodeType::VectorConstant => Ok(eval_vector_constant(params)),
        node_type @ (NodeType::VectorAdd
        | NodeType::VectorSubtract
        | NodeType::VectorMultiply
        | NodeType::VectorScale) => Ok(eval_vector_arithmetic(node_type, inputs)),
        NodeType::VectorNormalize => Ok(eval_vector_normalize(inputs)),
        NodeType::VectorDot => Ok(eval_vector_dot(inputs)),
        NodeType::VectorLength => Ok(eval_vector_length(inputs)),
        NodeType::VectorIndex => Ok(eval_vector_index(inputs)),
        NodeType::Join => Ok(eval_join(inputs)),
        NodeType::Split => Ok(eval_split(params, inputs)),
        node_type @ (NodeType::VectorMin
        | NodeType::VectorMax
        | NodeType::VectorMean
        | NodeType::VectorMedian
        | NodeType::VectorMode) => Ok(eval_vector_reducer(node_type, inputs)),
        NodeType::InverseKinematics => Ok(eval_inverse_kinematics(inputs)),
        #[cfg(feature = "urdf_ik")]
        NodeType::UrdfIkPosition => eval_urdf_position(rt, spec, params, inputs),
        #[cfg(not(feature = "urdf_ik"))]
        NodeType::UrdfIkPosition => {
            Err("UrdfIkPosition node requires the 'urdf_ik' feature".to_string())
        }
        #[cfg(feature = "urdf_ik")]
        NodeType::UrdfIkPose => eval_urdf_pose(rt, spec, params, inputs),
        #[cfg(not(feature = "urdf_ik"))]
        NodeType::UrdfIkPose => Err("UrdfIkPose node requires the 'urdf_ik' feature".to_string()),
        #[cfg(feature = "urdf_ik")]
        NodeType::UrdfFk => eval_urdf_fk(rt, spec, params, inputs),
        #[cfg(not(feature = "urdf_ik"))]
        NodeType::UrdfFk => Err("UrdfFk node requires the 'urdf_ik' feature".to_string()),
        NodeType::Output => Ok(eval_output(inputs)),
    }
}

fn input_or_default(inputs: &HashMap<String, PortValue>, key: &str) -> PortValue {
    inputs
        .get(key)
        .cloned()
        .unwrap_or_else(|| PortValue::new(Value::Float(0.0)))
}

fn eval_constant(params: &NodeParams) -> OutputMap {
    single_output(params.value.clone().unwrap_or(Value::Float(0.0)))
}

fn eval_slider(params: &NodeParams) -> OutputMap {
    single_output(Value::Float(
        params.value.as_ref().map(as_float).unwrap_or(0.0),
    ))
}

fn eval_multi_slider(params: &NodeParams) -> OutputMap {
    let mut map: OutputMap = OutputMap::default();
    map.insert(
        "x".to_string(),
        PortValue::new(Value::Float(params.x.unwrap_or(0.0))),
    );
    map.insert(
        "y".to_string(),
        PortValue::new(Value::Float(params.y.unwrap_or(0.0))),
    );
    map.insert(
        "z".to_string(),
        PortValue::new(Value::Float(params.z.unwrap_or(0.0))),
    );
    map
}

fn eval_arithmetic(kind: &NodeType, inputs: &HashMap<String, PortValue>) -> OutputMap {
    match kind {
        NodeType::Add => {
            let values: Vec<Value> = inputs.values().map(|pv| pv.value.clone()).collect();
            let result = fold_numeric_variadic(&values, |x, y| x + y, Value::Float(0.0));
            single_output(result)
        }
        NodeType::Multiply => {
            let values: Vec<Value> = inputs.values().map(|pv| pv.value.clone()).collect();
            let result = fold_numeric_variadic(&values, |x, y| x * y, Value::Float(1.0));
            single_output(result)
        }
        NodeType::Subtract => {
            let lhs = input_or_default(inputs, "lhs");
            let rhs = input_or_default(inputs, "rhs");
            single_output(binary_numeric(&lhs.value, &rhs.value, |x, y| x - y))
        }
        NodeType::Divide => {
            let lhs = input_or_default(inputs, "lhs");
            let rhs = input_or_default(inputs, "rhs");
            single_output(binary_numeric(&lhs.value, &rhs.value, |x, y| {
                if y != 0.0 {
                    x / y
                } else {
                    f32::NAN
                }
            }))
        }
        NodeType::Power => {
            let base = input_or_default(inputs, "base");
            let exp = input_or_default(inputs, "exp");
            single_output(binary_numeric(&base.value, &exp.value, |x, y| x.powf(y)))
        }
        NodeType::Log => {
            let value = input_or_default(inputs, "value");
            let base = input_or_default(inputs, "base");
            single_output(binary_numeric(&value.value, &base.value, |x, b| x.log(b)))
        }
        _ => unreachable!(),
    }
}

fn eval_trig(kind: &NodeType, inputs: &HashMap<String, PortValue>) -> OutputMap {
    let input = input_or_default(inputs, "in");
    let op = match kind {
        NodeType::Sin => f32::sin,
        NodeType::Cos => f32::cos,
        NodeType::Tan => f32::tan,
        _ => unreachable!(),
    };
    single_output(unary_numeric(&input.value, op))
}

fn eval_time(rt: &GraphRuntime) -> OutputMap {
    single_output(Value::Float(rt.t))
}

fn eval_oscillator(rt: &GraphRuntime, inputs: &HashMap<String, PortValue>) -> OutputMap {
    let freq_port = input_or_default(inputs, "frequency");
    let phase_port = input_or_default(inputs, "phase");

    let freq_value = freq_port.value;
    let phase_value = phase_port.value;

    let freq_flat = flatten_numeric(&freq_value);
    let phase_flat = flatten_numeric(&phase_value);

    let value = match (freq_flat, phase_flat) {
        (Some(freq_flat), Some(phase_flat)) => match align_flattened(&freq_flat, &phase_flat) {
            Ok((layout, freqs, phases)) => {
                let data: Vec<f32> = freqs
                    .into_iter()
                    .zip(phases)
                    .map(|(f, phase)| (std::f32::consts::TAU * f * rt.t + phase).sin())
                    .collect();
                layout.reconstruct(&data)
            }
            Err(layout) => layout.fill_with(f32::NAN),
        },
        (Some(freq_flat), None) => {
            let FlatValue {
                layout,
                data: freqs,
            } = freq_flat;
            let phase_scalar = as_float(&phase_value);
            let data: Vec<f32> = freqs
                .into_iter()
                .map(|f| (std::f32::consts::TAU * f * rt.t + phase_scalar).sin())
                .collect();
            layout.reconstruct(&data)
        }
        (None, Some(phase_flat)) => {
            let FlatValue {
                layout,
                data: phases,
            } = phase_flat;
            let freq_scalar = as_float(&freq_value);
            let data: Vec<f32> = phases
                .into_iter()
                .map(|phase| (std::f32::consts::TAU * freq_scalar * rt.t + phase).sin())
                .collect();
            layout.reconstruct(&data)
        }
        (None, None) => {
            let f = as_float(&freq_value);
            let phase = as_float(&phase_value);
            Value::Float((std::f32::consts::TAU * f * rt.t + phase).sin())
        }
    };

    single_output(value)
}

fn eval_stateful(
    kind: &NodeType,
    rt: &mut GraphRuntime,
    spec: &NodeSpec,
    params: &NodeParams,
    inputs: &HashMap<String, PortValue>,
) -> OutputMap {
    let input = input_or_default(inputs, "in");
    match (kind, flatten_numeric(&input.value)) {
        (NodeType::Spring, Some(flat)) => {
            let dt = if rt.dt.is_finite() {
                rt.dt.max(0.0)
            } else {
                0.0
            };
            let stiffness = params.stiffness.unwrap_or(120.0);
            let stiffness = if stiffness.is_finite() {
                stiffness.max(0.0)
            } else {
                0.0
            };
            let damping = params.damping.unwrap_or(20.0);
            let damping = if damping.is_finite() {
                damping.max(0.0)
            } else {
                0.0
            };
            let mass = params.mass.unwrap_or(1.0);
            const MIN_MASS: f32 = 1.0e-4;
            let mass = if mass.is_finite() {
                mass.max(MIN_MASS)
            } else {
                1.0
            };

            let state = rt.spring_state_mut(&spec.id, &flat);
            state.target = flat.data.clone();

            if dt <= 0.0 {
                state.position = state.target.clone();
                state.velocity.fill(0.0);
            } else {
                let inv_mass = 1.0 / mass;
                for ((pos, vel), target) in state
                    .position
                    .iter_mut()
                    .zip(state.velocity.iter_mut())
                    .zip(state.target.iter())
                {
                    let displacement = *pos - *target;
                    let spring_force = -stiffness * displacement;
                    let damping_force = -damping * *vel;
                    let acceleration = (spring_force + damping_force) * inv_mass;
                    *vel += acceleration * dt;
                    *pos += *vel * dt;
                }
            }

            single_output(state.layout.reconstruct(&state.position))
        }
        (NodeType::Damp, Some(flat)) => {
            let dt = if rt.dt.is_finite() {
                rt.dt.max(0.0)
            } else {
                0.0
            };
            let half_life = params.half_life.unwrap_or(0.1);
            let half_life = if half_life.is_finite() {
                half_life
            } else {
                0.1
            };
            let state = rt.damp_state_mut(&spec.id, &flat);
            if dt <= 0.0 || half_life <= 0.0 {
                state.value = flat.data.clone();
            } else {
                let hl = half_life.max(1.0e-6);
                let decay = (-std::f32::consts::LN_2 * dt / hl).exp();
                for (value, target) in state.value.iter_mut().zip(flat.data.iter()) {
                    *value = *target + (*value - *target) * decay;
                }
            }
            single_output(state.layout.reconstruct(&state.value))
        }
        (NodeType::Slew, Some(flat)) => {
            let dt = if rt.dt.is_finite() {
                rt.dt.max(0.0)
            } else {
                0.0
            };
            let max_rate = params.max_rate.unwrap_or(1.0);
            let max_rate = if max_rate.is_finite() { max_rate } else { 1.0 };
            let state = rt.slew_state_mut(&spec.id, &flat);
            if dt <= 0.0 || max_rate <= 0.0 {
                state.value = flat.data.clone();
            } else {
                let max_delta = max_rate * dt;
                for (value, target) in state.value.iter_mut().zip(flat.data.iter()) {
                    let delta = *target - *value;
                    if delta.abs() <= max_delta {
                        *value = *target;
                    } else if delta > 0.0 {
                        *value += max_delta;
                    } else {
                        *value -= max_delta;
                    }
                }
            }
            single_output(state.layout.reconstruct(&state.value))
        }
        _ => single_output(Value::Float(f32::NAN)),
    }
}

fn eval_logic(kind: &NodeType, inputs: &HashMap<String, PortValue>) -> OutputMap {
    let value = match kind {
        NodeType::And => {
            as_bool(&input_or_default(inputs, "lhs").value)
                && as_bool(&input_or_default(inputs, "rhs").value)
        }
        NodeType::Or => {
            as_bool(&input_or_default(inputs, "lhs").value)
                || as_bool(&input_or_default(inputs, "rhs").value)
        }
        NodeType::Xor => {
            as_bool(&input_or_default(inputs, "lhs").value)
                ^ as_bool(&input_or_default(inputs, "rhs").value)
        }
        NodeType::Not => !as_bool(&input_or_default(inputs, "in").value),
        _ => unreachable!(),
    };
    single_output(Value::Bool(value))
}

fn eval_comparison(kind: &NodeType, inputs: &HashMap<String, PortValue>) -> OutputMap {
    let lhs = input_or_default(inputs, "lhs");
    let rhs = input_or_default(inputs, "rhs");
    let value = match kind {
        NodeType::GreaterThan => as_float(&lhs.value) > as_float(&rhs.value),
        NodeType::LessThan => as_float(&lhs.value) < as_float(&rhs.value),
        NodeType::Equal => (as_float(&lhs.value) - as_float(&rhs.value)).abs() < 1e-6,
        NodeType::NotEqual => (as_float(&lhs.value) - as_float(&rhs.value)).abs() > 1e-6,
        _ => unreachable!(),
    };
    single_output(Value::Bool(value))
}

fn eval_if(inputs: &HashMap<String, PortValue>) -> OutputMap {
    let cond = as_bool(&input_or_default(inputs, "cond").value);
    let branch = if cond {
        input_or_default(inputs, "then")
    } else {
        input_or_default(inputs, "else")
    };
    single_output(branch.value)
}

fn eval_clamp(inputs: &HashMap<String, PortValue>) -> OutputMap {
    let value = input_or_default(inputs, "in");
    let min = input_or_default(inputs, "min");
    let max = input_or_default(inputs, "max");
    let clamped_low = binary_numeric(&value.value, &min.value, |x, m| x.max(m));
    single_output(binary_numeric(&clamped_low, &max.value, |x, m| x.min(m)))
}

fn eval_remap(inputs: &HashMap<String, PortValue>) -> OutputMap {
    let value = input_or_default(inputs, "in");
    let in_min = input_or_default(inputs, "in_min");
    let in_max = input_or_default(inputs, "in_max");
    let out_min = input_or_default(inputs, "out_min");
    let out_max = input_or_default(inputs, "out_max");

    let numer = binary_numeric(&value.value, &in_min.value, |v, min| v - min);
    let denom = binary_numeric(&in_max.value, &in_min.value, |max, min| max - min);
    let ratio = binary_numeric(
        &numer,
        &denom,
        |n, d| if d != 0.0 { n / d } else { f32::NAN },
    );
    let ratio_clamped = unary_numeric(&ratio, |x| x.clamp(0.0, 1.0));
    let span = binary_numeric(&out_max.value, &out_min.value, |max, min| max - min);
    let scaled = binary_numeric(&ratio_clamped, &span, |t, span| t * span);
    single_output(binary_numeric(&scaled, &out_min.value, |scaled, min| {
        scaled + min
    }))
}

fn eval_vec3_cross(inputs: &HashMap<String, PortValue>) -> OutputMap {
    let a = input_or_default(inputs, "a");
    let b = input_or_default(inputs, "b");
    match (flatten_numeric(&a.value), flatten_numeric(&b.value)) {
        (Some(a_flat), Some(b_flat))
            if a_flat.layout.scalar_len() == 3 && b_flat.layout.scalar_len() == 3 =>
        {
            let ax = a_flat.data[0];
            let ay = a_flat.data[1];
            let az = a_flat.data[2];
            let bx = b_flat.data[0];
            let by = b_flat.data[1];
            let bz = b_flat.data[2];
            let result = vec![ay * bz - az * by, az * bx - ax * bz, ax * by - ay * bx];
            single_output(a_flat.layout.reconstruct(&result))
        }
        (Some(a_flat), _) => single_output(a_flat.layout.fill_with(f32::NAN)),
        (_, Some(b_flat)) => single_output(b_flat.layout.fill_with(f32::NAN)),
        _ => single_output(Value::Vec3([f32::NAN, f32::NAN, f32::NAN])),
    }
}

fn eval_vector_constant(params: &NodeParams) -> OutputMap {
    if let Some(value) = &params.value {
        single_output(value.clone())
    } else {
        single_output(Value::Vector(Vec::new()))
    }
}

fn eval_vector_arithmetic(kind: &NodeType, inputs: &HashMap<String, PortValue>) -> OutputMap {
    match kind {
        NodeType::VectorAdd => {
            let a = input_or_default(inputs, "a");
            let b = input_or_default(inputs, "b");
            single_output(binary_numeric(&a.value, &b.value, |x, y| x + y))
        }
        NodeType::VectorSubtract => {
            let a = input_or_default(inputs, "a");
            let b = input_or_default(inputs, "b");
            single_output(binary_numeric(&a.value, &b.value, |x, y| x - y))
        }
        NodeType::VectorMultiply => {
            let a = input_or_default(inputs, "a");
            let b = input_or_default(inputs, "b");
            single_output(binary_numeric(&a.value, &b.value, |x, y| x * y))
        }
        NodeType::VectorScale => {
            let vector = input_or_default(inputs, "v");
            let scalar = input_or_default(inputs, "scalar");
            single_output(binary_numeric(&vector.value, &scalar.value, |x, s| x * s))
        }
        _ => unreachable!(),
    }
}

fn eval_vector_normalize(inputs: &HashMap<String, PortValue>) -> OutputMap {
    let value = input_or_default(inputs, "in");
    match flatten_numeric(&value.value) {
        Some(flat) => {
            let len_sq: f32 = flat.data.iter().map(|x| x * x).sum();
            let len = len_sq.sqrt();
            let normalized: Vec<f32> = if len > 0.0 {
                flat.data.iter().map(|x| *x / len).collect()
            } else {
                vec![f32::NAN; flat.data.len()]
            };
            single_output(flat.layout.reconstruct(&normalized))
        }
        None => single_output(Value::Float(f32::NAN)),
    }
}

fn eval_vector_dot(inputs: &HashMap<String, PortValue>) -> OutputMap {
    let a = input_or_default(inputs, "a");
    let b = input_or_default(inputs, "b");
    match (flatten_numeric(&a.value), flatten_numeric(&b.value)) {
        (Some(lhs), Some(rhs)) => match align_flattened(&lhs, &rhs) {
            Ok((_, da, db)) => {
                let sum = da.iter().zip(db.iter()).map(|(x, y)| x * y).sum::<f32>();
                single_output(Value::Float(sum))
            }
            Err(_) => single_output(Value::Float(f32::NAN)),
        },
        _ => single_output(Value::Float(f32::NAN)),
    }
}

fn eval_vector_length(inputs: &HashMap<String, PortValue>) -> OutputMap {
    let value = input_or_default(inputs, "in");
    match flatten_numeric(&value.value) {
        Some(flat) => {
            let len_sq: f32 = flat.data.iter().map(|x| x * x).sum();
            single_output(Value::Float(len_sq.sqrt()))
        }
        None => single_output(Value::Float(f32::NAN)),
    }
}

fn eval_vector_index(inputs: &HashMap<String, PortValue>) -> OutputMap {
    let value = input_or_default(inputs, "v");
    let index = as_float(&input_or_default(inputs, "index").value);
    if let Some(flat) = flatten_numeric(&value.value) {
        let i = index.floor() as isize;
        let len = flat.data.len() as isize;
        if i >= 0 && i < len {
            single_output(Value::Float(flat.data[i as usize]))
        } else {
            single_output(Value::Float(f32::NAN))
        }
    } else {
        single_output(Value::Float(f32::NAN))
    }
}

fn eval_join(inputs: &HashMap<String, PortValue>) -> OutputMap {
    let mut entries: Vec<_> = inputs.iter().collect();
    entries.sort_by(|(a, _), (b, _)| compare_variadic_keys(a, b));

    let mut out: Vec<f32> = Vec::new();
    for (_, port) in entries {
        if let Some(flat) = flatten_numeric(&port.value) {
            out.extend(flat.data);
        }
    }
    single_output(Value::Vector(out))
}

fn eval_split(params: &NodeParams, inputs: &HashMap<String, PortValue>) -> OutputMap {
    let mut map: OutputMap = OutputMap::default();
    let input = input_or_default(inputs, "in");
    let data = flatten_numeric(&input.value)
        .map(|f| f.data)
        .unwrap_or_default();
    let sizes = params.sizes.clone().unwrap_or_default();
    let sizes_usize: Vec<usize> = sizes.iter().map(|x| x.floor().max(0.0) as usize).collect();

    if sizes_usize.is_empty() {
        map.insert(
            "part1".to_string(),
            PortValue::new(Value::Vector(Vec::new())),
        );
        return map;
    }

    let total: usize = sizes_usize.iter().sum();
    if total == data.len() {
        let mut offset = 0usize;
        for (i, sz) in sizes_usize.iter().copied().enumerate() {
            let slice = data[offset..offset + sz].to_vec();
            offset += sz;
            map.insert(
                format!("part{}", i + 1),
                PortValue::new(Value::Vector(slice)),
            );
        }
    } else {
        for (i, sz) in sizes_usize.iter().copied().enumerate() {
            map.insert(
                format!("part{}", i + 1),
                PortValue::new(Value::Vector(vec![f32::NAN; sz])),
            );
        }
    }
    map
}

fn eval_vector_reducer(kind: &NodeType, inputs: &HashMap<String, PortValue>) -> OutputMap {
    let mut data = flatten_numeric(&input_or_default(inputs, "in").value)
        .map(|f| f.data)
        .unwrap_or_default();
    let result = match kind {
        NodeType::VectorMin => {
            if data.is_empty() {
                f32::NAN
            } else {
                data.iter().fold(f32::INFINITY, |acc, x| acc.min(*x))
            }
        }
        NodeType::VectorMax => {
            if data.is_empty() {
                f32::NAN
            } else {
                data.iter().fold(f32::NEG_INFINITY, |acc, x| acc.max(*x))
            }
        }
        NodeType::VectorMean => {
            if data.is_empty() {
                f32::NAN
            } else {
                data.iter().sum::<f32>() / (data.len() as f32)
            }
        }
        NodeType::VectorMedian => {
            if data.is_empty() {
                f32::NAN
            } else {
                data.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
                let n = data.len();
                if n % 2 == 1 {
                    data[n / 2]
                } else {
                    (data[n / 2 - 1] + data[n / 2]) / 2.0
                }
            }
        }
        NodeType::VectorMode => {
            if data.is_empty() {
                f32::NAN
            } else {
                let mut counts: hashbrown::HashMap<i64, (f32, usize)> = hashbrown::HashMap::new();
                for value in data {
                    if value.is_nan() {
                        continue;
                    }
                    let key = value.to_bits() as i64;
                    let entry = counts.entry(key).or_insert((value, 0));
                    entry.1 += 1;
                }
                if counts.is_empty() {
                    f32::NAN
                } else {
                    let mut best_val = f32::NAN;
                    let mut best_count = 0usize;
                    for (_key, (val, cnt)) in counts.iter() {
                        if *cnt > best_count {
                            best_count = *cnt;
                            best_val = *val;
                        } else if *cnt == best_count && val < &best_val {
                            best_val = *val;
                        }
                    }
                    best_val
                }
            }
        }
        _ => f32::NAN,
    };
    single_output(Value::Float(result))
}

fn eval_inverse_kinematics(inputs: &HashMap<String, PortValue>) -> OutputMap {
    let l1 = as_float(&input_or_default(inputs, "bone1").value);
    let l2 = as_float(&input_or_default(inputs, "bone2").value);
    let l3 = as_float(&input_or_default(inputs, "bone3").value);
    let theta = as_float(&input_or_default(inputs, "theta").value);
    let x = as_float(&input_or_default(inputs, "x").value);
    let y = as_float(&input_or_default(inputs, "y").value);

    let wx = x - l3 * theta.cos();
    let wy = y - l3 * theta.sin();
    let dist_sq = wx * wx + wy * wy;

    let value = if dist_sq > (l1 + l2) * (l1 + l2) || dist_sq < (l1 - l2) * (l1 - l2) {
        Value::Vec3([f32::NAN, f32::NAN, f32::NAN])
    } else {
        let cos_angle2 = (dist_sq - l1 * l1 - l2 * l2) / (2.0 * l1 * l2);
        let angle2 = cos_angle2.acos();
        let angle1 = wy.atan2(wx) - (l2 * angle2.sin()).atan2(l1 + l2 * angle2.cos());
        let angle3 = theta - angle1 - angle2;
        Value::Vec3([angle1, angle2, angle3])
    };

    single_output(value)
}

#[cfg(feature = "urdf_ik")]
fn eval_urdf_fk(
    rt: &mut GraphRuntime,
    spec: &NodeSpec,
    params: &NodeParams,
    inputs: &HashMap<String, PortValue>,
) -> Result<OutputMap, String> {
    let joints_port = inputs
        .get("joints")
        .ok_or_else(|| "UrdfFk requires 'joints' input".to_string())?;

    let urdf_xml = params
        .urdf_xml
        .as_deref()
        .filter(|s| !s.trim().is_empty())
        .ok_or_else(|| "UrdfFk requires non-empty 'urdf_xml' param".to_string())?;
    let root_link = params.root_link.as_deref().unwrap_or("base_link");
    let tip_link = params.tip_link.as_deref().unwrap_or("tool0");

    let key = IkKey {
        hash: hash_urdf_config(urdf_xml, root_link, tip_link),
        urdf_xml,
        root_link,
        tip_link,
    };
    let state = rt.kinematics_state_mut(&spec.id, key)?;

    let joints = fetch_joint_vector(
        &joints_port.value,
        state.dofs,
        params.joint_defaults.as_deref(),
        &state.joint_names,
    )?;

    apply_joint_positions(state, &joints)?;

    let (pos_arr, rot_arr) = tip_pose(state);
    let position_value = Value::Vec3(pos_arr);
    let rotation_value = Value::Quat(rot_arr);
    let transform_value = match (&position_value, &rotation_value) {
        (Value::Vec3(pos), Value::Quat(rot)) => Value::Transform {
            pos: *pos,
            rot: *rot,
            scale: [1.0, 1.0, 1.0],
        },
        _ => unreachable!(),
    };

    let mut outputs = HashMap::with_capacity(3);
    outputs.insert("position".to_string(), PortValue::new(position_value));
    outputs.insert("rotation".to_string(), PortValue::new(rotation_value));
    outputs.insert("transform".to_string(), PortValue::new(transform_value));

    Ok(outputs)
}

#[cfg(feature = "urdf_ik")]
fn eval_urdf_position(
    rt: &mut GraphRuntime,
    spec: &NodeSpec,
    params: &NodeParams,
    inputs: &HashMap<String, PortValue>,
) -> Result<OutputMap, String> {
    let target_pos = match input_or_default(inputs, "target_pos").value {
        Value::Vec3(arr) => arr,
        other => {
            return Err(format!(
                "UrdfIkPosition input 'target_pos' expects Vec3, received {:?}",
                other.kind()
            ));
        }
    };

    let urdf_xml = params
        .urdf_xml
        .as_deref()
        .filter(|s| !s.trim().is_empty())
        .ok_or_else(|| "UrdfIkPosition requires non-empty 'urdf_xml' param".to_string())?;
    let root_link = params.root_link.as_deref().unwrap_or("base_link");
    let tip_link = params.tip_link.as_deref().unwrap_or("tool0");

    let key = IkKey {
        hash: hash_urdf_config(urdf_xml, root_link, tip_link),
        urdf_xml,
        root_link,
        tip_link,
    };
    let state = rt.kinematics_state_mut(&spec.id, key)?;
    let dofs = state.dofs;
    let mut solver = k::JacobianIkSolver::default();

    let seed_candidate: Option<Vec<f32>> = inputs
        .get("seed")
        .map(|port| vector_from_value(&port.value, "UrdfIkPosition seed"))
        .transpose()?
        .or_else(|| params.seed.clone());

    let seed_provided = seed_candidate.is_some();
    let mut seed = seed_candidate.unwrap_or_else(|| state.chain.joint_positions());

    if seed.len() != dofs {
        if seed_provided {
            return Err(format!(
                "UrdfIkPosition seed length {} does not match chain DoF {dofs}",
                seed.len()
            ));
        }
        seed = vec![0.0; dofs];
    }

    let weights_ref = params.weights.as_ref().filter(|w| !w.is_empty());
    if let Some(weights) = weights_ref {
        if weights.len() != dofs {
            return Err(format!(
                "UrdfIkPosition weights length {} does not match chain DoF {dofs}",
                weights.len()
            ));
        }
    }
    let weights = weights_ref.map(|w| w.as_slice());

    let solution = solve_position(
        state,
        &mut solver,
        target_pos,
        seed.as_slice(),
        weights,
        params.max_iters.unwrap_or(100),
        params.tol_pos.unwrap_or(1e-3),
    )?;

    Ok(single_output(state.solution_record(&solution)))
}

#[cfg(feature = "urdf_ik")]
fn eval_urdf_pose(
    rt: &mut GraphRuntime,
    spec: &NodeSpec,
    params: &NodeParams,
    inputs: &HashMap<String, PortValue>,
) -> Result<OutputMap, String> {
    let target_pos = match input_or_default(inputs, "target_pos").value {
        Value::Vec3(arr) => arr,
        other => {
            return Err(format!(
                "UrdfIkPose input 'target_pos' expects Vec3, received {:?}",
                other.kind()
            ));
        }
    };
    let target_rot = {
        let port = input_or_default(inputs, "target_rot");
        quat_from_value(&port.value, "UrdfIkPose target_rot")?
    };

    let urdf_xml = params
        .urdf_xml
        .as_deref()
        .filter(|s| !s.trim().is_empty())
        .ok_or_else(|| "UrdfIkPose requires non-empty 'urdf_xml' param".to_string())?;
    let root_link = params.root_link.as_deref().unwrap_or("base_link");
    let tip_link = params.tip_link.as_deref().unwrap_or("tool0");

    let key = IkKey {
        hash: hash_urdf_config(urdf_xml, root_link, tip_link),
        urdf_xml,
        root_link,
        tip_link,
    };
    let state = rt.kinematics_state_mut(&spec.id, key)?;
    let dofs = state.dofs;
    let mut solver = k::JacobianIkSolver::default();

    let seed_candidate: Option<Vec<f32>> = inputs
        .get("seed")
        .map(|port| vector_from_value(&port.value, "UrdfIkPose seed"))
        .transpose()?
        .or_else(|| params.seed.clone());

    let seed_provided = seed_candidate.is_some();
    let mut seed = seed_candidate.unwrap_or_else(|| state.chain.joint_positions());

    if seed.len() != dofs {
        if seed_provided {
            return Err(format!(
                "UrdfIkPose seed length {} does not match chain DoF {dofs}",
                seed.len()
            ));
        }
        seed = vec![0.0; dofs];
    }

    let weights_ref = params.weights.as_ref().filter(|w| !w.is_empty());
    if let Some(weights) = weights_ref {
        if weights.len() != dofs {
            return Err(format!(
                "UrdfIkPose weights length {} does not match chain DoF {dofs}",
                weights.len()
            ));
        }
    }
    let weights = weights_ref.map(|w| w.as_slice());

    let solution = solve_pose(
        state,
        &mut solver,
        target_pos,
        target_rot,
        seed.as_slice(),
        weights,
        params.max_iters.unwrap_or(100),
        params.tol_pos.unwrap_or(1e-3),
        params.tol_rot.unwrap_or(1e-3),
    )?;

    Ok(single_output(state.solution_record(&solution)))
}

fn eval_output(inputs: &HashMap<String, PortValue>) -> OutputMap {
    single_output(input_or_default(inputs, "in").value)
}
/// Gather the most recent outputs for each of the node's input connections.
fn read_inputs(
    rt: &GraphRuntime,
    inputs: &HashMap<String, InputConnection>,
) -> HashMap<String, PortValue> {
    inputs
        .iter()
        .map(|(input_key, conn)| {
            let val = rt
                .outputs
                .get(&conn.node_id)
                .and_then(|outputs| outputs.get(&conn.output_key))
                .cloned()
                .unwrap_or_else(|| PortValue::new(Value::Float(0.0)));
            (input_key.clone(), val)
        })
        .collect()
}
