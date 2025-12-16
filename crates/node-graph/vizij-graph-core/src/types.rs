use hashbrown::{HashMap, HashSet};
use serde::{Deserialize, Serialize};
use vizij_api_core::{Shape, TypedPath, Value};

pub type NodeId = String;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
#[serde(rename_all = "lowercase")]
pub enum SelectorSegment {
    Field(String),
    Index(usize),
}

pub type Selector = Vec<SelectorSegment>;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
#[serde(rename_all = "lowercase")]
pub enum NodeType {
    // Scalars / arithmetic
    Constant,
    Slider,
    MultiSlider,
    Add,
    Subtract,
    Multiply,
    Divide,
    Power,
    Log,
    Abs,
    Modulo,
    Sqrt,
    Sign,
    Min,
    Max,
    Round,
    Sin,
    Cos,
    Tan,

    // Time & generators
    Time,
    Oscillator, // sin(2π f t + phase)

    // Transition & smoothing
    Spring,
    Damp,
    Slew,

    // Logic
    And,
    Or,
    Not,
    Xor,

    // Conditional
    GreaterThan,
    LessThan,
    Equal,
    NotEqual,
    If,
    // Control-flow helper: route by label
    Case,

    // Ranges
    Clamp,
    Remap,
    #[serde(rename = "centered_remap")]
    CenteredRemap,
    #[serde(rename = "piecewise_remap")]
    PiecewiseRemap,

    // 3D-specific utilities
    Vec3Cross,

    // Generic vector utilities
    VectorConstant,
    VectorAdd,
    VectorSubtract,
    VectorMultiply, // component-wise
    VectorScale,    // scalar * vector
    VectorNormalize,
    VectorDot,
    VectorLength,
    VectorIndex,
    Join,
    Split,
    VectorMin,
    VectorMax,
    VectorMean,
    VectorMedian,
    VectorMode,

    // Blend helpers
    WeightedSumVector,
    #[serde(rename = "default-blend")]
    DefaultBlend,
    BlendWeightedAverage,
    BlendAdditive,
    BlendMultiply,
    BlendWeightedOverlay,
    BlendWeightedAverageOverlay,
    BlendMax,

    // Robotics
    InverseKinematics,
    UrdfIkPosition,
    UrdfIkPose,
    UrdfFk,

    // IO
    Input,
    // Sinks (for external binding in hosts)
    Output,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct NodeParams {
    pub value: Option<Value>,
    pub frequency: Option<f32>,
    pub phase: Option<f32>,
    #[serde(default)]
    pub min: f32,
    #[serde(default)]
    pub max: f32,
    // Optional defaults for Vec3 constructor
    pub x: Option<f32>,
    pub y: Option<f32>,
    pub z: Option<f32>,
    // For Remap
    pub in_min: Option<f32>,
    pub in_max: Option<f32>,
    pub out_min: Option<f32>,
    pub out_max: Option<f32>,
    #[serde(default)]
    pub round_mode: Option<RoundMode>,
    // For Piecewise Remap
    #[serde(default)]
    pub clamp: Option<bool>,
    // For Centered Remap
    pub in_low: Option<f32>,
    pub in_anchor: Option<f32>,
    pub in_high: Option<f32>,
    pub out_low: Option<f32>,
    pub out_anchor: Option<f32>,
    pub out_high: Option<f32>,
    // For IK
    pub bone1: Option<f32>,
    pub bone2: Option<f32>,
    pub bone3: Option<f32>,
    pub urdf_xml: Option<String>,
    pub root_link: Option<String>,
    pub tip_link: Option<String>,
    pub seed: Option<Vec<f32>>,
    pub weights: Option<Vec<f32>>,
    pub max_iters: Option<u32>,
    pub tol_pos: Option<f32>,
    pub tol_rot: Option<f32>,
    #[serde(default)]
    pub joint_defaults: Option<Vec<(String, f32)>>,
    // For Splitter
    pub index: Option<f32>,
    // For Split sizes (vector of sizes, floored to usize)
    pub sizes: Option<Vec<f32>>,

    // Transition parameters
    pub stiffness: Option<f32>,
    pub damping: Option<f32>,
    pub mass: Option<f32>,
    pub half_life: Option<f32>,
    pub max_rate: Option<f32>,

    // For Case routing nodes
    #[serde(default)]
    pub case_labels: Option<Vec<String>>,

    // Optional target typed path for Output nodes and sinks.
    // Example: "robot1/Arm/Joint3.translation"
    #[serde(default)]
    pub path: Option<TypedPath>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "lowercase")]
pub enum RoundMode {
    #[default]
    Floor,
    Ceil,
    Trunc,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NodeSpec {
    pub id: NodeId,
    /// Accept either `"type"` (preferred) or legacy `"kind"` in incoming JSON.
    #[serde(rename = "type", alias = "kind")]
    pub kind: NodeType,
    #[serde(default)]
    pub params: NodeParams,
    #[serde(default)]
    pub output_shapes: HashMap<String, Shape>,
    #[serde(default)]
    pub input_defaults: HashMap<String, InputDefault>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct GraphSpec {
    pub nodes: Vec<NodeSpec>,
    #[serde(default)]
    pub edges: Vec<EdgeSpec>,
    /// Optional caller-managed cache key for plan reuse. When zero, the runtime falls back to
    /// hashing to detect structural changes.
    #[serde(default, skip_serializing_if = "crate::types::is_zero")]
    pub version: u64,
    /// Fingerprint of the structural layout (nodes/edges/params) used alongside `version`.
    #[serde(default, skip_serializing_if = "crate::types::is_zero")]
    pub fingerprint: u64,
}

pub(crate) fn is_zero(v: &u64) -> bool {
    *v == 0
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InputConnection {
    #[serde(default)]
    pub node_id: Option<NodeId>,
    #[serde(default = "default_output_key")]
    pub output_key: String,
    #[serde(default)]
    pub selector: Option<Selector>,
    #[serde(rename = "default", default)]
    pub default_value: Option<Value>,
    #[serde(default)]
    pub default_shape: Option<Shape>,
}

fn default_output_key() -> String {
    "out".to_string()
}

impl Default for InputConnection {
    fn default() -> Self {
        Self {
            node_id: None,
            output_key: default_output_key(),
            selector: None,
            default_value: None,
            default_shape: None,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InputDefault {
    #[serde(rename = "value")]
    pub value: Value,
    #[serde(default)]
    pub shape: Option<Shape>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EdgeOutputEndpoint {
    pub node_id: NodeId,
    #[serde(default = "default_output_key")]
    pub output: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EdgeInputEndpoint {
    pub node_id: NodeId,
    pub input: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EdgeSpec {
    pub from: EdgeOutputEndpoint,
    pub to: EdgeInputEndpoint,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[serde(default)]
    pub selector: Option<Selector>,
}

impl GraphSpec {
    /// Recompute the structural fingerprint for this spec without mutating the version counter.
    pub fn recompute_fingerprint(&self) -> u64 {
        crate::eval::fingerprint_spec(self)
    }

    /// Seed or bump the spec version and refresh the fingerprint for plan-cache reuse.
    ///
    /// - If `version` is zero, it is set to 1.
    /// - Otherwise, the version is saturated by `saturating_add(1)` to avoid wrap.
    pub fn with_cache(mut self) -> Self {
        self.version = if self.version == 0 {
            1
        } else {
            self.version.saturating_add(1)
        };
        self.fingerprint = self.recompute_fingerprint();
        self
    }

    pub fn input_connections(
        &self,
    ) -> Result<HashMap<NodeId, HashMap<String, InputConnection>>, String> {
        let mut map: HashMap<NodeId, HashMap<String, InputConnection>> = HashMap::new();
        let known: HashSet<&NodeId> = self.nodes.iter().map(|n| &n.id).collect();

        for node in &self.nodes {
            if node.input_defaults.is_empty() {
                continue;
            }

            let entry = map.entry(node.id.clone()).or_default();
            for (input, default_spec) in &node.input_defaults {
                entry.insert(
                    input.clone(),
                    InputConnection {
                        default_value: Some(default_spec.value.clone()),
                        default_shape: default_spec.shape.clone(),
                        ..InputConnection::default()
                    },
                );
            }
        }

        let mut seen_inputs: HashSet<(NodeId, String)> = HashSet::new();

        for edge in &self.edges {
            if !known.contains(&edge.from.node_id) {
                return Err(format!(
                    "edge references missing source node '{}'",
                    edge.from.node_id
                ));
            }
            if !known.contains(&edge.to.node_id) {
                return Err(format!(
                    "edge references missing target node '{}'",
                    edge.to.node_id
                ));
            }

            let key = (edge.to.node_id.clone(), edge.to.input.clone());
            if !seen_inputs.insert(key.clone()) {
                return Err(format!("duplicate edge for input '{}:{}'", key.0, key.1));
            }

            let entry = map.entry(edge.to.node_id.clone()).or_default();
            let connection_entry = entry
                .entry(edge.to.input.clone())
                .or_insert_with(InputConnection::default);

            connection_entry.node_id = Some(edge.from.node_id.clone());
            connection_entry.output_key = edge.from.output.clone();
            connection_entry.selector = edge.selector.clone();
        }

        Ok(map)
    }
}
