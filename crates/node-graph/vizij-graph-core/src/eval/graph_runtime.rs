//! Mutable runtime state that persists across node evaluations.

use crate::types::NodeId;
use hashbrown::{hash_map::Entry, HashMap};
use vizij_api_core::WriteBatch;

use super::urdfik::{build_chain_from_urdf, IkKey, UrdfIkState};
use super::value_layout::{FlatValue, PortValue, ValueLayout};

/// Internal integration state for a spring node. Values remain flattened for efficiency.
#[derive(Clone, Debug)]
pub struct SpringState {
    pub layout: ValueLayout,
    pub position: Vec<f32>,
    pub velocity: Vec<f32>,
    pub target: Vec<f32>,
}

impl SpringState {
    /// Create a new state seeded with the provided flat value.
    fn new(flat: &FlatValue) -> Self {
        let len = flat.data.len();
        SpringState {
            layout: flat.layout.clone(),
            position: flat.data.clone(),
            velocity: vec![0.0; len],
            target: flat.data.clone(),
        }
    }

    /// Reset the layout and buffers to match the most recent input.
    fn reset(&mut self, flat: &FlatValue) {
        let len = flat.data.len();
        self.layout = flat.layout.clone();
        self.position = flat.data.clone();
        self.velocity = vec![0.0; len];
        self.target = flat.data.clone();
    }
}

/// Integration state for a damp node.
#[derive(Clone, Debug)]
pub struct DampState {
    pub layout: ValueLayout,
    pub value: Vec<f32>,
}

impl DampState {
    /// Create a new state seeded with the provided flat value.
    fn new(flat: &FlatValue) -> Self {
        DampState {
            layout: flat.layout.clone(),
            value: flat.data.clone(),
        }
    }

    /// Reset the cached layout and value to match the incoming data.
    fn reset(&mut self, flat: &FlatValue) {
        self.layout = flat.layout.clone();
        self.value = flat.data.clone();
    }
}

/// Integration state for a slew node.
#[derive(Clone, Debug)]
pub struct SlewState {
    pub layout: ValueLayout,
    pub value: Vec<f32>,
}

impl SlewState {
    /// Create a new state seeded with the provided flat value.
    fn new(flat: &FlatValue) -> Self {
        SlewState {
            layout: flat.layout.clone(),
            value: flat.data.clone(),
        }
    }

    /// Reset the cached layout and value to match the incoming data.
    fn reset(&mut self, flat: &FlatValue) {
        self.layout = flat.layout.clone();
        self.value = flat.data.clone();
    }
}

/// State stored for each node that requires persistence across frames.
#[derive(Debug)]
pub enum NodeRuntimeState {
    Spring(SpringState),
    Damp(DampState),
    Slew(SlewState),
    #[cfg(feature = "urdf_ik")]
    UrdfIk(UrdfIkState),
}

/// Runtime data shared by all node evaluations.
#[derive(Debug, Default)]
pub struct GraphRuntime {
    pub t: f32,
    pub dt: f32,
    pub outputs: HashMap<NodeId, HashMap<String, PortValue>>,
    pub writes: WriteBatch,
    pub node_states: HashMap<NodeId, NodeRuntimeState>,
}

impl GraphRuntime {
    /// Fetch the spring state for `node_id`, creating or reinitialising it from `flat` as needed.
    pub fn spring_state_mut<'a>(
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

    /// Fetch the damp state for `node_id`, creating or reinitialising it from `flat` as needed.
    pub fn damp_state_mut<'a>(
        &'a mut self,
        node_id: &NodeId,
        flat: &FlatValue,
    ) -> &'a mut DampState {
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

    /// Fetch the slew state for `node_id`, creating or reinitialising it from `flat` as needed.
    pub fn slew_state_mut<'a>(
        &'a mut self,
        node_id: &NodeId,
        flat: &FlatValue,
    ) -> &'a mut SlewState {
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
    /// Fetch the cached URDF IK solver state for `node_id`, rebuilding it if the configuration
    /// hash changes.
    pub fn ik_state_mut<'a>(
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
