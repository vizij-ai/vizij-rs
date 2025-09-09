use serde::{Deserialize, Serialize};

pub type NodeId = String;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
#[serde(rename_all = "lowercase")]
pub enum NodeType {
    // Scalars / arithmetic
    Constant,
    Slider,
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

    // Vec3 utilities
    Vec3,           // make vec3(x,y,z) from inputs/params
    Vec3Split,      // split to x,y,z
    Vec3Add,
    Vec3Subtract,
    Vec3Multiply,   // component-wise
    Vec3Scale,      // scalar * vec3
    Vec3Normalize,
    Vec3Dot,
    Vec3Cross,
    Vec3Length,

    // Robotics
    InverseKinematics,

    // Sinks (for external binding in hosts)
    Output,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Copy)]
#[serde(untagged)]
pub enum Value {
    Float(f64),
    Bool(bool),
    Vec3([f64; 3]),
}

impl Default for Value {
    fn default() -> Self { Value::Float(0.0) }
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
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NodeSpec {
    pub id: NodeId,
    #[serde(rename = "type")]
    pub kind: NodeType,
    #[serde(default)]
    pub params: NodeParams,
    #[serde(default)]
    pub inputs: Vec<NodeId>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct GraphSpec {
    pub nodes: Vec<NodeSpec>,
}
