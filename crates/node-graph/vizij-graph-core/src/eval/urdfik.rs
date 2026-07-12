//! URDF inverse kinematics helpers gated behind the `urdf_ik` feature.

use crate::graph_value::GraphValue;
use hashbrown::HashMap;
#[cfg(feature = "urdf_ik")]
use k::InverseKinematicsSolver;
#[cfg(feature = "urdf_ik")]
use std::collections::hash_map::DefaultHasher;
#[cfg(feature = "urdf_ik")]
use std::fmt;
#[cfg(feature = "urdf_ik")]
use std::hash::{Hash, Hasher};
use vizij_api_core::value::VizijKind;

#[cfg(feature = "urdf_ik")]
/// Cached state for URDF chains shared by IK and FK nodes.
pub struct UrdfKinematicsState {
    pub hash: u64,
    pub dofs: usize,
    pub joint_names: Vec<String>,
    pub chain: k::SerialChain<f32>,
}

#[cfg(feature = "urdf_ik")]
impl fmt::Debug for UrdfKinematicsState {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("UrdfKinematicsState")
            .field("hash", &self.hash)
            .field("dofs", &self.dofs)
            .field("joint_names", &self.joint_names)
            .finish()
    }
}

#[cfg(feature = "urdf_ik")]
impl UrdfKinematicsState {
    /// Construct a new kinematics state from a serial chain and its joint names.
    pub fn new(hash: u64, chain: k::SerialChain<f32>, joint_names: Vec<String>) -> Self {
        let dofs = chain.dof();
        if dofs > 0 {
            let zero_seed = vec![0.0f32; dofs];
            chain.set_joint_positions_clamped(&zero_seed);
        }
        UrdfKinematicsState {
            hash,
            dofs,
            joint_names,
            chain,
        }
    }

    /// Produce a record value mapping joint names to solved angles.
    pub fn solution_record<V: GraphValue>(&self, joints: &[f32]) -> V {
        V::record(
            self.joint_names
                .iter()
                .zip(joints.iter())
                .map(|(name, angle)| (name.as_str(), V::float(*angle))),
        )
    }
}

#[cfg(feature = "urdf_ik")]
/// Key that uniquely identifies a URDF chain configuration for caching.
pub struct IkKey<'a> {
    pub hash: u64,
    pub urdf_xml: &'a str,
    pub root_link: &'a str,
    pub tip_link: &'a str,
}

#[cfg(feature = "urdf_ik")]
/// Compute a stable hash for a URDF IK configuration.
pub fn hash_urdf_config(urdf_xml: &str, root_link: &str, tip_link: &str) -> u64 {
    let mut hasher = DefaultHasher::new();
    urdf_xml.hash(&mut hasher);
    root_link.hash(&mut hasher);
    tip_link.hash(&mut hasher);
    hasher.finish()
}

#[cfg(feature = "urdf_ik")]
/// Build a serial chain between `root` and `tip` from the provided URDF XML.
pub fn build_chain_from_urdf(
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
/// Apply optional joint-space weights to the solver.
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
/// Solve for joint positions that reach `target_pos` while respecting `weights`.
pub fn solve_position(
    state: &mut UrdfKinematicsState,
    solver: &mut k::JacobianIkSolver<f32>,
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

    apply_weights(solver, seed, weights)?;

    solver.num_max_try = max_iters.max(1) as usize;
    solver.allowable_target_distance = tol_pos;
    solver.allowable_target_angle = std::f32::consts::PI;

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

    solver
        .solve_with_constraints(&state.chain, &target_pose, &constraints)
        .map_err(|err| format!("IK solve failed: {err}"))?;

    Ok(state.chain.joint_positions())
}

#[cfg(feature = "urdf_ik")]
fn scalar_from_value<V: GraphValue>(value: &V) -> Result<f32, String> {
    let mismatch = || format!("expected numeric scalar, received {:?}", V::kind(value));
    match V::kind(value) {
        VizijKind::Float => V::as_float(value).ok_or_else(mismatch),
        VizijKind::Bool => Ok(if V::as_bool(value).unwrap_or(false) {
            1.0
        } else {
            0.0
        }),
        VizijKind::Vec2 => Ok(V::as_vec2(value).ok_or_else(mismatch)?[0]),
        VizijKind::Vec3 => Ok(V::as_vec3(value).ok_or_else(mismatch)?[0]),
        VizijKind::Vec4 => Ok(V::as_vec4(value).ok_or_else(mismatch)?[0]),
        VizijKind::Quat => Ok(V::as_quat(value).ok_or_else(mismatch)?[0]),
        VizijKind::Vector => V::as_vector(value)
            .and_then(|xs| xs.first().copied())
            .ok_or_else(|| "vector is empty".to_string()),
        VizijKind::Array => {
            let items = V::as_array(value).ok_or_else(mismatch)?;
            if items.len() == 1 {
                scalar_from_value(&items[0])
            } else {
                Err(mismatch())
            }
        }
        VizijKind::Enum => {
            let (_, payload) = V::as_enumeration(value).ok_or_else(mismatch)?;
            scalar_from_value(payload)
        }
        _ => Err(mismatch()),
    }
}

#[cfg(feature = "urdf_ik")]
fn align_sequence_with_defaults(
    values: &[f32],
    expected: usize,
    joint_names: &[String],
    defaults: &HashMap<&str, f32>,
) -> Vec<f32> {
    let mut result = Vec::with_capacity(expected);
    for (index, joint) in joint_names.iter().enumerate() {
        if let Some(value) = values.get(index) {
            result.push(*value);
        } else if let Some(default_value) = defaults.get(joint.as_str()) {
            result.push(*default_value);
        } else {
            result.push(0.0);
        }
    }
    result
}

#[cfg(feature = "urdf_ik")]
/// Coerce input joint data into a vector matching the chain joint order.
pub fn fetch_joint_vector<V: GraphValue>(
    value: &V,
    expected: usize,
    defaults: Option<&[(String, f32)]>,
    joint_names: &[String],
) -> Result<Vec<f32>, String> {
    let mut default_map: HashMap<&str, f32> = HashMap::new();
    if let Some(entries) = defaults {
        for (name, angle) in entries {
            default_map.insert(name.as_str(), *angle);
        }
    }

    match V::kind(value) {
        VizijKind::Record => {
            let entries = V::as_record(value).unwrap_or_default();
            let by_name: HashMap<&str, &V> = entries.into_iter().collect();
            let mut result = Vec::with_capacity(expected);
            for joint in joint_names {
                if let Some(entry) = by_name.get(joint.as_str()) {
                    result.push(scalar_from_value(*entry)?);
                } else if let Some(default_value) = default_map.get(joint.as_str()) {
                    result.push(*default_value);
                } else {
                    result.push(0.0);
                }
            }
            Ok(result)
        }
        VizijKind::Array => {
            let items = V::as_array(value).unwrap_or_default();
            let mut sequence = Vec::with_capacity(items.len());
            for item in items {
                sequence.push(scalar_from_value(item)?);
            }
            Ok(align_sequence_with_defaults(
                &sequence,
                expected,
                joint_names,
                &default_map,
            ))
        }
        _ => {
            let sequence = V::to_vector(value);
            Ok(align_sequence_with_defaults(
                &sequence,
                expected,
                joint_names,
                &default_map,
            ))
        }
    }
}

#[cfg(feature = "urdf_ik")]
/// Apply joint values to the cached serial chain.
pub fn apply_joint_positions(
    state: &mut UrdfKinematicsState,
    joints: &[f32],
) -> Result<(), String> {
    if joints.len() != state.dofs {
        return Err(format!(
            "joint vector length {} does not match chain DoF {}",
            joints.len(),
            state.dofs
        ));
    }
    state
        .chain
        .set_joint_positions(joints)
        .map_err(|err| format!("failed to apply joint values: {err}"))?;
    Ok(())
}

#[cfg(feature = "urdf_ik")]
/// Extract the current tip pose (position + quaternion) from the chain.
pub fn tip_pose(state: &UrdfKinematicsState) -> ([f32; 3], [f32; 4]) {
    let end = state.chain.end_transform();
    let pos = end.translation.vector;
    let rot = end.rotation;
    ([pos.x, pos.y, pos.z], [rot.i, rot.j, rot.k, rot.w])
}

#[allow(clippy::too_many_arguments)]
#[cfg(feature = "urdf_ik")]
/// Solve for joint positions that reach both `target_pos` and `target_rot`.
pub fn solve_pose(
    state: &mut UrdfKinematicsState,
    solver: &mut k::JacobianIkSolver<f32>,
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

    apply_weights(solver, seed, weights)?;

    solver.num_max_try = max_iters.max(1) as usize;
    solver.allowable_target_distance = tol_pos;
    solver.allowable_target_angle = tol_rot;

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

    solver
        .solve(&state.chain, &target_pose)
        .map_err(|err| format!("IK solve failed: {err}"))?;

    Ok(state.chain.joint_positions())
}

#[cfg(feature = "urdf_ik")]
/// Extract the numeric components from a supported value type.
pub fn vector_from_value<V: GraphValue>(value: &V, label: &str) -> Result<Vec<f32>, String> {
    if let Some(xs) = V::as_vector(value) {
        return Ok(xs.to_vec());
    }
    V::as_vec2(value)
        .map(|a| a.to_vec())
        .or_else(|| V::as_vec3(value).map(|a| a.to_vec()))
        .or_else(|| V::as_vec4(value).map(|a| a.to_vec()))
        .or_else(|| V::as_quat(value).map(|a| a.to_vec()))
        .ok_or_else(|| {
            format!(
                "{label} expects a numeric vector, received {:?}",
                V::kind(value)
            )
        })
}

#[cfg(feature = "urdf_ik")]
/// Interpet a value as a quaternion `[x, y, z, w]`.
pub fn quat_from_value<V: GraphValue>(value: &V, label: &str) -> Result<[f32; 4], String> {
    V::as_quat(value)
        .or_else(|| V::as_vec4(value))
        .or_else(|| match V::as_vector(value) {
            Some(xs) if xs.len() == 4 => Some([xs[0], xs[1], xs[2], xs[3]]),
            _ => None,
        })
        .ok_or_else(|| {
            format!(
                "{label} expects a quaternion (x, y, z, w), received {:?}",
                V::kind(value)
            )
        })
}
