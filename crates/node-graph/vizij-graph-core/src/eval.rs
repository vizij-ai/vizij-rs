// Adapted to use vizij_api_core::Value (f32-based) and f32 arithmetic.

use crate::types::{GraphSpec, InputConnection, NodeId, NodeSpec, NodeType};
use hashbrown::{hash_map::Entry, HashMap};
#[cfg(feature = "urdf_ik")]
use k::InverseKinematicsSolver;
use std::cmp::Ordering;
#[cfg(feature = "urdf_ik")]
use std::collections::hash_map::DefaultHasher;
#[cfg(feature = "urdf_ik")]
use std::fmt;
#[cfg(feature = "urdf_ik")]
use std::hash::{Hash, Hasher};
use vizij_api_core::shape::Field;
use vizij_api_core::{coercion, Shape, ShapeId, Value, WriteBatch, WriteOp};

#[derive(Clone, Debug, PartialEq)]
enum ValueLayout {
    Scalar,
    Vec2,
    Vec3,
    Vec4,
    Quat,
    ColorRgba,
    Transform,
    Vector(usize),
    Record(Vec<(String, ValueLayout)>),
    Array(Vec<ValueLayout>),
    List(Vec<ValueLayout>),
    Tuple(Vec<ValueLayout>),
}

#[derive(Clone, Debug)]
struct FlatValue {
    layout: ValueLayout,
    data: Vec<f32>,
}

impl ValueLayout {
    fn scalar_len(&self) -> usize {
        match self {
            ValueLayout::Scalar => 1,
            ValueLayout::Vec2 => 2,
            ValueLayout::Vec3 => 3,
            ValueLayout::Vec4 => 4,
            ValueLayout::Quat => 4,
            ValueLayout::ColorRgba => 4,
            ValueLayout::Transform => 10,
            ValueLayout::Vector(len) => *len,
            ValueLayout::Record(fields) => {
                fields.iter().map(|(_, layout)| layout.scalar_len()).sum()
            }
            ValueLayout::Array(items) | ValueLayout::List(items) | ValueLayout::Tuple(items) => {
                items.iter().map(|layout| layout.scalar_len()).sum()
            }
        }
    }

    fn is_scalar(&self) -> bool {
        matches!(self, ValueLayout::Scalar)
    }

    fn reconstruct(&self, data: &[f32]) -> Value {
        match self {
            ValueLayout::Scalar => Value::Float(data.first().copied().unwrap_or(f32::NAN)),
            ValueLayout::Vec2 => {
                let mut arr = [0.0; 2];
                for (i, slot) in arr.iter_mut().enumerate() {
                    *slot = *data.get(i).unwrap_or(&f32::NAN);
                }
                Value::Vec2(arr)
            }
            ValueLayout::Vec3 => {
                let mut arr = [0.0; 3];
                for (i, slot) in arr.iter_mut().enumerate() {
                    *slot = *data.get(i).unwrap_or(&f32::NAN);
                }
                Value::Vec3(arr)
            }
            ValueLayout::Vec4 => {
                let mut arr = [0.0; 4];
                for (i, slot) in arr.iter_mut().enumerate() {
                    *slot = *data.get(i).unwrap_or(&f32::NAN);
                }
                Value::Vec4(arr)
            }
            ValueLayout::Quat => {
                let mut arr = [0.0; 4];
                for (i, slot) in arr.iter_mut().enumerate() {
                    *slot = *data.get(i).unwrap_or(&f32::NAN);
                }
                Value::Quat(arr)
            }
            ValueLayout::ColorRgba => {
                let mut arr = [0.0; 4];
                for (i, slot) in arr.iter_mut().enumerate() {
                    *slot = *data.get(i).unwrap_or(&f32::NAN);
                }
                Value::ColorRgba(arr)
            }
            ValueLayout::Transform => {
                let mut pos = [0.0; 3];
                let mut rot = [0.0; 4];
                let mut scale = [0.0; 3];
                for (i, slot) in pos.iter_mut().enumerate() {
                    *slot = *data.get(i).unwrap_or(&f32::NAN);
                }
                for (i, slot) in rot.iter_mut().enumerate() {
                    *slot = *data.get(3 + i).unwrap_or(&f32::NAN);
                }
                for (i, slot) in scale.iter_mut().enumerate() {
                    *slot = *data.get(7 + i).unwrap_or(&f32::NAN);
                }
                Value::Transform { pos, rot, scale }
            }
            ValueLayout::Vector(len) => {
                let mut out = Vec::with_capacity(*len);
                out.extend((0..*len).map(|i| *data.get(i).unwrap_or(&f32::NAN)));
                Value::Vector(out)
            }
            ValueLayout::Record(fields) => {
                let mut map = HashMap::new();
                let mut offset = 0usize;
                for (key, layout) in fields.iter() {
                    let len = layout.scalar_len();
                    let slice = &data[offset..offset + len];
                    offset += len;
                    map.insert(key.clone(), layout.reconstruct(slice));
                }
                Value::Record(map)
            }
            ValueLayout::Array(items) => {
                let mut out = Vec::with_capacity(items.len());
                let mut offset = 0usize;
                for layout in items.iter() {
                    let len = layout.scalar_len();
                    let slice = &data[offset..offset + len];
                    offset += len;
                    out.push(layout.reconstruct(slice));
                }
                Value::Array(out)
            }
            ValueLayout::List(items) => {
                let mut out = Vec::with_capacity(items.len());
                let mut offset = 0usize;
                for layout in items.iter() {
                    let len = layout.scalar_len();
                    let slice = &data[offset..offset + len];
                    offset += len;
                    out.push(layout.reconstruct(slice));
                }
                Value::List(out)
            }
            ValueLayout::Tuple(items) => {
                let mut out = Vec::with_capacity(items.len());
                let mut offset = 0usize;
                for layout in items.iter() {
                    let len = layout.scalar_len();
                    let slice = &data[offset..offset + len];
                    offset += len;
                    out.push(layout.reconstruct(slice));
                }
                Value::Tuple(out)
            }
        }
    }

    fn fill_with(&self, value: f32) -> Value {
        let len = self.scalar_len();
        let data = vec![value; len];
        self.reconstruct(&data)
    }
}

#[derive(Clone, Debug)]
struct SpringState {
    layout: ValueLayout,
    position: Vec<f32>,
    velocity: Vec<f32>,
    target: Vec<f32>,
}

impl SpringState {
    fn new(flat: &FlatValue) -> Self {
        let len = flat.data.len();
        SpringState {
            layout: flat.layout.clone(),
            position: flat.data.clone(),
            velocity: vec![0.0; len],
            target: flat.data.clone(),
        }
    }

    fn reset(&mut self, flat: &FlatValue) {
        let len = flat.data.len();
        self.layout = flat.layout.clone();
        self.position = flat.data.clone();
        self.velocity = vec![0.0; len];
        self.target = flat.data.clone();
    }
}

#[derive(Clone, Debug)]
struct DampState {
    layout: ValueLayout,
    value: Vec<f32>,
}

impl DampState {
    fn new(flat: &FlatValue) -> Self {
        DampState {
            layout: flat.layout.clone(),
            value: flat.data.clone(),
        }
    }

    fn reset(&mut self, flat: &FlatValue) {
        self.layout = flat.layout.clone();
        self.value = flat.data.clone();
    }
}

#[derive(Clone, Debug)]
struct SlewState {
    layout: ValueLayout,
    value: Vec<f32>,
}

impl SlewState {
    fn new(flat: &FlatValue) -> Self {
        SlewState {
            layout: flat.layout.clone(),
            value: flat.data.clone(),
        }
    }

    fn reset(&mut self, flat: &FlatValue) {
        self.layout = flat.layout.clone();
        self.value = flat.data.clone();
    }
}

#[derive(Debug)]
enum NodeRuntimeState {
    Spring(SpringState),
    Damp(DampState),
    Slew(SlewState),
    #[cfg(feature = "urdf_ik")]
    UrdfIk(UrdfIkState),
}

#[cfg(feature = "urdf_ik")]
struct UrdfIkState {
    hash: u64,
    dofs: usize,
    joint_names: Vec<String>,
    chain: k::SerialChain<f32>,
    solver: k::JacobianIkSolver<f32>,
}

#[cfg(feature = "urdf_ik")]
impl fmt::Debug for UrdfIkState {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("UrdfIkState")
            .field("hash", &self.hash)
            .field("dofs", &self.dofs)
            .field("joint_names", &self.joint_names)
            .finish()
    }
}

#[cfg(feature = "urdf_ik")]
impl UrdfIkState {
    fn new(hash: u64, chain: k::SerialChain<f32>, joint_names: Vec<String>) -> Self {
        let dofs = chain.dof();
        if dofs > 0 {
            let zero_seed = vec![0.0f32; dofs];
            chain.set_joint_positions_clamped(&zero_seed);
        }
        UrdfIkState {
            hash,
            dofs,
            joint_names,
            chain,
            solver: k::JacobianIkSolver::default(),
        }
    }

    fn solution_record(&self, joints: &[f32]) -> Value {
        let mut map = HashMap::with_capacity(self.joint_names.len());
        for (name, angle) in self.joint_names.iter().zip(joints.iter()) {
            map.insert(name.clone(), Value::Float(*angle));
        }
        Value::Record(map)
    }
}

#[cfg(feature = "urdf_ik")]
struct IkKey<'a> {
    hash: u64,
    urdf_xml: &'a str,
    root_link: &'a str,
    tip_link: &'a str,
}

#[cfg(feature = "urdf_ik")]
fn hash_urdf_config(urdf_xml: &str, root_link: &str, tip_link: &str) -> u64 {
    let mut hasher = DefaultHasher::new();
    urdf_xml.hash(&mut hasher);
    root_link.hash(&mut hasher);
    tip_link.hash(&mut hasher);
    hasher.finish()
}

#[cfg(feature = "urdf_ik")]
fn build_chain_from_urdf(
    urdf_xml: &str,
    root: &str,
    tip: &str,
) -> Result<(k::SerialChain<f32>, Vec<String>), String> {
    if urdf_xml.trim().is_empty() {
        return Err("URDF XML is empty".to_string());
    }

    let robot = urdf_rs::read_from_string(urdf_xml)
        .map_err(|err| format!("failed to parse URDF: {err}"))?;

    let link_to_joint = k::urdf::link_to_joint_map(&robot);
    let tip_joint = link_to_joint
        .get(tip)
        .ok_or_else(|| format!("tip link '{tip}' not found in URDF"))?;
    let root_joint = link_to_joint
        .get(root)
        .ok_or_else(|| format!("root link '{root}' not found in URDF"))?;

    let chain = k::Chain::<f32>::from(robot);

    let tip_node = chain
        .find(tip_joint)
        .ok_or_else(|| format!("tip joint '{tip_joint}' not found in chain"))?;

    let root_node = if root_joint == k::urdf::ROOT_JOINT_NAME {
        chain
            .iter()
            .next()
            .ok_or_else(|| "URDF chain is empty".to_string())?
    } else {
        chain
            .find(root_joint)
            .ok_or_else(|| format!("root joint '{root_joint}' not found in chain"))?
    };

    let serial = k::SerialChain::from_end_to_root(tip_node, root_node);

    let mut iter = serial.iter();
    let first = iter
        .next()
        .ok_or_else(|| format!("no chain found between '{root}' and '{tip}'"))?;
    let mut last = first;
    for node in iter {
        last = node;
    }

    let first_joint = first.joint().name.clone();
    if root_joint != k::urdf::ROOT_JOINT_NAME && first_joint != *root_joint {
        return Err(format!(
            "root link '{root}' (joint '{root_joint}') is not an ancestor of tip '{tip}'"
        ));
    }

    let tip_joint_name = last.joint().name.clone();
    if tip_joint_name != *tip_joint {
        return Err(format!(
            "tip link '{tip}' (joint '{tip_joint}') is not reachable from root '{root}'"
        ));
    }

    let joint_names: Vec<String> = serial
        .iter_joints()
        .map(|joint| joint.name.clone())
        .collect();

    let dofs = serial.dof();
    if dofs == 0 {
        return Err("selected chain has no movable joints".to_string());
    }

    Ok((serial, joint_names))
}

#[cfg(feature = "urdf_ik")]
fn apply_weights(
    solver: &mut k::JacobianIkSolver<f32>,
    reference: &[f32],
    weights: Option<&[f32]>,
) -> Result<(), String> {
    if let Some(w) = weights {
        solver.set_nullspace_function(Box::new(k::create_reference_positions_nullspace_function(
            reference.to_vec(),
            w.to_vec(),
        )));
    } else {
        solver.clear_nullspace_function();
    }
    Ok(())
}

#[cfg(feature = "urdf_ik")]
fn solve_position(
    state: &mut UrdfIkState,
    target_pos: [f32; 3],
    seed: &[f32],
    weights: Option<&[f32]>,
    max_iters: u32,
    tol_pos: f32,
) -> Result<Vec<f32>, String> {
    state
        .chain
        .set_joint_positions(seed)
        .map_err(|err| format!("failed to apply joint seed: {err}"))?;

    apply_weights(&mut state.solver, seed, weights)?;

    state.solver.num_max_try = max_iters.max(1) as usize;
    state.solver.allowable_target_distance = tol_pos;
    state.solver.allowable_target_angle = std::f32::consts::PI;

    let target_pose = k::Isometry3::from_parts(
        k::Translation3::new(target_pos[0], target_pos[1], target_pos[2]),
        k::UnitQuaternion::identity(),
    );

    let constraints = k::Constraints {
        rotation_x: false,
        rotation_y: false,
        rotation_z: false,
        ..Default::default()
    };

    state
        .solver
        .solve_with_constraints(&state.chain, &target_pose, &constraints)
        .map_err(|err| format!("IK solve failed: {err}"))?;

    Ok(state.chain.joint_positions())
}

#[allow(clippy::too_many_arguments)]
#[cfg(feature = "urdf_ik")]
fn solve_pose(
    state: &mut UrdfIkState,
    target_pos: [f32; 3],
    target_rot: [f32; 4],
    seed: &[f32],
    weights: Option<&[f32]>,
    max_iters: u32,
    tol_pos: f32,
    tol_rot: f32,
) -> Result<Vec<f32>, String> {
    state
        .chain
        .set_joint_positions(seed)
        .map_err(|err| format!("failed to apply joint seed: {err}"))?;

    apply_weights(&mut state.solver, seed, weights)?;

    state.solver.num_max_try = max_iters.max(1) as usize;
    state.solver.allowable_target_distance = tol_pos;
    state.solver.allowable_target_angle = tol_rot;

    let rotation = k::UnitQuaternion::new_normalize(k::nalgebra::Quaternion::new(
        target_rot[3],
        target_rot[0],
        target_rot[1],
        target_rot[2],
    ));
    let target_pose = k::Isometry3::from_parts(
        k::Translation3::new(target_pos[0], target_pos[1], target_pos[2]),
        rotation,
    );

    state
        .solver
        .solve(&state.chain, &target_pose)
        .map_err(|err| format!("IK solve failed: {err}"))?;

    Ok(state.chain.joint_positions())
}

#[cfg(feature = "urdf_ik")]
fn vector_from_value(value: &Value, label: &str) -> Result<Vec<f32>, String> {
    match value {
        Value::Vector(vec) => Ok(vec.clone()),
        Value::Vec2(arr) => Ok(arr.to_vec()),
        Value::Vec3(arr) => Ok(arr.to_vec()),
        Value::Vec4(arr) => Ok(arr.to_vec()),
        Value::Quat(arr) => Ok(arr.to_vec()),
        _ => Err(format!(
            "{label} expects a numeric vector, received {:?}",
            value.kind()
        )),
    }
}

#[cfg(feature = "urdf_ik")]
fn quat_from_value(value: &Value, label: &str) -> Result<[f32; 4], String> {
    match value {
        Value::Quat(arr) => Ok(*arr),
        Value::Vec4(arr) => Ok(*arr),
        Value::Vector(vec) if vec.len() == 4 => Ok([vec[0], vec[1], vec[2], vec[3]]),
        _ => Err(format!(
            "{label} expects a quaternion (x, y, z, w), received {:?}",
            value.kind()
        )),
    }
}

const MIN_MASS: f32 = 1.0e-4;

fn flatten_numeric(value: &Value) -> Option<FlatValue> {
    match value {
        Value::Float(f) => Some(FlatValue {
            layout: ValueLayout::Scalar,
            data: vec![*f],
        }),
        Value::Vec2(a) => Some(FlatValue {
            layout: ValueLayout::Vec2,
            data: a.to_vec(),
        }),
        Value::Vec3(a) => Some(FlatValue {
            layout: ValueLayout::Vec3,
            data: a.to_vec(),
        }),
        Value::Vec4(a) => Some(FlatValue {
            layout: ValueLayout::Vec4,
            data: a.to_vec(),
        }),
        Value::Quat(a) => Some(FlatValue {
            layout: ValueLayout::Quat,
            data: a.to_vec(),
        }),
        Value::ColorRgba(a) => Some(FlatValue {
            layout: ValueLayout::ColorRgba,
            data: a.to_vec(),
        }),
        Value::Transform { pos, rot, scale } => {
            let mut data = Vec::with_capacity(10);
            data.extend_from_slice(pos);
            data.extend_from_slice(rot);
            data.extend_from_slice(scale);
            Some(FlatValue {
                layout: ValueLayout::Transform,
                data,
            })
        }
        Value::Vector(vec) => Some(FlatValue {
            layout: ValueLayout::Vector(vec.len()),
            data: vec.clone(),
        }),
        Value::Record(map) => {
            let mut fields: Vec<_> = map.iter().collect();
            fields.sort_by(|a, b| a.0.cmp(b.0));
            let mut layouts = Vec::with_capacity(fields.len());
            let mut data = Vec::new();
            for (key, val) in fields {
                let flat = flatten_numeric(val)?;
                data.extend(&flat.data);
                layouts.push((key.clone(), flat.layout));
            }
            Some(FlatValue {
                layout: ValueLayout::Record(layouts),
                data,
            })
        }
        Value::Array(items) => {
            let mut layouts = Vec::with_capacity(items.len());
            let mut data = Vec::new();
            for item in items.iter() {
                let flat = flatten_numeric(item)?;
                data.extend(&flat.data);
                layouts.push(flat.layout);
            }
            Some(FlatValue {
                layout: ValueLayout::Array(layouts),
                data,
            })
        }
        Value::List(items) => {
            let mut layouts = Vec::with_capacity(items.len());
            let mut data = Vec::new();
            for item in items.iter() {
                let flat = flatten_numeric(item)?;
                data.extend(&flat.data);
                layouts.push(flat.layout);
            }
            Some(FlatValue {
                layout: ValueLayout::List(layouts),
                data,
            })
        }
        Value::Tuple(items) => {
            let mut layouts = Vec::with_capacity(items.len());
            let mut data = Vec::new();
            for item in items.iter() {
                let flat = flatten_numeric(item)?;
                data.extend(&flat.data);
                layouts.push(flat.layout);
            }
            Some(FlatValue {
                layout: ValueLayout::Tuple(layouts),
                data,
            })
        }
        _ => None,
    }
}

fn align_flattened(
    a: &FlatValue,
    b: &FlatValue,
) -> Result<(ValueLayout, Vec<f32>, Vec<f32>), ValueLayout> {
    if a.layout == b.layout {
        return Ok((a.layout.clone(), a.data.clone(), b.data.clone()));
    }
    if a.layout.is_scalar() {
        let layout = b.layout.clone();
        let len = layout.scalar_len();
        let repeated = vec![a.data.first().copied().unwrap_or(f32::NAN); len];
        return Ok((layout, repeated, b.data.clone()));
    }
    if b.layout.is_scalar() {
        let layout = a.layout.clone();
        let len = layout.scalar_len();
        let repeated = vec![b.data.first().copied().unwrap_or(f32::NAN); len];
        return Ok((layout, a.data.clone(), repeated));
    }

    let len_a = a.layout.scalar_len();
    let len_b = b.layout.scalar_len();
    if len_a >= len_b {
        Err(a.layout.clone())
    } else {
        Err(b.layout.clone())
    }
}

fn parse_variadic_key(key: &str) -> (&str, Option<usize>) {
    if let Some((prefix, tail)) = key.rsplit_once('_') {
        if let Ok(idx) = tail.parse::<usize>() {
            return (prefix, Some(idx));
        }
    }
    (key, None)
}

fn compare_variadic_keys(a: &str, b: &str) -> Ordering {
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

fn binary_numeric<F>(lhs: &Value, rhs: &Value, op: F) -> Value
where
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
        (None, None) => Value::Float(f32::NAN),
    }
}

fn unary_numeric<F>(input: &Value, op: F) -> Value
where
    F: Fn(f32) -> f32 + Copy,
{
    match flatten_numeric(input) {
        Some(flat) => {
            let data: Vec<f32> = flat.data.iter().map(|x| op(*x)).collect();
            flat.layout.reconstruct(&data)
        }
        None => Value::Float(f32::NAN),
    }
}

fn fold_numeric_variadic<F>(values: &[Value], op: F, empty_fallback: Value) -> Value
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

#[derive(Clone, Debug)]
pub struct PortValue {
    pub value: Value,
    pub shape: Shape,
}

impl PortValue {
    pub fn new(value: Value) -> Self {
        let shape = infer_shape(&value);
        PortValue { value, shape }
    }
}

impl Default for PortValue {
    fn default() -> Self {
        PortValue::new(Value::Float(0.0))
    }
}

fn infer_shape(value: &Value) -> Shape {
    Shape::new(infer_shape_id(value))
}

fn infer_shape_id(value: &Value) -> ShapeId {
    match value {
        Value::Float(_) => ShapeId::Scalar,
        Value::Bool(_) => ShapeId::Bool,
        Value::Vec2(_) => ShapeId::Vec2,
        Value::Vec3(_) => ShapeId::Vec3,
        Value::Vec4(_) => ShapeId::Vec4,
        Value::Quat(_) => ShapeId::Quat,
        Value::ColorRgba(_) => ShapeId::ColorRgba,
        Value::Transform { .. } => ShapeId::Transform,
        Value::Vector(vec) => ShapeId::Vector {
            len: if vec.is_empty() {
                None
            } else {
                Some(vec.len())
            },
        },
        Value::Text(_) => ShapeId::Text,
        Value::Enum(tag, boxed) => ShapeId::Enum(vec![(tag.clone(), infer_shape_id(boxed))]),
        Value::Record(map) => {
            let mut fields: Vec<Field> = map
                .iter()
                .map(|(name, value)| Field {
                    name: name.clone(),
                    shape: infer_shape_id(value),
                })
                .collect();
            fields.sort_by(|a, b| a.name.cmp(&b.name));
            ShapeId::Record(fields)
        }
        Value::Array(items) => {
            if let Some(first) = items.first() {
                let first_shape = infer_shape_id(first);
                let consistent = items.iter().all(|item| infer_shape_id(item) == first_shape);
                let inner = if consistent {
                    first_shape
                } else {
                    ShapeId::Scalar
                };
                ShapeId::Array(Box::new(inner), items.len())
            } else {
                ShapeId::Array(Box::new(ShapeId::Scalar), 0)
            }
        }
        Value::List(items) => {
            if let Some(first) = items.first() {
                let first_shape = infer_shape_id(first);
                let consistent = items.iter().all(|item| infer_shape_id(item) == first_shape);
                let inner = if consistent {
                    first_shape
                } else {
                    ShapeId::Scalar
                };
                ShapeId::List(Box::new(inner))
            } else {
                ShapeId::List(Box::new(ShapeId::Scalar))
            }
        }
        Value::Tuple(items) => {
            let shapes = items.iter().map(infer_shape_id).collect();
            ShapeId::Tuple(shapes)
        }
    }
}

#[derive(Debug, Default)]
pub struct GraphRuntime {
    pub t: f32,
    pub dt: f32,
    pub outputs: HashMap<NodeId, HashMap<String, PortValue>>,
    pub writes: WriteBatch,
    node_states: HashMap<NodeId, NodeRuntimeState>,
}

impl GraphRuntime {
    fn spring_state_mut<'a>(
        &'a mut self,
        node_id: &NodeId,
        flat: &FlatValue,
    ) -> &'a mut SpringState {
        match self.node_states.entry(node_id.clone()) {
            Entry::Occupied(mut occupied) => {
                {
                    let state = occupied.get_mut();
                    match state {
                        NodeRuntimeState::Spring(inner) => {
                            if inner.layout != flat.layout {
                                inner.reset(flat);
                            }
                        }
                        _ => {
                            *state = NodeRuntimeState::Spring(SpringState::new(flat));
                        }
                    }
                }
                match occupied.into_mut() {
                    NodeRuntimeState::Spring(inner) => inner,
                    _ => unreachable!(),
                }
            }
            Entry::Vacant(vacant) => {
                match vacant.insert(NodeRuntimeState::Spring(SpringState::new(flat))) {
                    NodeRuntimeState::Spring(inner) => inner,
                    _ => unreachable!(),
                }
            }
        }
    }

    fn damp_state_mut<'a>(&'a mut self, node_id: &NodeId, flat: &FlatValue) -> &'a mut DampState {
        match self.node_states.entry(node_id.clone()) {
            Entry::Occupied(mut occupied) => {
                {
                    let state = occupied.get_mut();
                    match state {
                        NodeRuntimeState::Damp(inner) => {
                            if inner.layout != flat.layout {
                                inner.reset(flat);
                            }
                        }
                        _ => {
                            *state = NodeRuntimeState::Damp(DampState::new(flat));
                        }
                    }
                }
                match occupied.into_mut() {
                    NodeRuntimeState::Damp(inner) => inner,
                    _ => unreachable!(),
                }
            }
            Entry::Vacant(vacant) => {
                match vacant.insert(NodeRuntimeState::Damp(DampState::new(flat))) {
                    NodeRuntimeState::Damp(inner) => inner,
                    _ => unreachable!(),
                }
            }
        }
    }

    fn slew_state_mut<'a>(&'a mut self, node_id: &NodeId, flat: &FlatValue) -> &'a mut SlewState {
        match self.node_states.entry(node_id.clone()) {
            Entry::Occupied(mut occupied) => {
                {
                    let state = occupied.get_mut();
                    match state {
                        NodeRuntimeState::Slew(inner) => {
                            if inner.layout != flat.layout {
                                inner.reset(flat);
                            }
                        }
                        _ => {
                            *state = NodeRuntimeState::Slew(SlewState::new(flat));
                        }
                    }
                }
                match occupied.into_mut() {
                    NodeRuntimeState::Slew(inner) => inner,
                    _ => unreachable!(),
                }
            }
            Entry::Vacant(vacant) => {
                match vacant.insert(NodeRuntimeState::Slew(SlewState::new(flat))) {
                    NodeRuntimeState::Slew(inner) => inner,
                    _ => unreachable!(),
                }
            }
        }
    }

    #[cfg(feature = "urdf_ik")]
    fn ik_state_mut<'a>(
        &'a mut self,
        node_id: &NodeId,
        key: IkKey<'_>,
    ) -> Result<&'a mut UrdfIkState, String> {
        let build_state = || -> Result<UrdfIkState, String> {
            let (chain, joint_names) =
                build_chain_from_urdf(key.urdf_xml, key.root_link, key.tip_link)?;
            Ok(UrdfIkState::new(key.hash, chain, joint_names))
        };

        match self.node_states.entry(node_id.clone()) {
            Entry::Occupied(occupied) => {
                let state = occupied.into_mut();
                match state {
                    NodeRuntimeState::UrdfIk(inner) => {
                        if inner.hash != key.hash {
                            *inner = build_state()?;
                        }
                        Ok(inner)
                    }
                    _ => {
                        *state = NodeRuntimeState::UrdfIk(build_state()?);
                        match state {
                            NodeRuntimeState::UrdfIk(inner) => Ok(inner),
                            _ => unreachable!(),
                        }
                    }
                }
            }
            Entry::Vacant(vacant) => {
                match vacant.insert(NodeRuntimeState::UrdfIk(build_state()?)) {
                    NodeRuntimeState::UrdfIk(inner) => Ok(inner),
                    _ => unreachable!(),
                }
            }
        }
    }
}

fn as_float(v: &Value) -> f32 {
    coercion::to_float(v)
}

fn as_bool(v: &Value) -> bool {
    match v {
        Value::Bool(b) => *b,
        Value::Text(s) => !s.is_empty(),
        _ => coercion::to_vector(v).iter().any(|x| *x != 0.0),
    }
}

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

macro_rules! out_map {
    ($key:expr, $val:expr) => {{
        let mut map = HashMap::new();
        map.insert($key.to_string(), PortValue::new($val));
        map
    }};
    ($val:expr) => {
        out_map!("out", $val)
    };
}

pub fn eval_node(rt: &mut GraphRuntime, spec: &NodeSpec) -> Result<(), String> {
    let ivals = read_inputs(rt, &spec.inputs);
    let t = rt.t;
    let p = &spec.params;

    let get_input = |key: &str| {
        ivals
            .get(key)
            .cloned()
            .unwrap_or_else(|| PortValue::new(Value::Float(0.0)))
    };

    let mut outputs = match spec.kind {
        NodeType::Constant => out_map!(p.value.clone().unwrap_or(Value::Float(0.0))),
        NodeType::Slider => out_map!(Value::Float(p.value.as_ref().map(as_float).unwrap_or(0.0))),
        NodeType::MultiSlider => {
            let mut map = HashMap::new();
            let x = p.x.unwrap_or(0.0);
            let y = p.y.unwrap_or(0.0);
            let z = p.z.unwrap_or(0.0);
            map.insert("x".to_string(), PortValue::new(Value::Float(x)));
            map.insert("y".to_string(), PortValue::new(Value::Float(y)));
            map.insert("z".to_string(), PortValue::new(Value::Float(z)));
            map
        }
        NodeType::Add => {
            let inputs: Vec<Value> = ivals.values().map(|pv| pv.value.clone()).collect();
            let result = fold_numeric_variadic(&inputs, |x, y| x + y, Value::Float(0.0));
            out_map!(result)
        }
        NodeType::Subtract => {
            let lhs = get_input("lhs");
            let rhs = get_input("rhs");
            out_map!(binary_numeric(&lhs.value, &rhs.value, |x, y| x - y))
        }
        NodeType::Multiply => {
            let inputs: Vec<Value> = ivals.values().map(|pv| pv.value.clone()).collect();
            let result = fold_numeric_variadic(&inputs, |x, y| x * y, Value::Float(1.0));
            out_map!(result)
        }
        NodeType::Divide => {
            let lhs = get_input("lhs");
            let rhs = get_input("rhs");
            out_map!(binary_numeric(&lhs.value, &rhs.value, |x, y| if y != 0.0 {
                x / y
            } else {
                f32::NAN
            }))
        }
        NodeType::Power => {
            let base = get_input("base");
            let exp = get_input("exp");
            out_map!(binary_numeric(&base.value, &exp.value, |x, y| x.powf(y)))
        }
        NodeType::Log => {
            let val = get_input("value");
            let base = get_input("base");
            out_map!(binary_numeric(&val.value, &base.value, |x, b| x.log(b)))
        }
        NodeType::Sin => {
            let v = get_input("in");
            out_map!(unary_numeric(&v.value, |x| x.sin()))
        }
        NodeType::Cos => {
            let v = get_input("in");
            out_map!(unary_numeric(&v.value, |x| x.cos()))
        }
        NodeType::Tan => {
            let v = get_input("in");
            out_map!(unary_numeric(&v.value, |x| x.tan()))
        }

        NodeType::Time => out_map!(Value::Float(t)),
        NodeType::Oscillator => {
            let freq_port = get_input("frequency");
            let phase_port = get_input("phase");

            let freq_value = freq_port.value;
            let phase_value = phase_port.value;

            let freq_flat = flatten_numeric(&freq_value);
            let phase_flat = flatten_numeric(&phase_value);

            let value = match (freq_flat, phase_flat) {
                (Some(freq_flat), Some(phase_flat)) => {
                    match align_flattened(&freq_flat, &phase_flat) {
                        Ok((layout, freqs, phases)) => {
                            let data: Vec<f32> = freqs
                                .into_iter()
                                .zip(phases)
                                .map(|(f, phase)| (std::f32::consts::TAU * f * t + phase).sin())
                                .collect();
                            layout.reconstruct(&data)
                        }
                        Err(layout) => layout.fill_with(f32::NAN),
                    }
                }
                (Some(freq_flat), None) => {
                    let FlatValue {
                        layout,
                        data: freqs,
                    } = freq_flat;
                    let phase_scalar = as_float(&phase_value);
                    let data: Vec<f32> = freqs
                        .into_iter()
                        .map(|f| (std::f32::consts::TAU * f * t + phase_scalar).sin())
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
                        .map(|phase| (std::f32::consts::TAU * freq_scalar * t + phase).sin())
                        .collect();
                    layout.reconstruct(&data)
                }
                (None, None) => {
                    let f = as_float(&freq_value);
                    let phase = as_float(&phase_value);
                    Value::Float((std::f32::consts::TAU * f * t + phase).sin())
                }
            };

            out_map!(value)
        }
        NodeType::Spring => {
            let input = get_input("in");
            match flatten_numeric(&input.value) {
                Some(flat) => {
                    let dt = if rt.dt.is_finite() {
                        rt.dt.max(0.0)
                    } else {
                        0.0
                    };
                    let stiffness = p.stiffness.unwrap_or(120.0);
                    let stiffness = if stiffness.is_finite() {
                        stiffness.max(0.0)
                    } else {
                        0.0
                    };
                    let damping = p.damping.unwrap_or(20.0);
                    let damping = if damping.is_finite() {
                        damping.max(0.0)
                    } else {
                        0.0
                    };
                    let mass = p.mass.unwrap_or(1.0);
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

                    out_map!(state.layout.reconstruct(&state.position))
                }
                None => out_map!(Value::Float(f32::NAN)),
            }
        }
        NodeType::Damp => {
            let input = get_input("in");
            match flatten_numeric(&input.value) {
                Some(flat) => {
                    let dt = if rt.dt.is_finite() {
                        rt.dt.max(0.0)
                    } else {
                        0.0
                    };
                    let half_life = p.half_life.unwrap_or(0.1);
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
                    out_map!(state.layout.reconstruct(&state.value))
                }
                None => out_map!(Value::Float(f32::NAN)),
            }
        }
        NodeType::Slew => {
            let input = get_input("in");
            match flatten_numeric(&input.value) {
                Some(flat) => {
                    let dt = if rt.dt.is_finite() {
                        rt.dt.max(0.0)
                    } else {
                        0.0
                    };
                    let max_rate = p.max_rate.unwrap_or(1.0);
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
                    out_map!(state.layout.reconstruct(&state.value))
                }
                None => out_map!(Value::Float(f32::NAN)),
            }
        }

        NodeType::And => out_map!(Value::Bool(
            as_bool(&get_input("lhs").value) && as_bool(&get_input("rhs").value)
        )),
        NodeType::Or => out_map!(Value::Bool(
            as_bool(&get_input("lhs").value) || as_bool(&get_input("rhs").value)
        )),
        NodeType::Not => out_map!(Value::Bool(!as_bool(&get_input("in").value))),
        NodeType::Xor => out_map!(Value::Bool(
            as_bool(&get_input("lhs").value) ^ as_bool(&get_input("rhs").value)
        )),

        NodeType::GreaterThan => out_map!(Value::Bool(
            as_float(&get_input("lhs").value) > as_float(&get_input("rhs").value)
        )),
        NodeType::LessThan => out_map!(Value::Bool(
            as_float(&get_input("lhs").value) < as_float(&get_input("rhs").value)
        )),
        NodeType::Equal => out_map!(Value::Bool(
            (as_float(&get_input("lhs").value) - as_float(&get_input("rhs").value)).abs() < 1e-6
        )),
        NodeType::NotEqual => out_map!(Value::Bool(
            (as_float(&get_input("lhs").value) - as_float(&get_input("rhs").value)).abs() > 1e-6
        )),
        NodeType::If => {
            let cond = as_bool(&get_input("cond").value);
            out_map!(if cond {
                get_input("then").value
            } else {
                get_input("else").value
            })
        }

        NodeType::Clamp => {
            let input = get_input("in");
            let min = get_input("min");
            let max = get_input("max");
            let clamped_low = binary_numeric(&input.value, &min.value, |x, m| x.max(m));
            out_map!(binary_numeric(&clamped_low, &max.value, |x, m| x.min(m)))
        }

        NodeType::Remap => {
            let value = get_input("in");
            let in_min = get_input("in_min");
            let in_max = get_input("in_max");
            let out_min = get_input("out_min");
            let out_max = get_input("out_max");

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
            out_map!(binary_numeric(
                &scaled,
                &out_min.value,
                |scaled, min| scaled + min
            ))
        }

        NodeType::Vec3Cross => {
            let a_val = get_input("a");
            let b_val = get_input("b");
            match (flatten_numeric(&a_val.value), flatten_numeric(&b_val.value)) {
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
                    out_map!(a_flat.layout.reconstruct(&result))
                }
                (Some(a_flat), _) => out_map!(a_flat.layout.fill_with(f32::NAN)),
                (_, Some(b_flat)) => out_map!(b_flat.layout.fill_with(f32::NAN)),
                _ => out_map!(Value::Vec3([f32::NAN, f32::NAN, f32::NAN])),
            }
        }

        // Generic vector utilities
        NodeType::VectorConstant => {
            if let Some(val) = &p.value {
                out_map!(val.clone())
            } else {
                out_map!(Value::Vector(Vec::new()))
            }
        }
        NodeType::VectorAdd => {
            let a = get_input("a");
            let b = get_input("b");
            out_map!(binary_numeric(&a.value, &b.value, |x, y| x + y))
        }
        NodeType::VectorSubtract => {
            let a = get_input("a");
            let b = get_input("b");
            out_map!(binary_numeric(&a.value, &b.value, |x, y| x - y))
        }
        NodeType::VectorMultiply => {
            let a = get_input("a");
            let b = get_input("b");
            out_map!(binary_numeric(&a.value, &b.value, |x, y| x * y))
        }
        NodeType::VectorScale => {
            let scalar = get_input("scalar");
            let vector = get_input("v");
            out_map!(binary_numeric(&vector.value, &scalar.value, |x, s| x * s))
        }
        NodeType::VectorNormalize => {
            let value = get_input("in");
            match flatten_numeric(&value.value) {
                Some(flat) => {
                    let len_sq: f32 = flat.data.iter().map(|x| x * x).sum();
                    let len = len_sq.sqrt();
                    let normalized: Vec<f32> = if len > 0.0 {
                        flat.data.iter().map(|x| *x / len).collect()
                    } else {
                        vec![f32::NAN; flat.data.len()]
                    };
                    out_map!(flat.layout.reconstruct(&normalized))
                }
                None => out_map!(Value::Float(f32::NAN)),
            }
        }
        NodeType::VectorDot => {
            let a = get_input("a");
            let b = get_input("b");
            match (flatten_numeric(&a.value), flatten_numeric(&b.value)) {
                (Some(fa), Some(fb)) => match align_flattened(&fa, &fb) {
                    Ok((_, da, db)) => {
                        let sum = da.iter().zip(db.iter()).map(|(x, y)| x * y).sum::<f32>();
                        out_map!(Value::Float(sum))
                    }
                    Err(_) => out_map!(Value::Float(f32::NAN)),
                },
                _ => out_map!(Value::Float(f32::NAN)),
            }
        }
        NodeType::VectorLength => {
            let value = get_input("in");
            match flatten_numeric(&value.value) {
                Some(flat) => {
                    let len_sq: f32 = flat.data.iter().map(|x| x * x).sum();
                    out_map!(Value::Float(len_sq.sqrt()))
                }
                None => out_map!(Value::Float(f32::NAN)),
            }
        }
        NodeType::VectorIndex => {
            let value = get_input("v");
            let idx = as_float(&get_input("index").value);
            if let Some(flat) = flatten_numeric(&value.value) {
                let i = idx.floor() as isize;
                let len = flat.data.len() as isize;
                if i >= 0 && i < len {
                    out_map!(Value::Float(flat.data[i as usize]))
                } else {
                    out_map!(Value::Float(f32::NAN))
                }
            } else {
                out_map!(Value::Float(f32::NAN))
            }
        }

        // Placeholder implementations to satisfy exhaustive match (schema may not expose these yet)

        // Join: flatten all inputs in handle order (operands_1, operands_2, ...)
        NodeType::Join => {
            let mut entries: Vec<_> = ivals.iter().collect();
            entries.sort_by(|(ka, _), (kb, _)| compare_variadic_keys(ka, kb));

            let mut out: Vec<f32> = Vec::new();
            for (_, v) in entries {
                if let Some(flat) = flatten_numeric(&v.value) {
                    out.extend(flat.data);
                }
            }
            out_map!(Value::Vector(out))
        }

        // Split: sizes param (floored). Emit variadic outputs: part1..partN.
        // If sum(sizes) != len(v), produce NaN vectors for each requested size.
        NodeType::Split => {
            let input = get_input("in");
            let v = flatten_numeric(&input.value)
                .map(|f| f.data)
                .unwrap_or_default();
            let sizes = p.sizes.clone().unwrap_or_default();
            let sizes_usize: Vec<usize> =
                sizes.iter().map(|x| x.floor().max(0.0) as usize).collect();

            let mut map: HashMap<String, PortValue> = HashMap::new();
            if sizes_usize.is_empty() {
                // No sizes specified: emit a single empty vector as 'part1'
                map.insert(
                    "part1".to_string(),
                    PortValue::new(Value::Vector(Vec::new())),
                );
                map
            } else {
                let total: usize = sizes_usize.iter().sum();
                if total == v.len() {
                    let mut offset = 0usize;
                    for (i, sz) in sizes_usize.iter().copied().enumerate() {
                        let slice = v[offset..offset + sz].to_vec();
                        offset += sz;
                        map.insert(
                            format!("part{}", i + 1),
                            PortValue::new(Value::Vector(slice)),
                        );
                    }
                    map
                } else {
                    // Mismatch: return NaN vectors for each requested size
                    for (i, sz) in sizes_usize.iter().copied().enumerate() {
                        map.insert(
                            format!("part{}", i + 1),
                            PortValue::new(Value::Vector(vec![f32::NAN; sz])),
                        );
                    }
                    map
                }
            }
        }

        // Reducers (vector -> scalar)
        NodeType::VectorMin => {
            let value = get_input("in");
            let out = match flatten_numeric(&value.value) {
                Some(flat) if !flat.data.is_empty() => {
                    flat.data.iter().fold(f32::INFINITY, |acc, x| acc.min(*x))
                }
                _ => f32::NAN,
            };
            out_map!(Value::Float(out))
        }
        NodeType::VectorMax => {
            let value = get_input("in");
            let out = match flatten_numeric(&value.value) {
                Some(flat) if !flat.data.is_empty() => flat
                    .data
                    .iter()
                    .fold(f32::NEG_INFINITY, |acc, x| acc.max(*x)),
                _ => f32::NAN,
            };
            out_map!(Value::Float(out))
        }
        NodeType::VectorMean => {
            let value = get_input("in");
            let out = match flatten_numeric(&value.value) {
                Some(flat) if !flat.data.is_empty() => {
                    let sum: f32 = flat.data.iter().sum();
                    sum / (flat.data.len() as f32)
                }
                _ => f32::NAN,
            };
            out_map!(Value::Float(out))
        }
        NodeType::VectorMedian => {
            let mut data = flatten_numeric(&get_input("in").value)
                .map(|f| f.data)
                .unwrap_or_default();
            let out = if data.is_empty() {
                f32::NAN
            } else {
                data.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
                let n = data.len();
                if n % 2 == 1 {
                    data[n / 2]
                } else {
                    (data[n / 2 - 1] + data[n / 2]) / 2.0
                }
            };
            out_map!(Value::Float(out))
        }
        NodeType::VectorMode => {
            let data = flatten_numeric(&get_input("in").value)
                .map(|f| f.data)
                .unwrap_or_default();
            let out = if data.is_empty() {
                f32::NAN
            } else {
                let mut map: hashbrown::HashMap<i64, (f32, usize)> = hashbrown::HashMap::new();
                for x in data {
                    if x.is_nan() {
                        continue;
                    }
                    let key = x.to_bits() as i64;
                    let entry = map.entry(key).or_insert((x, 0));
                    entry.1 += 1;
                }
                if map.is_empty() {
                    f32::NAN
                } else {
                    let mut best_val = f32::NAN;
                    let mut best_count = 0usize;
                    for (_k, (val, cnt)) in map.iter() {
                        if *cnt > best_count {
                            best_count = *cnt;
                            best_val = *val;
                        } else if *cnt == best_count && val < &best_val {
                            best_val = *val;
                        }
                    }
                    best_val
                }
            };
            out_map!(Value::Float(out))
        }

        NodeType::InverseKinematics => {
            let l1 = as_float(&get_input("bone1").value);
            let l2 = as_float(&get_input("bone2").value);
            let l3 = as_float(&get_input("bone3").value);
            let theta = as_float(&get_input("theta").value);
            let x = as_float(&get_input("x").value);
            let y = as_float(&get_input("y").value);

            let wx = x - l3 * theta.cos();
            let wy = y - l3 * theta.sin();
            let dist_sq = wx * wx + wy * wy;

            out_map!(
                if dist_sq > (l1 + l2) * (l1 + l2) || dist_sq < (l1 - l2) * (l1 - l2) {
                    Value::Vec3([f32::NAN, f32::NAN, f32::NAN])
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

        #[cfg(feature = "urdf_ik")]
        NodeType::UrdfIkPosition => {
            let target_pos = match get_input("target_pos").value {
                Value::Vec3(arr) => arr,
                other => {
                    return Err(format!(
                        "UrdfIkPosition input 'target_pos' expects Vec3, received {:?}",
                        other.kind()
                    ));
                }
            };

            let urdf_xml = p
                .urdf_xml
                .as_deref()
                .filter(|s| !s.trim().is_empty())
                .ok_or_else(|| "UrdfIkPosition requires non-empty 'urdf_xml' param".to_string())?;
            let root_link = p.root_link.as_deref().unwrap_or("base_link");
            let tip_link = p.tip_link.as_deref().unwrap_or("tool0");

            let key = IkKey {
                hash: hash_urdf_config(urdf_xml, root_link, tip_link),
                urdf_xml,
                root_link,
                tip_link,
            };
            let state = rt.ik_state_mut(&spec.id, key)?;
            let dofs = state.dofs;

            let seed_candidate: Option<Vec<f32>> = if let Some(port) = ivals.get("seed") {
                Some(vector_from_value(&port.value, "UrdfIkPosition seed")?)
            } else {
                p.seed.clone()
            };

            let seed_provided = seed_candidate.is_some();
            let mut seed = match seed_candidate {
                Some(vec) => vec,
                None => state.chain.joint_positions(),
            };

            if seed.len() != dofs {
                if seed_provided {
                    return Err(format!(
                        "UrdfIkPosition seed length {} does not match chain DoF {dofs}",
                        seed.len()
                    ));
                }
                seed = vec![0.0; dofs];
            }

            let weights_ref = p.weights.as_ref().filter(|w| !w.is_empty());
            if let Some(w) = weights_ref {
                if w.len() != dofs {
                    return Err(format!(
                        "UrdfIkPosition weights length {} does not match chain DoF {dofs}",
                        w.len()
                    ));
                }
            }
            let weights = weights_ref.map(|w| w.as_slice());

            let solution = solve_position(
                state,
                target_pos,
                seed.as_slice(),
                weights,
                p.max_iters.unwrap_or(100),
                p.tol_pos.unwrap_or(1e-3),
            )?;

            out_map!(state.solution_record(&solution))
        }

        #[cfg(not(feature = "urdf_ik"))]
        NodeType::UrdfIkPosition => {
            return Err("UrdfIkPosition node requires the 'urdf_ik' feature".to_string());
        }

        #[cfg(feature = "urdf_ik")]
        NodeType::UrdfIkPose => {
            let target_pos = match get_input("target_pos").value {
                Value::Vec3(arr) => arr,
                other => {
                    return Err(format!(
                        "UrdfIkPose input 'target_pos' expects Vec3, received {:?}",
                        other.kind()
                    ));
                }
            };
            let target_rot = {
                let rot_port = get_input("target_rot");
                quat_from_value(&rot_port.value, "UrdfIkPose target_rot")?
            };

            let urdf_xml = p
                .urdf_xml
                .as_deref()
                .filter(|s| !s.trim().is_empty())
                .ok_or_else(|| "UrdfIkPose requires non-empty 'urdf_xml' param".to_string())?;
            let root_link = p.root_link.as_deref().unwrap_or("base_link");
            let tip_link = p.tip_link.as_deref().unwrap_or("tool0");

            let key = IkKey {
                hash: hash_urdf_config(urdf_xml, root_link, tip_link),
                urdf_xml,
                root_link,
                tip_link,
            };
            let state = rt.ik_state_mut(&spec.id, key)?;
            let dofs = state.dofs;

            let seed_candidate: Option<Vec<f32>> = if let Some(port) = ivals.get("seed") {
                Some(vector_from_value(&port.value, "UrdfIkPose seed")?)
            } else {
                p.seed.clone()
            };

            let seed_provided = seed_candidate.is_some();
            let mut seed = match seed_candidate {
                Some(vec) => vec,
                None => state.chain.joint_positions(),
            };

            if seed.len() != dofs {
                if seed_provided {
                    return Err(format!(
                        "UrdfIkPose seed length {} does not match chain DoF {dofs}",
                        seed.len()
                    ));
                }
                seed = vec![0.0; dofs];
            }

            let weights_ref = p.weights.as_ref().filter(|w| !w.is_empty());
            if let Some(w) = weights_ref {
                if w.len() != dofs {
                    return Err(format!(
                        "UrdfIkPose weights length {} does not match chain DoF {dofs}",
                        w.len()
                    ));
                }
            }
            let weights = weights_ref.map(|w| w.as_slice());

            let solution = solve_pose(
                state,
                target_pos,
                target_rot,
                seed.as_slice(),
                weights,
                p.max_iters.unwrap_or(100),
                p.tol_pos.unwrap_or(1e-3),
                p.tol_rot.unwrap_or(1e-3),
            )?;

            out_map!(state.solution_record(&solution))
        }

        #[cfg(not(feature = "urdf_ik"))]
        NodeType::UrdfIkPose => {
            return Err("UrdfIkPose node requires the 'urdf_ik' feature".to_string());
        }

        NodeType::Output => out_map!(get_input("in").value),
    };

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

fn enforce_output_shapes(
    spec: &NodeSpec,
    outputs: &mut HashMap<String, PortValue>,
) -> Result<(), String> {
    if spec.output_shapes.is_empty() {
        return Ok(());
    }

    for (key, declared) in spec.output_shapes.iter() {
        let port = outputs.get_mut(key).ok_or_else(|| {
            format!(
                "node '{}' missing declared output '{}' during evaluation",
                spec.id, key
            )
        })?;

        if !value_matches_shape(&declared.id, &port.value) {
            return Err(format!(
                "node '{}' output '{}' does not match declared shape {:?}",
                spec.id, key, declared.id
            ));
        }

        port.shape = declared.clone();
    }

    Ok(())
}

fn value_matches_shape(shape: &ShapeId, value: &Value) -> bool {
    match shape {
        ShapeId::Scalar => matches!(value, Value::Float(_)),
        ShapeId::Bool => matches!(value, Value::Bool(_)),
        ShapeId::Vec2 => matches!(value, Value::Vec2(_)),
        ShapeId::Vec3 => matches!(value, Value::Vec3(_)),
        ShapeId::Vec4 => matches!(value, Value::Vec4(_)),
        ShapeId::Quat => matches!(value, Value::Quat(_)),
        ShapeId::ColorRgba => matches!(value, Value::ColorRgba(_)),
        ShapeId::Transform => matches!(value, Value::Transform { .. }),
        ShapeId::Text => matches!(value, Value::Text(_)),
        ShapeId::Vector { len } => match value {
            Value::Vector(items) => match len {
                Some(expected) => items.len() == *expected,
                None => true,
            },
            _ => false,
        },
        ShapeId::Record(fields) => match value {
            Value::Record(map) => fields.iter().all(|field| {
                map.get(&field.name)
                    .map(|v| value_matches_shape(&field.shape, v))
                    .unwrap_or(false)
            }),
            _ => false,
        },
        ShapeId::Array(inner, len) => match value {
            Value::Array(items) => {
                items.len() == *len && items.iter().all(|item| value_matches_shape(inner, item))
            }
            _ => false,
        },
        ShapeId::List(inner) => match value {
            Value::List(items) => items.iter().all(|item| value_matches_shape(inner, item)),
            _ => false,
        },
        ShapeId::Tuple(entries) => match value {
            Value::Tuple(items) => {
                items.len() == entries.len()
                    && items
                        .iter()
                        .zip(entries.iter())
                        .all(|(item, shape)| value_matches_shape(shape, item))
            }
            _ => false,
        },
        ShapeId::Enum(variants) => match value {
            Value::Enum(tag, boxed) => variants
                .iter()
                .find(|(variant, _)| variant == tag)
                .is_some_and(|(_, shape)| value_matches_shape(shape, boxed)),
            _ => false,
        },
    }
}
pub fn evaluate_all(rt: &mut GraphRuntime, spec: &GraphSpec) -> Result<(), String> {
    rt.outputs.clear();
    rt.writes = WriteBatch::new();
    rt.node_states
        .retain(|id, _| spec.nodes.iter().any(|node| node.id == *id));

    let order = crate::topo::topo_order(&spec.nodes)?;
    for id in order {
        if let Some(node) = spec.nodes.iter().find(|n| n.id == id) {
            eval_node(rt, node)?;
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::{GraphSpec, InputConnection, NodeParams, NodeType};
    use hashbrown::HashMap;

    fn constant_node(id: &str, value: Value) -> NodeSpec {
        NodeSpec {
            id: id.to_string(),
            kind: NodeType::Constant,
            params: NodeParams {
                value: Some(value),
                ..Default::default()
            },
            inputs: HashMap::new(),
            output_shapes: HashMap::new(),
        }
    }

    #[test]
    fn it_should_respect_declared_shape() {
        let mut node = constant_node("a", Value::Float(1.0));
        node.output_shapes
            .insert("out".to_string(), Shape::new(ShapeId::Scalar));

        let spec = GraphSpec { nodes: vec![node] };
        let mut rt = GraphRuntime::default();
        evaluate_all(&mut rt, &spec).expect("shape should match");
        let outputs = rt.outputs.get("a").expect("outputs present");
        let port = outputs.get("out").expect("out port present");
        assert!(matches!(port.shape.id, ShapeId::Scalar));
    }

    #[test]
    fn it_should_error_when_shape_mismatches() {
        let mut node = constant_node("a", Value::Float(1.0));
        node.output_shapes
            .insert("out".to_string(), Shape::new(ShapeId::Vec3));

        let spec = GraphSpec { nodes: vec![node] };
        let mut rt = GraphRuntime::default();
        let err = evaluate_all(&mut rt, &spec).expect_err("should fail due to mismatch");
        assert!(err.contains("does not match declared shape"));
    }

    #[test]
    fn it_should_emit_write_for_output_nodes() {
        let mut output_inputs = HashMap::new();
        output_inputs.insert(
            "in".to_string(),
            InputConnection {
                node_id: "src".to_string(),
                output_key: "out".to_string(),
            },
        );

        let graph = GraphSpec {
            nodes: vec![
                constant_node("src", Value::Float(2.0)),
                NodeSpec {
                    id: "out".to_string(),
                    kind: NodeType::Output,
                    params: NodeParams {
                        path: Some(
                            vizij_api_core::TypedPath::parse("robot1/Arm/Joint.angle")
                                .expect("valid path"),
                        ),
                        ..Default::default()
                    },
                    inputs: output_inputs,
                    output_shapes: HashMap::new(),
                },
            ],
        };

        let mut rt = GraphRuntime::default();
        evaluate_all(&mut rt, &graph).expect("graph should evaluate");
        assert_eq!(rt.writes.iter().count(), 1);
        let op = rt.writes.iter().next().expect("write present");
        assert_eq!(op.path.to_string(), "robot1/Arm/Joint.angle");
        match op.value {
            Value::Float(f) => assert_eq!(f, 2.0),
            _ => panic!("expected float write"),
        }
    }

    #[test]
    fn join_respects_operand_order() {
        let mut inputs = HashMap::new();
        inputs.insert(
            "operands_1".to_string(),
            InputConnection {
                node_id: "a".to_string(),
                output_key: "out".to_string(),
            },
        );
        inputs.insert(
            "operands_2".to_string(),
            InputConnection {
                node_id: "b".to_string(),
                output_key: "out".to_string(),
            },
        );
        inputs.insert(
            "operands_3".to_string(),
            InputConnection {
                node_id: "c".to_string(),
                output_key: "out".to_string(),
            },
        );

        let graph = GraphSpec {
            nodes: vec![
                constant_node("a", Value::Vector(vec![1.0, 2.0])),
                constant_node("b", Value::Vector(vec![3.0])),
                constant_node("c", Value::Vector(vec![4.0, 5.0])),
                NodeSpec {
                    id: "join".to_string(),
                    kind: NodeType::Join,
                    params: NodeParams::default(),
                    inputs,
                    output_shapes: HashMap::new(),
                },
            ],
        };

        let mut rt = GraphRuntime::default();
        evaluate_all(&mut rt, &graph).expect("join should evaluate");
        let outputs = rt.outputs.get("join").expect("join outputs present");
        let port = outputs.get("out").expect("out port present");
        match &port.value {
            Value::Vector(vec) => assert_eq!(vec, &vec![1.0, 2.0, 3.0, 4.0, 5.0]),
            other => panic!("expected vector output, got {:?}", other),
        }
    }

    #[test]
    fn oscillator_broadcasts_vector_inputs() {
        let mut inputs = HashMap::new();
        inputs.insert(
            "frequency".to_string(),
            InputConnection {
                node_id: "freq".to_string(),
                output_key: "out".to_string(),
            },
        );
        inputs.insert(
            "phase".to_string(),
            InputConnection {
                node_id: "phase".to_string(),
                output_key: "out".to_string(),
            },
        );

        let graph = GraphSpec {
            nodes: vec![
                constant_node("freq", Value::Vector(vec![1.0, 2.0, 3.0])),
                constant_node("phase", Value::Float(0.0)),
                NodeSpec {
                    id: "osc".to_string(),
                    kind: NodeType::Oscillator,
                    params: NodeParams::default(),
                    inputs,
                    output_shapes: HashMap::new(),
                },
            ],
        };

        let mut rt = GraphRuntime {
            t: 0.5,
            ..Default::default()
        };
        evaluate_all(&mut rt, &graph).expect("oscillator should evaluate");

        let outputs = rt.outputs.get("osc").expect("osc outputs present");
        let port = outputs.get("out").expect("osc out port present");
        let expected: Vec<f32> = vec![1.0, 2.0, 3.0]
            .into_iter()
            .map(|f| (std::f32::consts::TAU * f * rt.t).sin())
            .collect();

        match &port.value {
            Value::Vector(vec) => {
                assert_eq!(vec.len(), expected.len());
                for (actual, expected) in vec.iter().zip(expected.iter()) {
                    assert!(
                        (actual - expected).abs() < 1e-6,
                        "expected {expected}, got {actual}"
                    );
                }
            }
            other => panic!("expected vector output, got {:?}", other),
        }
    }

    #[test]
    fn it_should_infer_vector_length_hints() {
        let node = constant_node("vec", Value::Vector(vec![1.0, 2.0, 3.0]));
        let spec = GraphSpec { nodes: vec![node] };
        let mut rt = GraphRuntime::default();
        evaluate_all(&mut rt, &spec).expect("graph should evaluate");
        let outputs = rt.outputs.get("vec").expect("outputs present");
        let port = outputs.get("out").expect("out port");
        match &port.shape.id {
            ShapeId::Vector { len } => assert_eq!(*len, Some(3)),
            other => panic!("expected vector shape, got {:?}", other),
        }
    }

    #[test]
    fn it_should_error_when_declared_output_missing() {
        let mut node = constant_node("a", Value::Float(1.0));
        node.output_shapes
            .insert("secondary".to_string(), Shape::new(ShapeId::Scalar));

        let spec = GraphSpec { nodes: vec![node] };
        let mut rt = GraphRuntime::default();
        let err = evaluate_all(&mut rt, &spec).expect_err("missing declared output should error");
        assert!(err.contains("missing declared output"));
    }

    #[test]
    fn it_should_validate_vector_length_against_declared_shape() {
        let mut node = constant_node("a", Value::Vector(vec![1.0, 2.0, 3.0]));
        node.output_shapes.insert(
            "out".to_string(),
            Shape::new(ShapeId::Vector { len: Some(4) }),
        );

        let spec = GraphSpec { nodes: vec![node] };
        let mut rt = GraphRuntime::default();
        let err = evaluate_all(&mut rt, &spec).expect_err("vector length mismatch should error");
        assert!(err.contains("does not match declared shape"));
    }

    #[test]
    fn it_should_reject_invalid_paths_during_deserialization() {
        let json = r#"{
            "id": "node",
            "type": "output",
            "params": { "path": "robot/invalid/" },
            "inputs": {},
            "output_shapes": {}
        }"#;

        let err = serde_json::from_str::<NodeSpec>(json)
            .expect_err("invalid typed path should fail to parse");
        assert!(err.to_string().contains("path"));
    }

    #[test]
    fn spring_node_transitions_toward_new_target() {
        let mut spring_inputs = HashMap::new();
        spring_inputs.insert(
            "in".to_string(),
            InputConnection {
                node_id: "target".to_string(),
                output_key: "out".to_string(),
            },
        );

        let spring = NodeSpec {
            id: "spring".to_string(),
            kind: NodeType::Spring,
            params: NodeParams {
                stiffness: Some(30.0),
                damping: Some(6.0),
                mass: Some(1.0),
                ..Default::default()
            },
            inputs: spring_inputs,
            output_shapes: HashMap::new(),
        };

        let mut spec = GraphSpec {
            nodes: vec![constant_node("target", Value::Float(0.0)), spring],
        };

        let mut rt = GraphRuntime::default();
        evaluate_all(&mut rt, &spec).expect("initial evaluate");

        spec.nodes[0].params.value = Some(Value::Float(10.0));

        rt.dt = 1.0 / 60.0;
        rt.t += rt.dt;
        evaluate_all(&mut rt, &spec).expect("first step");
        let first = match rt
            .outputs
            .get("spring")
            .and_then(|map| map.get("out"))
            .map(|pv| pv.value.clone())
            .expect("spring output")
        {
            Value::Float(f) => f,
            other => panic!("expected float, got {:?}", other),
        };
        assert!(
            (first - 10.0).abs() > 0.01,
            "spring should not immediately reach target"
        );

        for _ in 0..240 {
            rt.dt = 1.0 / 60.0;
            rt.t += rt.dt;
            evaluate_all(&mut rt, &spec).expect("subsequent step");
        }

        let final_val = match rt
            .outputs
            .get("spring")
            .and_then(|map| map.get("out"))
            .map(|pv| pv.value.clone())
            .expect("spring output")
        {
            Value::Float(f) => f,
            other => panic!("expected float, got {:?}", other),
        };
        assert!(
            (final_val - 10.0).abs() < 0.1,
            "spring should converge to target"
        );
    }

    #[test]
    fn damp_node_smooths_toward_target() {
        let mut damp_inputs = HashMap::new();
        damp_inputs.insert(
            "in".to_string(),
            InputConnection {
                node_id: "target".to_string(),
                output_key: "out".to_string(),
            },
        );

        let damp = NodeSpec {
            id: "damp".to_string(),
            kind: NodeType::Damp,
            params: NodeParams {
                half_life: Some(0.2),
                ..Default::default()
            },
            inputs: damp_inputs,
            output_shapes: HashMap::new(),
        };

        let mut spec = GraphSpec {
            nodes: vec![constant_node("target", Value::Float(0.0)), damp],
        };

        let mut rt = GraphRuntime::default();
        evaluate_all(&mut rt, &spec).expect("initial evaluate");

        spec.nodes[0].params.value = Some(Value::Float(1.0));
        rt.dt = 0.1;
        rt.t += rt.dt;
        evaluate_all(&mut rt, &spec).expect("first step");

        let first = match rt
            .outputs
            .get("damp")
            .and_then(|map| map.get("out"))
            .map(|pv| pv.value.clone())
            .expect("damp output")
        {
            Value::Float(f) => f,
            other => panic!("expected float, got {:?}", other),
        };
        assert!(first > 0.0 && first < 1.0, "damp should move but not snap");

        for _ in 0..20 {
            rt.dt = 0.1;
            rt.t += rt.dt;
            evaluate_all(&mut rt, &spec).expect("subsequent step");
        }

        let final_val = match rt
            .outputs
            .get("damp")
            .and_then(|map| map.get("out"))
            .map(|pv| pv.value.clone())
            .expect("damp output")
        {
            Value::Float(f) => f,
            other => panic!("expected float, got {:?}", other),
        };
        assert!(
            (final_val - 1.0).abs() < 0.05,
            "damp should approach target"
        );
    }

    #[test]
    fn slew_node_limits_rate_of_change() {
        let mut slew_inputs = HashMap::new();
        slew_inputs.insert(
            "in".to_string(),
            InputConnection {
                node_id: "target".to_string(),
                output_key: "out".to_string(),
            },
        );

        let slew = NodeSpec {
            id: "slew".to_string(),
            kind: NodeType::Slew,
            params: NodeParams {
                max_rate: Some(2.0),
                ..Default::default()
            },
            inputs: slew_inputs,
            output_shapes: HashMap::new(),
        };

        let mut spec = GraphSpec {
            nodes: vec![constant_node("target", Value::Float(0.0)), slew],
        };

        let mut rt = GraphRuntime::default();
        evaluate_all(&mut rt, &spec).expect("initial evaluate");

        spec.nodes[0].params.value = Some(Value::Float(5.0));
        rt.dt = 0.25;
        rt.t += rt.dt;
        evaluate_all(&mut rt, &spec).expect("slew step");

        let first = match rt
            .outputs
            .get("slew")
            .and_then(|map| map.get("out"))
            .map(|pv| pv.value.clone())
            .expect("slew output")
        {
            Value::Float(f) => f,
            other => panic!("expected float, got {:?}", other),
        };

        assert!(
            (first - 0.5).abs() < 1e-6,
            "slew should move at configured rate"
        );

        for _ in 0..10 {
            rt.dt = 0.25;
            rt.t += rt.dt;
            evaluate_all(&mut rt, &spec).expect("subsequent step");
        }

        let final_val = match rt
            .outputs
            .get("slew")
            .and_then(|map| map.get("out"))
            .map(|pv| pv.value.clone())
            .expect("slew output")
        {
            Value::Float(f) => f,
            other => panic!("expected float, got {:?}", other),
        };
        assert!(
            (final_val - 5.0).abs() < 0.25,
            "slew should eventually reach target"
        );
    }

    #[cfg(feature = "urdf_ik")]
    mod urdf_ik {
        use super::*;

        const PLANAR_URDF: &str = r#"
<robot name="planar_arm">
  <link name="base_link" />
  <link name="link1" />
  <link name="link2" />
  <link name="tool" />

  <joint name="joint1" type="revolute">
    <parent link="base_link" />
    <child link="link1" />
    <origin xyz="0 0 0" rpy="0 0 0" />
    <axis xyz="0 0 1" />
    <limit lower="-3.1416" upper="3.1416" effort="1" velocity="1" />
  </joint>

  <joint name="joint2" type="revolute">
    <parent link="link1" />
    <child link="link2" />
    <origin xyz="0.5 0 0" rpy="0 0 0" />
    <axis xyz="0 0 1" />
    <limit lower="-3.1416" upper="3.1416" effort="1" velocity="1" />
  </joint>

  <joint name="joint3" type="revolute">
    <parent link="link2" />
    <child link="tool" />
    <origin xyz="0.5 0 0" rpy="0 0 0" />
    <axis xyz="0 0 1" />
    <limit lower="-3.1416" upper="3.1416" effort="1" velocity="1" />
  </joint>
</robot>
"#;

        const POSE_URDF: &str = r#"
<robot name="pose_arm">
  <link name="base_link" />
  <link name="link1" />
  <link name="link2" />
  <link name="link3" />
  <link name="link4" />
  <link name="link5" />
  <link name="link6" />
  <link name="tool" />

  <joint name="joint1" type="revolute">
    <parent link="base_link" />
    <child link="link1" />
    <origin xyz="0 0 0.1" rpy="0 0 0" />
    <axis xyz="0 0 1" />
    <limit lower="-3.1416" upper="3.1416" effort="1" velocity="1" />
  </joint>

  <joint name="joint2" type="revolute">
    <parent link="link1" />
    <child link="link2" />
    <origin xyz="0.2 0 0" rpy="0 0 0" />
    <axis xyz="0 1 0" />
    <limit lower="-3.1416" upper="3.1416" effort="1" velocity="1" />
  </joint>

  <joint name="joint3" type="revolute">
    <parent link="link2" />
    <child link="link3" />
    <origin xyz="0.2 0 0" rpy="0 0 0" />
    <axis xyz="1 0 0" />
    <limit lower="-3.1416" upper="3.1416" effort="1" velocity="1" />
  </joint>

  <joint name="joint4" type="revolute">
    <parent link="link3" />
    <child link="link4" />
    <origin xyz="0.2 0 0" rpy="0 0 0" />
    <axis xyz="0 0 1" />
    <limit lower="-3.1416" upper="3.1416" effort="1" velocity="1" />
  </joint>

  <joint name="joint5" type="revolute">
    <parent link="link4" />
    <child link="link5" />
    <origin xyz="0.15 0 0" rpy="0 0 0" />
    <axis xyz="0 1 0" />
    <limit lower="-3.1416" upper="3.1416" effort="1" velocity="1" />
  </joint>

  <joint name="joint6" type="revolute">
    <parent link="link5" />
    <child link="link6" />
    <origin xyz="0.1 0 0" rpy="0 0 0" />
    <axis xyz="1 0 0" />
    <limit lower="-3.1416" upper="3.1416" effort="1" velocity="1" />
  </joint>

  <joint name="tool_joint" type="fixed">
    <parent link="link6" />
    <child link="tool" />
    <origin xyz="0.1 0 0" rpy="0 0 0" />
  </joint>
</robot>
"#;

        fn params_for(urdf: &str) -> NodeParams {
            NodeParams {
                urdf_xml: Some(urdf.to_string()),
                root_link: Some("base_link".to_string()),
                tip_link: Some("tool".to_string()),
                max_iters: Some(200),
                tol_pos: Some(1e-3),
                tol_rot: Some(1e-3),
                ..Default::default()
            }
        }

        fn run_graph(nodes: Vec<NodeSpec>) -> Result<GraphRuntime, String> {
            let spec = GraphSpec { nodes };
            let mut rt = GraphRuntime::default();
            evaluate_all(&mut rt, &spec)?;
            Ok(rt)
        }

        fn extract_angles(record: &HashMap<String, Value>, names: &[&str]) -> Vec<f32> {
            names
                .iter()
                .map(|name| match record.get(*name) {
                    Some(Value::Float(angle)) => *angle,
                    other => panic!("expected float angle for {name}, got {:?}", other),
                })
                .collect()
        }

        #[test]
        fn urdf_ik_position_reaches_target() {
            let target_pos_id = "target_pos";
            let ik_id = "ik";

            let seed = vec![0.1, -0.2, 0.3, 0.2, -0.1, 0.25];
            let (chain, _) = super::super::build_chain_from_urdf(POSE_URDF, "base_link", "tool")
                .expect("valid chain");
            chain.set_joint_positions(&seed).expect("apply seed for fk");
            let target_pose = chain.end_transform();
            let target_pos_vec = target_pose.translation.vector;

            let mut nodes = Vec::new();
            nodes.push(super::constant_node(
                target_pos_id,
                Value::Vec3([target_pos_vec.x, target_pos_vec.y, target_pos_vec.z]),
            ));

            let mut inputs = HashMap::new();
            inputs.insert(
                "target_pos".to_string(),
                InputConnection {
                    node_id: target_pos_id.to_string(),
                    output_key: "out".to_string(),
                },
            );

            let mut params = params_for(POSE_URDF);
            params.seed = Some(seed.clone());

            nodes.push(NodeSpec {
                id: ik_id.to_string(),
                kind: NodeType::UrdfIkPosition,
                params,
                inputs,
                output_shapes: HashMap::new(),
            });

            let rt = run_graph(nodes).expect("IK position solve should succeed");
            let record = match rt
                .outputs
                .get(ik_id)
                .and_then(|map| map.get("out"))
                .map(|pv| pv.value.clone())
                .expect("ik output")
            {
                Value::Record(map) => map,
                other => panic!("expected record output, got {:?}", other),
            };

            assert_eq!(record.len(), 6, "expected six joint outputs");
            let joint_names = ["joint1", "joint2", "joint3", "joint4", "joint5", "joint6"];
            let angles = extract_angles(&record, &joint_names);

            for (expected, actual) in seed.iter().zip(angles.iter()) {
                assert!((expected - actual).abs() < 1e-3);
            }

            let (chain_verify, _) =
                super::super::build_chain_from_urdf(POSE_URDF, "base_link", "tool")
                    .expect("valid chain");
            chain_verify
                .set_joint_positions(&angles)
                .expect("seed application");
            let end = chain_verify.end_transform();
            let pos = end.translation.vector;
            assert!((pos.x - target_pos_vec.x).abs() < 5e-3);
            assert!((pos.y - target_pos_vec.y).abs() < 5e-3);
            assert!((pos.z - target_pos_vec.z).abs() < 5e-3);
        }

        #[test]
        fn urdf_ik_pose_matches_target_orientation() {
            let target_pos_id = "target_pos";
            let target_rot_id = "target_rot";
            let ik_id = "ik_pose";

            let seed = vec![0.3, -0.45, 0.35, 0.15, -0.2, 0.18];
            let (chain, _) = super::super::build_chain_from_urdf(POSE_URDF, "base_link", "tool")
                .expect("valid chain");
            chain.set_joint_positions(&seed).expect("seed fk");
            let base_pose = chain.end_transform();
            let base_pos = base_pose.translation.vector;

            let desired_rot = base_pose.rotation;

            let mut nodes = Vec::new();
            nodes.push(super::constant_node(
                target_pos_id,
                Value::Vec3([base_pos.x, base_pos.y, base_pos.z]),
            ));
            nodes.push(super::constant_node(
                target_rot_id,
                Value::Quat([desired_rot.i, desired_rot.j, desired_rot.k, desired_rot.w]),
            ));

            let mut inputs = HashMap::new();
            inputs.insert(
                "target_pos".to_string(),
                InputConnection {
                    node_id: target_pos_id.to_string(),
                    output_key: "out".to_string(),
                },
            );
            inputs.insert(
                "target_rot".to_string(),
                InputConnection {
                    node_id: target_rot_id.to_string(),
                    output_key: "out".to_string(),
                },
            );

            let mut params = params_for(POSE_URDF);
            params.max_iters = Some(600);
            params.seed = Some(seed.clone());

            nodes.push(NodeSpec {
                id: ik_id.to_string(),
                kind: NodeType::UrdfIkPose,
                params,
                inputs,
                output_shapes: HashMap::new(),
            });

            let rt = run_graph(nodes).expect("IK pose solve should succeed");
            let record = match rt
                .outputs
                .get(ik_id)
                .and_then(|map| map.get("out"))
                .map(|pv| pv.value.clone())
                .expect("ik output")
            {
                Value::Record(map) => map,
                other => panic!("expected record output, got {:?}", other),
            };

            assert_eq!(record.len(), 6, "expected six joint outputs");
            let joint_names = ["joint1", "joint2", "joint3", "joint4", "joint5", "joint6"];
            let angles = extract_angles(&record, &joint_names);

            for (expected, actual) in seed.iter().zip(angles.iter()) {
                assert!((expected - actual).abs() < 1e-3);
            }

            let (chain_verify, _) =
                super::super::build_chain_from_urdf(POSE_URDF, "base_link", "tool")
                    .expect("valid chain");
            chain_verify
                .set_joint_positions(&angles)
                .expect("apply joints");
            let end = chain_verify.end_transform();
            let pos = end.translation.vector;
            assert!((pos.x - base_pos.x).abs() < 5e-3);
            assert!((pos.y - base_pos.y).abs() < 5e-3);
            assert!((pos.z - base_pos.z).abs() < 5e-3);

            let angle_err = desired_rot.angle_to(&end.rotation);
            assert!(angle_err.abs() < 5e-3);
        }

        #[test]
        fn malformed_urdf_returns_error() {
            let mut params = params_for(PLANAR_URDF);
            params.urdf_xml = Some("<robot".to_string());

            let mut nodes = Vec::new();
            nodes.push(super::constant_node(
                "target_pos",
                Value::Vec3([0.5, 0.0, 0.0]),
            ));

            let mut inputs = HashMap::new();
            inputs.insert(
                "target_pos".to_string(),
                InputConnection {
                    node_id: "target_pos".to_string(),
                    output_key: "out".to_string(),
                },
            );

            nodes.push(NodeSpec {
                id: "ik".to_string(),
                kind: NodeType::UrdfIkPosition,
                params,
                inputs,
                output_shapes: HashMap::new(),
            });

            let spec = GraphSpec { nodes };
            let mut rt = GraphRuntime::default();
            let err = evaluate_all(&mut rt, &spec).expect_err("malformed URDF should error");
            assert!(err.contains("parse URDF"));
        }

        #[test]
        fn seed_length_mismatch_errors() {
            let mut params = params_for(PLANAR_URDF);
            params.seed = Some(vec![0.0]);

            let mut nodes = Vec::new();
            nodes.push(super::constant_node(
                "target_pos",
                Value::Vec3([0.5, 0.0, 0.0]),
            ));

            let mut inputs = HashMap::new();
            inputs.insert(
                "target_pos".to_string(),
                InputConnection {
                    node_id: "target_pos".to_string(),
                    output_key: "out".to_string(),
                },
            );

            nodes.push(NodeSpec {
                id: "ik".to_string(),
                kind: NodeType::UrdfIkPosition,
                params,
                inputs,
                output_shapes: HashMap::new(),
            });

            let spec = GraphSpec { nodes };
            let mut rt = GraphRuntime::default();
            let err = evaluate_all(&mut rt, &spec).expect_err("seed mismatch should error");
            assert!(err.contains("seed length"));
        }

        #[test]
        fn weights_length_mismatch_errors() {
            let mut params = params_for(PLANAR_URDF);
            params.weights = Some(vec![1.0]);

            let mut nodes = Vec::new();
            nodes.push(super::constant_node(
                "target_pos",
                Value::Vec3([0.5, 0.0, 0.0]),
            ));

            let mut inputs = HashMap::new();
            inputs.insert(
                "target_pos".to_string(),
                InputConnection {
                    node_id: "target_pos".to_string(),
                    output_key: "out".to_string(),
                },
            );

            nodes.push(NodeSpec {
                id: "ik".to_string(),
                kind: NodeType::UrdfIkPosition,
                params,
                inputs,
                output_shapes: HashMap::new(),
            });

            let spec = GraphSpec { nodes };
            let mut rt = GraphRuntime::default();
            let err = evaluate_all(&mut rt, &spec).expect_err("weights mismatch should error");
            assert!(err.contains("weights length"));
        }
    }
}
