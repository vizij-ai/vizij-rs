use hashbrown::{HashMap, HashSet};
use serde::{Deserialize, Serialize};
use vizij_api_core::{Shape, TypedPath, Value};

/// Node identifier used for edges and runtime lookups.
pub type NodeId = String;

/// Selector segment for projecting structured values.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
#[serde(rename_all = "lowercase")]
pub enum SelectorSegment {
    /// Record/struct field access.
    Field(String),
    /// Array/list/vector index access.
    Index(usize),
}

/// Selector path used to project a node output.
pub type Selector = Vec<SelectorSegment>;

/// Supported node kinds in a [`GraphSpec`].
///
/// These values map to node registry entries and JSON `type` strings.
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

/// Parameter payload for node configuration.
///
/// The fields here are optional because the node registry declares which inputs are relevant
/// for a given [`NodeType`]. Unused fields are ignored during evaluation.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct NodeParams {
    /// Constant payload used by `Constant`, `VectorConstant`, and related nodes.
    pub value: Option<Value>,
    /// Oscillator frequency in Hz.
    pub frequency: Option<f32>,
    /// Oscillator phase offset in radians.
    pub phase: Option<f32>,
    /// Lower bound for slider, clamp, and remap helpers.
    #[serde(default)]
    pub min: f32,
    /// Upper bound for slider, clamp, and remap helpers.
    #[serde(default)]
    pub max: f32,
    // Optional defaults for Vec3 constructor
    /// Default x component for vec3 constructors.
    pub x: Option<f32>,
    /// Default y component for vec3 constructors.
    pub y: Option<f32>,
    /// Default z component for vec3 constructors.
    pub z: Option<f32>,
    // For Remap
    /// Remap input minimum.
    pub in_min: Option<f32>,
    /// Remap input maximum.
    pub in_max: Option<f32>,
    /// Remap output minimum.
    pub out_min: Option<f32>,
    /// Remap output maximum.
    pub out_max: Option<f32>,
    /// Rounding mode for `Round` nodes.
    #[serde(default)]
    pub round_mode: Option<RoundMode>,
    // For Piecewise Remap
    /// Whether piecewise remap clamps outside the provided range.
    #[serde(default)]
    pub clamp: Option<bool>,
    // For Centered Remap
    /// Centered remap input low value.
    pub in_low: Option<f32>,
    /// Centered remap input anchor value.
    pub in_anchor: Option<f32>,
    /// Centered remap input high value.
    pub in_high: Option<f32>,
    /// Centered remap output low value.
    pub out_low: Option<f32>,
    /// Centered remap output anchor value.
    pub out_anchor: Option<f32>,
    /// Centered remap output high value.
    pub out_high: Option<f32>,
    // For IK
    /// First bone length for analytic IK.
    pub bone1: Option<f32>,
    /// Second bone length for analytic IK.
    pub bone2: Option<f32>,
    /// Third bone length for analytic IK.
    pub bone3: Option<f32>,
    /// URDF payload for IK nodes.
    pub urdf_xml: Option<String>,
    /// Root link name for URDF chains.
    pub root_link: Option<String>,
    /// Tip link name for URDF chains.
    pub tip_link: Option<String>,
    /// Optional joint seed for URDF IK solvers.
    pub seed: Option<Vec<f32>>,
    /// Optional joint-space weights for URDF IK.
    pub weights: Option<Vec<f32>>,
    /// Maximum IK solver iterations.
    pub max_iters: Option<u32>,
    /// Positional tolerance for IK solvers.
    pub tol_pos: Option<f32>,
    /// Rotational tolerance for IK solvers.
    pub tol_rot: Option<f32>,
    /// Default joint values keyed by joint name.
    #[serde(default)]
    pub joint_defaults: Option<Vec<(String, f32)>>,
    // For Splitter
    /// Index for `VectorIndex` and similar nodes.
    pub index: Option<f32>,
    // For Split sizes (vector of sizes, floored to usize)
    /// Sizes for `Split` outputs (floored to integers).
    pub sizes: Option<Vec<f32>>,

    // Transition parameters
    /// Spring stiffness constant.
    pub stiffness: Option<f32>,
    /// Spring damping value.
    pub damping: Option<f32>,
    /// Spring mass value.
    pub mass: Option<f32>,
    /// Damp half-life in seconds.
    pub half_life: Option<f32>,
    /// Slew max rate per second.
    pub max_rate: Option<f32>,

    // For Case routing nodes
    /// Explicit label list for `Case` routing.
    #[serde(default)]
    pub case_labels: Option<Vec<String>>,

    // Optional target typed path for Output nodes and sinks.
    // Example: "robot1/Arm/Joint3.translation"
    /// Target path for `Output` nodes.
    #[serde(default)]
    pub path: Option<TypedPath>,
}

/// Round behavior for the `Round` node.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "lowercase")]
pub enum RoundMode {
    /// Round toward negative infinity.
    #[default]
    Floor,
    /// Round toward positive infinity.
    Ceil,
    /// Round toward zero.
    Trunc,
}

/// Node entry inside a [`GraphSpec`].
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NodeSpec {
    /// Unique node identifier.
    pub id: NodeId,
    /// Accept either `"type"` (preferred) or legacy `"kind"` in incoming JSON.
    #[serde(rename = "type", alias = "kind")]
    pub kind: NodeType,
    /// Parameter values for this node.
    #[serde(default)]
    pub params: NodeParams,
    /// Optional declared output shapes keyed by output port name.
    #[serde(default)]
    pub output_shapes: HashMap<String, Shape>,
    /// Default values for named inputs.
    #[serde(default)]
    pub input_defaults: HashMap<String, InputDefault>,
}

/// Top-level graph document consumed by the evaluator.
///
/// Use [`GraphSpec::with_cache`] to seed the plan cache fingerprint/version.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct GraphSpec {
    /// Nodes in the graph.
    pub nodes: Vec<NodeSpec>,
    /// Directed edges connecting node outputs to inputs.
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

/// Connection description used when building input bindings.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InputConnection {
    /// Source node id, or `None` when only defaults are supplied.
    #[serde(default)]
    pub node_id: Option<NodeId>,
    /// Output port key on the source node.
    #[serde(default = "default_output_key")]
    pub output_key: String,
    /// Optional selector path applied to the source value.
    #[serde(default)]
    pub selector: Option<Selector>,
    /// Default value used when no edge provides an input.
    #[serde(rename = "default", default)]
    pub default_value: Option<Value>,
    /// Default shape hint applied to the default value.
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

/// Default value and optional shape for a named input.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InputDefault {
    /// Default value for the input.
    #[serde(rename = "value")]
    pub value: Value,
    /// Optional shape hint for the input.
    #[serde(default)]
    pub shape: Option<Shape>,
}

/// Output endpoint referenced by an [`EdgeSpec`].
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EdgeOutputEndpoint {
    /// Source node identifier.
    pub node_id: NodeId,
    /// Output port key on the source node.
    #[serde(default = "default_output_key")]
    pub output: String,
}

/// Input endpoint referenced by an [`EdgeSpec`].
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EdgeInputEndpoint {
    /// Target node identifier.
    pub node_id: NodeId,
    /// Input port key on the target node.
    pub input: String,
}

/// Directed edge between two nodes.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EdgeSpec {
    /// Source endpoint.
    pub from: EdgeOutputEndpoint,
    /// Destination endpoint.
    pub to: EdgeInputEndpoint,
    /// Optional selector path applied to the source value.
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

    /// Build a map of node inputs to their resolved connections or defaults.
    ///
    /// # Errors
    ///
    /// Returns errors for missing nodes or duplicate input edges.
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
