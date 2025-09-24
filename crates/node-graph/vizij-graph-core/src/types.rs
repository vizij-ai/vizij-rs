use hashbrown::HashMap;
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
    Case,

    // Ranges
    Clamp,
    Remap,

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

    // Animation blend support
    WeightedSumVector,
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
    // For Case routing nodes
    pub case_labels: Option<Vec<String>>,

    // Transition parameters
    pub stiffness: Option<f32>,
    pub damping: Option<f32>,
    pub mass: Option<f32>,
    pub half_life: Option<f32>,
    pub max_rate: Option<f32>,

    // Optional target typed path for Output nodes and sinks.
    // Example: "robot1/Arm/Joint3.translation"
    #[serde(default)]
    pub path: Option<TypedPath>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InputConnection {
    pub node_id: NodeId,
    #[serde(default = "default_output_key")]
    pub output_key: String,
    #[serde(default)]
    pub selector: Option<Selector>,
}

fn default_output_key() -> String {
    "out".to_string()
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
    pub inputs: HashMap<String, InputConnection>,
    #[serde(default)]
    pub output_shapes: HashMap<String, Shape>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct GraphSpec {
    pub nodes: Vec<NodeSpec>,
}
