use serde::{Deserialize, Serialize};

/// Parameters for the spring interpolation function
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SpringParams {
    pub mass: f32,
    pub stiffness: f32,
    pub damping: f32,
}

/// Parameters for the Bezier interpolation function
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct BezierParams {
    pub x1: f32,
    pub y1: f32,
    pub x2: f32,
    pub y2: f32,
}

/// Parameters for the step interpolation function
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct StepParams {
    /// The point at which the step occurs (0.0 for start, 0.5 for middle, 1.0 for end)
    pub point: f32,
}

/// Enum to hold the different interpolation parameter structs
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum InterpolationParams {
    Spring(SpringParams),
    Bezier(BezierParams),
    Step(StepParams),
}
