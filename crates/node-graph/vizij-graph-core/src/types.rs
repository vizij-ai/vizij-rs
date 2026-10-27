//! Canonical graph specification types shared across all Vizij graph hosts.
//!
//! These structs are the serde-facing representation consumed by the evaluator, wasm bridge,
//! fixture tooling, and orchestration layer.

use hashbrown::{HashMap, HashSet};
use serde::{Deserialize, Serialize};
use vizij_api_core::{Shape, TypedPath, Value};

pub use crate::kind::Kind;
pub use crate::select::{Selector, SelectorSegment};
use uuid::Uuid;

/// Stable node identifier within a single [`GraphSpec`].
pub type NodeId = String;

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

    // External function invocation
    /// Invokes an external function (resolved by opaque id) through the host-provided
    /// [`NodeFunctions`](crate::eval::NodeFunctions) interface.
    ExternalFunction,
    /// Hosts one task run: invokes a module function each evaluation and emits
    /// the run's behavior [`Status`](crate::task) value on `out`, latched once
    /// terminal.
    ///
    /// Unlike [`ExternalFunction`](Self::ExternalFunction), the call is fully
    /// self-contained in the params — [`module`](NodeParams::module),
    /// [`function`](NodeParams::function), and the argument bundle in
    /// [`value`](NodeParams::value) — so a run grafts into a live graph without
    /// argument-feeder wiring.
    TaskRun,
    /// Spawns a task run of [`function`](NodeParams::function) each time its
    /// arguments change: the keyed variadic `args` inputs (slot `i` carries
    /// the parameter whose id is `record_keys[i]`) over the
    /// [`value`](NodeParams::value) bundle's fields. The first evaluation
    /// sets the baseline and spawns nothing, so a graph spawns on a change it
    /// observes, never on being loaded.
    ///
    /// Where a [`TaskRun`](Self::TaskRun) node *is* one run's call, this node
    /// starts runs: the request is queued on the runtime's
    /// [`spawns`](crate::tree_runtime::TreeRuntime::spawns) and served by the host
    /// once the evaluation is done. The graph keeps no handle — a run it
    /// spawns is the host's to end and clean up.
    Spawn,
}

impl NodeType {
    /// The kind of function a node of this type calls: `None` for an
    /// `input` or `output` node, which read and write the store.
    pub fn kind(&self) -> Option<Kind> {
        Some(match self {
            NodeType::Constant => Kind::Constant,
            NodeType::Slider => Kind::Slider,
            NodeType::MultiSlider => Kind::MultiSlider,
            NodeType::Add => Kind::Add,
            NodeType::Subtract => Kind::Subtract,
            NodeType::Multiply => Kind::Multiply,
            NodeType::Divide => Kind::Divide,
            NodeType::Power => Kind::Power,
            NodeType::Log => Kind::Log,
            NodeType::Abs => Kind::Abs,
            NodeType::Modulo => Kind::Modulo,
            NodeType::Sqrt => Kind::Sqrt,
            NodeType::Sign => Kind::Sign,
            NodeType::Min => Kind::Min,
            NodeType::Max => Kind::Max,
            NodeType::Round => Kind::Round,
            NodeType::Sin => Kind::Sin,
            NodeType::Cos => Kind::Cos,
            NodeType::Tan => Kind::Tan,
            NodeType::Time => Kind::Time,
            NodeType::Oscillator => Kind::Oscillator,
            NodeType::Spring => Kind::Spring,
            NodeType::Damp => Kind::Damp,
            NodeType::Slew => Kind::Slew,
            NodeType::And => Kind::And,
            NodeType::Or => Kind::Or,
            NodeType::Not => Kind::Not,
            NodeType::Xor => Kind::Xor,
            NodeType::GreaterThan => Kind::GreaterThan,
            NodeType::LessThan => Kind::LessThan,
            NodeType::Equal => Kind::Equal,
            NodeType::NotEqual => Kind::NotEqual,
            NodeType::If => Kind::If,
            NodeType::Case => Kind::Case,
            NodeType::Clamp => Kind::Clamp,
            NodeType::Remap => Kind::Remap,
            NodeType::CenteredRemap => Kind::CenteredRemap,
            NodeType::PiecewiseRemap => Kind::PiecewiseRemap,
            NodeType::Vec3Cross => Kind::Vec3Cross,
            NodeType::VectorConstant => Kind::VectorConstant,
            NodeType::VectorAdd => Kind::VectorAdd,
            NodeType::VectorSubtract => Kind::VectorSubtract,
            NodeType::VectorMultiply => Kind::VectorMultiply,
            NodeType::VectorScale => Kind::VectorScale,
            NodeType::VectorNormalize => Kind::VectorNormalize,
            NodeType::VectorDot => Kind::VectorDot,
            NodeType::VectorLength => Kind::VectorLength,
            NodeType::VectorIndex => Kind::VectorIndex,
            NodeType::Join => Kind::Join,
            NodeType::Split => Kind::Split,
            NodeType::VectorMin => Kind::VectorMin,
            NodeType::VectorMax => Kind::VectorMax,
            NodeType::VectorMean => Kind::VectorMean,
            NodeType::VectorMedian => Kind::VectorMedian,
            NodeType::VectorMode => Kind::VectorMode,
            NodeType::ToVector => Kind::ToVector,
            NodeType::FromVector => Kind::FromVector,
            NodeType::SimpleNoise => Kind::SimpleNoise,
            NodeType::PerlinNoise => Kind::PerlinNoise,
            NodeType::SimplexNoise => Kind::SimplexNoise,
            NodeType::WeightedSumVector => Kind::WeightedSumVector,
            NodeType::DefaultBlend => Kind::DefaultBlend,
            NodeType::BlendWeightedAverage => Kind::BlendWeightedAverage,
            NodeType::BlendAdditive => Kind::BlendAdditive,
            NodeType::BlendMultiply => Kind::BlendMultiply,
            NodeType::BlendWeightedOverlay => Kind::BlendWeightedOverlay,
            NodeType::BlendWeightedAverageOverlay => Kind::BlendWeightedAverageOverlay,
            NodeType::BlendMax => Kind::BlendMax,
            NodeType::InverseKinematics => Kind::InverseKinematics,
            NodeType::UrdfIkPosition => Kind::UrdfIkPosition,
            NodeType::UrdfIkPose => Kind::UrdfIkPose,
            NodeType::UrdfFk => Kind::UrdfFk,
            NodeType::BuildRecord => Kind::BuildRecord,
            NodeType::ReadRecord => Kind::ReadRecord,
            NodeType::SwitchRecord => Kind::SwitchRecord,
            NodeType::MergeRecord => Kind::MergeRecord,
            NodeType::SplitRecord => Kind::SplitRecord,
            NodeType::MathMultRecord => Kind::MathMultRecord,
            NodeType::MathAddRecord => Kind::MathAddRecord,
            NodeType::MathDivRecord => Kind::MathDivRecord,
            NodeType::MathSubRecord => Kind::MathSubRecord,
            NodeType::ExternalFunction => Kind::ExternalFunction,
            NodeType::TaskRun => Kind::TaskRun,
            NodeType::Spawn => Kind::Spawn,
            NodeType::Input | NodeType::Output => return None,
        })
    }
}

impl From<Kind> for NodeType {
    fn from(kind: Kind) -> Self {
        match kind {
            Kind::Constant => NodeType::Constant,
            Kind::Slider => NodeType::Slider,
            Kind::MultiSlider => NodeType::MultiSlider,
            Kind::Add => NodeType::Add,
            Kind::Subtract => NodeType::Subtract,
            Kind::Multiply => NodeType::Multiply,
            Kind::Divide => NodeType::Divide,
            Kind::Power => NodeType::Power,
            Kind::Log => NodeType::Log,
            Kind::Abs => NodeType::Abs,
            Kind::Modulo => NodeType::Modulo,
            Kind::Sqrt => NodeType::Sqrt,
            Kind::Sign => NodeType::Sign,
            Kind::Min => NodeType::Min,
            Kind::Max => NodeType::Max,
            Kind::Round => NodeType::Round,
            Kind::Sin => NodeType::Sin,
            Kind::Cos => NodeType::Cos,
            Kind::Tan => NodeType::Tan,
            Kind::Time => NodeType::Time,
            Kind::Oscillator => NodeType::Oscillator,
            Kind::Spring => NodeType::Spring,
            Kind::Damp => NodeType::Damp,
            Kind::Slew => NodeType::Slew,
            Kind::And => NodeType::And,
            Kind::Or => NodeType::Or,
            Kind::Not => NodeType::Not,
            Kind::Xor => NodeType::Xor,
            Kind::GreaterThan => NodeType::GreaterThan,
            Kind::LessThan => NodeType::LessThan,
            Kind::Equal => NodeType::Equal,
            Kind::NotEqual => NodeType::NotEqual,
            Kind::If => NodeType::If,
            Kind::Case => NodeType::Case,
            Kind::Clamp => NodeType::Clamp,
            Kind::Remap => NodeType::Remap,
            Kind::CenteredRemap => NodeType::CenteredRemap,
            Kind::PiecewiseRemap => NodeType::PiecewiseRemap,
            Kind::Vec3Cross => NodeType::Vec3Cross,
            Kind::VectorConstant => NodeType::VectorConstant,
            Kind::VectorAdd => NodeType::VectorAdd,
            Kind::VectorSubtract => NodeType::VectorSubtract,
            Kind::VectorMultiply => NodeType::VectorMultiply,
            Kind::VectorScale => NodeType::VectorScale,
            Kind::VectorNormalize => NodeType::VectorNormalize,
            Kind::VectorDot => NodeType::VectorDot,
            Kind::VectorLength => NodeType::VectorLength,
            Kind::VectorIndex => NodeType::VectorIndex,
            Kind::Join => NodeType::Join,
            Kind::Split => NodeType::Split,
            Kind::VectorMin => NodeType::VectorMin,
            Kind::VectorMax => NodeType::VectorMax,
            Kind::VectorMean => NodeType::VectorMean,
            Kind::VectorMedian => NodeType::VectorMedian,
            Kind::VectorMode => NodeType::VectorMode,
            Kind::ToVector => NodeType::ToVector,
            Kind::FromVector => NodeType::FromVector,
            Kind::SimpleNoise => NodeType::SimpleNoise,
            Kind::PerlinNoise => NodeType::PerlinNoise,
            Kind::SimplexNoise => NodeType::SimplexNoise,
            Kind::WeightedSumVector => NodeType::WeightedSumVector,
            Kind::DefaultBlend => NodeType::DefaultBlend,
            Kind::BlendWeightedAverage => NodeType::BlendWeightedAverage,
            Kind::BlendAdditive => NodeType::BlendAdditive,
            Kind::BlendMultiply => NodeType::BlendMultiply,
            Kind::BlendWeightedOverlay => NodeType::BlendWeightedOverlay,
            Kind::BlendWeightedAverageOverlay => NodeType::BlendWeightedAverageOverlay,
            Kind::BlendMax => NodeType::BlendMax,
            Kind::InverseKinematics => NodeType::InverseKinematics,
            Kind::UrdfIkPosition => NodeType::UrdfIkPosition,
            Kind::UrdfIkPose => NodeType::UrdfIkPose,
            Kind::UrdfFk => NodeType::UrdfFk,
            Kind::BuildRecord => NodeType::BuildRecord,
            Kind::ReadRecord => NodeType::ReadRecord,
            Kind::SwitchRecord => NodeType::SwitchRecord,
            Kind::MergeRecord => NodeType::MergeRecord,
            Kind::SplitRecord => NodeType::SplitRecord,
            Kind::MathMultRecord => NodeType::MathMultRecord,
            Kind::MathAddRecord => NodeType::MathAddRecord,
            Kind::MathDivRecord => NodeType::MathDivRecord,
            Kind::MathSubRecord => NodeType::MathSubRecord,
            Kind::ExternalFunction => NodeType::ExternalFunction,
            Kind::TaskRun => NodeType::TaskRun,
            Kind::Spawn => NodeType::Spawn,
        }
    }
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

    // For BuildRecord/ReadRecord/TaskRun – one key string per variadic slot, in
    // slot order (a TaskRun key is the id of the out-parameter the slot carries)
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

    /// Stable id of the node-function invoked by [`NodeType::ExternalFunction`].
    ///
    /// The graph attaches no meaning to this id beyond passing it to the host
    /// [`NodeFunctions`](crate::eval::NodeFunctions); the host resolves and invokes it.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub function: Option<Uuid>,
    /// Ordered arg-key handles matching the [`NodeType::ExternalFunction`] variadic `args` inputs.
    ///
    /// These are opaque to the graph; the host pairs each with the positional value in the call.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub param_ids: Option<Vec<Uuid>>,
    /// Record field carrying each entry's **target path** in the keyed batch a
    /// **path-less** [`NodeType::Output`] applies. With
    /// [`value_field`](Self::value_field), it turns the node's input — a batch
    /// of records naming their own keys (e.g. a module call's "what changed")
    /// — into one write per record. Ignored when [`path`](Self::path) is set.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub key_field: Option<Uuid>,
    /// Record field carrying each entry's value; see [`key_field`](Self::key_field).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub value_field: Option<Uuid>,
    /// Module the [`NodeType::TaskRun`] function is dispatched to, when the run
    /// names one. `None` leaves resolution to the host's function→module map,
    /// like an [`NodeType::ExternalFunction`] call.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub module: Option<Uuid>,
}

pub use crate::params::RoundMode;

impl NodeParams {
    /// The parameters the node functions read: all but the store path and
    /// the keyed-batch fields, which the importer and the tree read.
    pub fn params(&self) -> crate::params::Params {
        crate::params::Params {
            value: self.value.clone(),
            frequency: self.frequency,
            phase: self.phase,
            min: self.min,
            max: self.max,
            x: self.x,
            y: self.y,
            z: self.z,
            in_min: self.in_min,
            in_max: self.in_max,
            out_min: self.out_min,
            out_max: self.out_max,
            round_mode: self.round_mode.clone(),
            clamp: self.clamp,
            in_low: self.in_low,
            in_anchor: self.in_anchor,
            in_high: self.in_high,
            out_low: self.out_low,
            out_anchor: self.out_anchor,
            out_high: self.out_high,
            bone1: self.bone1,
            bone2: self.bone2,
            bone3: self.bone3,
            urdf_xml: self.urdf_xml.clone(),
            root_link: self.root_link.clone(),
            tip_link: self.tip_link.clone(),
            seed: self.seed.clone(),
            weights: self.weights.clone(),
            max_iters: self.max_iters,
            tol_pos: self.tol_pos,
            tol_rot: self.tol_rot,
            joint_defaults: self.joint_defaults.clone(),
            index: self.index,
            sizes: self.sizes.clone(),
            noise_seed: self.noise_seed,
            octaves: self.octaves,
            lacunarity: self.lacunarity,
            persistence: self.persistence,
            stiffness: self.stiffness,
            damping: self.damping,
            mass: self.mass,
            half_life: self.half_life,
            max_rate: self.max_rate,
            case_labels: self.case_labels.clone(),
            record_keys: self.record_keys.clone(),
            keys: self.keys.clone(),
            function: self.function,
            param_ids: self.param_ids.clone(),
            module: self.module,
        }
    }
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
    /// Caller-managed version of the spec: a host keying specs by version
    /// loads one of a new version again.
    #[serde(default, skip_serializing_if = "crate::types::is_zero")]
    pub version: u64,
    /// Not read by the runtime. It stays in the format, so documents that
    /// carry it still parse.
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
    /// Seed or bump the spec version, so a host keying specs by version
    /// loads it again.
    ///
    /// - If `version` is zero, it is set to 1.
    /// - Otherwise, the version is saturated by `saturating_add(1)` to avoid wrap.
    pub fn with_cache(mut self) -> Self {
        self.version = if self.version == 0 {
            1
        } else {
            self.version.saturating_add(1)
        };
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

            // The keys are copied only for an input no default or edge has
            // reached yet.
            let connection_entry = map
                .entry_ref(edge.to.node_id.as_str())
                .or_default()
                .entry_ref(edge.to.input.as_str())
                .or_default();
            // An edge sets the source an inline default leaves unset: one set
            // already is a second edge into the input.
            if connection_entry.node_id.is_some() {
                return Err(format!(
                    "duplicate edge for input '{}:{}'",
                    edge.to.node_id, edge.to.input
                ));
            }

            connection_entry.node_id = Some(edge.from.node_id.clone());
            connection_entry.output_key = edge.from.output.clone();
            connection_entry.selector = edge.selector.clone();
        }

        Ok(map)
    }

    /// Whether this graph can be evaluated: the checks whose failure would
    /// fail every evaluation of it, made before it is installed rather than at
    /// its first evaluation.
    ///
    /// It refuses a graph where:
    /// - an edge names a node the graph lacks, or two edges feed one input;
    /// - two nodes share an id, or the edges form a cycle;
    /// - a node leaves one of its [required inputs](crate::schema::required_inputs)
    ///   with neither an edge nor an inline default.
    ///
    /// The error names the node and, for a required input, the port. What a
    /// graph needs only at evaluation time, such as a value staged on an input
    /// path, is not checked: it can arrive before the graph next evaluates.
    pub fn validate(&self) -> Result<(), String> {
        let connections = self.input_connections()?;
        crate::topo::topo_order(&self.nodes, &self.edges)?;
        for node in &self.nodes {
            let wired = connections.get(&node.id);
            for port in crate::schema::required_inputs(&node.kind) {
                let satisfied = wired
                    .and_then(|inputs| inputs.get(*port))
                    .is_some_and(|input| input.node_id.is_some() || input.default_value.is_some());
                if !satisfied {
                    return Err(format!(
                        "node '{}' ({:?}) needs its input '{port}': it has no edge and no default",
                        node.id, node.kind,
                    ));
                }
            }
        }
        Ok(())
    }
}
