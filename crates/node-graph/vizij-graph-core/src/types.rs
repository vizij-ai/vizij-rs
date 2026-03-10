//! Canonical graph specification types shared across all Vizij graph hosts.
//!
//! These structs are the serde-facing representation consumed by the evaluator, wasm bridge,
//! fixture tooling, and orchestration layer.

use hashbrown::{HashMap, HashSet};
use serde::{Deserialize, Serialize};
use vizij_api_core::{Shape, TypedPath, Value};

/// Stable node identifier within a single [`GraphSpec`].
pub type NodeId = String;

/// One step of a selector path applied to a structured output value.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
#[serde(rename_all = "lowercase")]
pub enum SelectorSegment {
    /// Traverse into a record/struct field by name.
    Field(String),
    /// Traverse into an array/vector/list position by zero-based index.
    Index(usize),
}

/// Selector path applied after reading an upstream output port.
pub type Selector = Vec<SelectorSegment>;

/// Canonical node kinds accepted by graph JSON and emitted by the schema registry.
///
/// These names serialize using lowercase serde names (with a few explicit aliases), so hosts
/// should prefer this enum over hard-coded string literals when possible.
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
    /// Emits `sin(2π f t + phase)` using the node's configured parameters.
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
    /// Control-flow helper that routes one of several branches by label.
    Case,

    // Ranges
    Clamp,
    Remap,
    /// Remap around an anchor point with independent low/high ranges.
    #[serde(rename = "centered_remap")]
    CenteredRemap,
    /// Remap through multiple segments, optionally clamping at the ends.
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
    ToVector,
    FromVector,

    // Noise generators
    SimpleNoise,
    PerlinNoise,
    SimplexNoise,

    // Blend helpers
    WeightedSumVector,
    /// Host-default blending strategy for structured values.
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
    /// URDF-backed IK solver targeting translation only.
    UrdfIkPosition,
    /// URDF-backed IK solver targeting translation and rotation.
    UrdfIkPose,
    /// URDF-backed forward-kinematics solver.
    UrdfFk,

    // Records
    BuildRecord,
    ReadRecord,
    SwitchRecord,
    MergeRecord,
    SplitRecord,
    MathMultRecord,
    MathAddRecord,
    MathDivRecord,
    MathSubRecord,

    // IO
    /// Reads a staged host value by typed path.
    Input,
    /// Writes a value to a host-visible typed path sink.
    Output,
}

/// Node-construction parameters consumed selectively by different [`NodeType`] variants.
///
/// Most fields are optional and ignored by node kinds that do not read them. This keeps the JSON
/// surface forward-compatible while still allowing a single serde contract for all nodes.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct NodeParams {
    /// Literal value for constant-like nodes and inline defaults.
    pub value: Option<Value>,
    /// Frequency parameter used by oscillators and noise-style generators.
    pub frequency: Option<f32>,
    /// Phase offset used by oscillators.
    pub phase: Option<f32>,
    /// Lower bound used by range-based nodes such as sliders, clamp, and remap variants.
    #[serde(default)]
    pub min: f32,
    /// Upper bound used by range-based nodes such as sliders, clamp, and remap variants.
    #[serde(default)]
    pub max: f32,
    /// Optional `x` component seed for vector-construction nodes.
    pub x: Option<f32>,
    /// Optional `y` component seed for vector-construction nodes.
    pub y: Option<f32>,
    /// Optional `z` component seed for vector-construction nodes.
    pub z: Option<f32>,
    /// Source low bound for remap-style nodes.
    pub in_min: Option<f32>,
    /// Source high bound for remap-style nodes.
    pub in_max: Option<f32>,
    /// Destination low bound for remap-style nodes.
    pub out_min: Option<f32>,
    /// Destination high bound for remap-style nodes.
    pub out_max: Option<f32>,
    /// Rounding mode used by [`NodeType::Round`].
    #[serde(default)]
    pub round_mode: Option<RoundMode>,
    /// Clamp behavior for [`NodeType::PiecewiseRemap`].
    #[serde(default)]
    pub clamp: Option<bool>,
    /// Input low anchor for [`NodeType::CenteredRemap`].
    pub in_low: Option<f32>,
    /// Input center anchor for [`NodeType::CenteredRemap`].
    pub in_anchor: Option<f32>,
    /// Input high anchor for [`NodeType::CenteredRemap`].
    pub in_high: Option<f32>,
    /// Output low anchor for [`NodeType::CenteredRemap`].
    pub out_low: Option<f32>,
    /// Output center anchor for [`NodeType::CenteredRemap`].
    pub out_anchor: Option<f32>,
    /// Output high anchor for [`NodeType::CenteredRemap`].
    pub out_high: Option<f32>,
    /// Bone length for lightweight analytic IK nodes.
    pub bone1: Option<f32>,
    /// Bone length for lightweight analytic IK nodes.
    pub bone2: Option<f32>,
    /// Bone length for lightweight analytic IK nodes.
    pub bone3: Option<f32>,
    /// Raw URDF XML fed to URDF-backed kinematics nodes.
    pub urdf_xml: Option<String>,
    /// Root link name used when building a URDF chain.
    pub root_link: Option<String>,
    /// Tip/end-effector link name used when building a URDF chain.
    pub tip_link: Option<String>,
    /// Optional seed vector passed into IK solvers.
    pub seed: Option<Vec<f32>>,
    /// Optional per-joint weights passed into IK solvers.
    pub weights: Option<Vec<f32>>,
    /// Maximum solver iterations for IK nodes.
    pub max_iters: Option<u32>,
    /// Positional tolerance for IK convergence.
    pub tol_pos: Option<f32>,
    /// Rotational tolerance for IK convergence.
    pub tol_rot: Option<f32>,
    /// Default joint values keyed by joint name for URDF-backed nodes.
    #[serde(default)]
    pub joint_defaults: Option<Vec<(String, f32)>>,
    /// Zero-based component index used by indexing/splitting nodes.
    pub index: Option<f32>,
    /// Segment sizes for [`NodeType::Split`], floored to whole-number widths by the evaluator.
    pub sizes: Option<Vec<f32>>,

    /// Noise seed value passed into procedural noise nodes.
    pub noise_seed: Option<f32>,
    /// Number of noise octaves to accumulate.
    pub octaves: Option<f32>,
    /// Frequency multiplier between octaves for noise nodes.
    pub lacunarity: Option<f32>,
    /// Amplitude falloff between octaves for noise nodes.
    pub persistence: Option<f32>,

    /// Spring stiffness for smoothing nodes.
    pub stiffness: Option<f32>,
    /// Damping coefficient for smoothing nodes.
    pub damping: Option<f32>,
    /// Effective mass for spring-style nodes.
    pub mass: Option<f32>,
    /// Half-life parameter for damp nodes.
    pub half_life: Option<f32>,
    /// Maximum change rate for slew nodes.
    pub max_rate: Option<f32>,

    /// Branch labels for [`NodeType::Case`], matched in declaration order.
    #[serde(default)]
    pub case_labels: Option<Vec<String>>,

    // For BuildRecord/ReadRecord – one key string per variadic slot, in slot order
    #[serde(default)]
    pub record_keys: Option<Vec<String>>,

    // For SplitRecord – comma-separated field keys to include in `included` output
    #[serde(default)]
    pub keys: Option<String>,

    // Optional target typed path for Output nodes and sinks.
    // Example: "robot1/Arm/Joint3.translation"
    /// Optional sink path for [`NodeType::Output`] and host-visible sink nodes.
    ///
    /// Example: `"robot1/Arm/Joint3.translation"`.
    #[serde(default)]
    pub path: Option<TypedPath>,
}

/// Rounding strategy for [`NodeType::Round`].
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

/// Single node declaration within a [`GraphSpec`].
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NodeSpec {
    /// Unique node identifier referenced by edges and runtime outputs.
    pub id: NodeId,
    /// Accept either `"type"` (preferred) or legacy `"kind"` in incoming JSON.
    #[serde(rename = "type", alias = "kind")]
    pub kind: NodeType,
    /// Parameter bag interpreted according to [`Self::kind`].
    #[serde(default)]
    pub params: NodeParams,
    /// Optional declared output shapes keyed by port name.
    #[serde(default)]
    pub output_shapes: HashMap<String, Shape>,
    /// Inline default values used when an input port has no inbound edge.
    #[serde(default)]
    pub input_defaults: HashMap<String, InputDefault>,
}

/// Serializable graph layout evaluated by the runtime and wasm hosts.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct GraphSpec {
    /// Ordered node declarations. Their order is preserved in plan caches and output snapshots.
    pub nodes: Vec<NodeSpec>,
    /// Directed edges connecting source outputs to target inputs.
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
    /// Upstream node id providing the value, or `None` when only an inline default is present.
    #[serde(default)]
    pub node_id: Option<NodeId>,
    /// Source output port name. Defaults to `"out"` for single-output nodes.
    #[serde(default = "default_output_key")]
    pub output_key: String,
    /// Optional selector applied after reading the source port.
    #[serde(default)]
    pub selector: Option<Selector>,
    /// Inline default value used when no source edge is wired.
    #[serde(rename = "default", default)]
    pub default_value: Option<Value>,
    /// Optional declared shape for [`Self::default_value`].
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
    /// Inline default value for an unconnected input port.
    #[serde(rename = "value")]
    pub value: Value,
    /// Optional declared shape paired with [`Self::value`].
    #[serde(default)]
    pub shape: Option<Shape>,
}

/// Source side of an edge.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EdgeOutputEndpoint {
    /// Node emitting the value.
    pub node_id: NodeId,
    /// Output port name. Defaults to `"out"` when omitted in JSON.
    #[serde(default = "default_output_key")]
    pub output: String,
}

/// Target side of an edge.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EdgeInputEndpoint {
    /// Node consuming the value.
    pub node_id: NodeId,
    /// Target input port name.
    pub input: String,
}

/// Directed connection between a source output and target input.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EdgeSpec {
    /// Source endpoint.
    pub from: EdgeOutputEndpoint,
    /// Target endpoint.
    pub to: EdgeInputEndpoint,
    /// Optional selector applied to the source value before assignment.
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

    /// Build the effective input wiring table for each node.
    ///
    /// Inline defaults are inserted first, then explicit edges replace those defaults on the same
    /// input. Returns an error when an edge references an unknown node or when multiple edges
    /// target the same node/input pair.
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
