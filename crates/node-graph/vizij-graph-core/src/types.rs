use hashbrown::HashMap;
use serde::{Deserialize, Serialize};

pub type NodeId = String;

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

    // Robotics
    InverseKinematics,

    // Sinks (for external binding in hosts)
    Output,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(untagged)]
pub enum Value {
    Float(f64),
    Bool(bool),
    Vec3([f64; 3]),
    Vector(Vec<f64>),
}

impl Default for Value {
    fn default() -> Self {
        Value::Float(0.0)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct NodeParams {
    pub value: Option<Value>,
    pub frequency: Option<f64>,
    pub phase: Option<f64>,
    #[serde(default)]
    pub min: f64,
    #[serde(default)]
    pub max: f64,
    // Optional defaults for Vec3 constructor
    pub x: Option<f64>,
    pub y: Option<f64>,
    pub z: Option<f64>,
    // For Remap
    pub in_min: Option<f64>,
    pub in_max: Option<f64>,
    pub out_min: Option<f64>,
    pub out_max: Option<f64>,
    // For IK
    pub bone1: Option<f64>,
    pub bone2: Option<f64>,
    pub bone3: Option<f64>,
    // For Splitter
    pub index: Option<f64>,
    // For Split sizes (vector of sizes, floored to usize)
    pub sizes: Option<Vec<f64>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InputConnection {
    pub node_id: NodeId,
    #[serde(default = "default_output_key")]
    pub output_key: String,
}

fn default_output_key() -> String {
    "out".to_string()
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NodeSpec {
    pub id: NodeId,
    #[serde(rename = "type")]
    pub kind: NodeType,
    #[serde(default)]
    pub params: NodeParams,
    #[serde(default)]
    pub inputs: HashMap<String, InputConnection>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct GraphSpec {
    pub nodes: Vec<NodeSpec>,
}
