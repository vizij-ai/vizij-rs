//! URDF inverse kinematics helpers gated behind the `urdf_ik` feature.

use hashbrown::HashMap;
#[cfg(feature = "urdf_ik")]
use k::InverseKinematicsSolver;
#[cfg(feature = "urdf_ik")]
use std::collections::hash_map::DefaultHasher;
#[cfg(feature = "urdf_ik")]
use std::fmt;
#[cfg(feature = "urdf_ik")]
use std::hash::{Hash, Hasher};
use vizij_api_core::Value;

#[cfg(feature = "urdf_ik")]
/// Cached state for URDF IK nodes.
pub struct UrdfIkState {
    pub hash: u64,
    pub dofs: usize,
    pub joint_names: Vec<String>,
    pub chain: k::SerialChain<f32>,
    pub solver: k::JacobianIkSolver<f32>,
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
    /// Construct a new IK state from a serial chain and its joint names.
    pub fn new(hash: u64, chain: k::SerialChain<f32>, joint_names: Vec<String>) -> Self {
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

    /// Produce a record value mapping joint names to solved angles.
    pub fn solution_record(&self, joints: &[f32]) -> Value {
        let mut map = HashMap::with_capacity(self.joint_names.len());
        for (name, angle) in self.joint_names.iter().zip(joints.iter()) {
            map.insert(name.clone(), Value::Float(*angle));
        }
        Value::Record(map)
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
/// Solve for joint positions that reach both `target_pos` and `target_rot`.
pub fn solve_pose(
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
/// Extract the numeric components from a supported value type.
pub fn vector_from_value(value: &Value, label: &str) -> Result<Vec<f32>, String> {
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
/// Interpet a [`Value`] as a quaternion `[x, y, z, w]`.
pub fn quat_from_value(value: &Value, label: &str) -> Result<[f32; 4], String> {
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
