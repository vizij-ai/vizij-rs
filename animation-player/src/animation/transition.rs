use crate::animation::ids::KeypointId;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Defines the type of transition between keypoints
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum TransitionVariant {
    Linear,
    Bezier,
    Catmullrom,
    Hermite,
    Constant,
    StepStart,
    StepEnd,
    StepMiddle,
    StepAfter,
    StepBefore,
    Step,
    // Map to existing InterpolationType variants
    Cubic,
    Spring,
    EaseIn,
    EaseOut,
    EaseInOut,
    BSpline,
}

impl From<&str> for TransitionVariant {
    fn from(s: &str) -> Self {
        match s.to_lowercase().as_str() {
            "linear" => Self::Linear,
            "bezier" => Self::Bezier,
            "catmullrom" => Self::Catmullrom,
            "hermite" => Self::Hermite,
            "constant" => Self::Constant,
            "step_start" => Self::StepStart,
            "step_end" => Self::StepEnd,
            "step_middle" => Self::StepMiddle,
            "step" => Self::Step,
            "step_after" => Self::StepAfter,
            "step_before" => Self::StepBefore,
            "cubic" => Self::Cubic,
            "spring" => Self::Spring,
            "ease_in" => Self::EaseIn,
            "ease_out" => Self::EaseOut,
            "ease_in_out" => Self::EaseInOut,
            "b_spline" => Self::BSpline,
            _ => Self::Cubic, // Default to cubic for unknown types
        }
    }
}

impl TransitionVariant {
    /// Get the name of this transition variant
    #[inline]
    pub fn name(&self) -> &'static str {
        match self {
            Self::Linear => "linear",
            Self::Bezier => "bezier",
            Self::Catmullrom => "catmullrom",
            Self::Hermite => "hermite",
            Self::Constant => "constant",
            Self::StepStart => "step_start",
            Self::StepEnd => "step_end",
            Self::Step => "step",
            Self::StepMiddle => "step_middle",
            Self::StepAfter => "step_after",
            Self::StepBefore => "step_before",
            Self::Cubic => "cubic",
            Self::Spring => "spring",
            Self::EaseIn => "ease_in",
            Self::EaseOut => "ease_out",
            Self::EaseInOut => "ease_in_out",
            Self::BSpline => "b_spline",
        }
    }
}

/// Represents a transition between two keypoints
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AnimationTransition {
    /// Unique identifier for this transition
    pub id: String,
    /// Pair of keypoint IDs that this transition connects
    pub keypoints: [KeypointId; 2],
    /// The type of transition/interpolation to use
    pub variant: TransitionVariant,
    /// Additional parameters for the transition (e.g., bezier control points, spring constants)
    pub parameters: HashMap<String, String>,
}

impl AnimationTransition {
    /// Create a new transition between two keypoints
    #[inline]
    pub fn new(
        from_keypoint: KeypointId,
        to_keypoint: KeypointId,
        variant: TransitionVariant,
    ) -> Self {
        use uuid::Uuid;
        Self {
            id: Uuid::new_v4().to_string(),
            keypoints: [from_keypoint, to_keypoint],
            variant,
            parameters: HashMap::new(),
        }
    }

    /// Create a new transition with specified ID
    #[inline]
    pub fn with_id(
        id: impl Into<String>,
        from_keypoint: KeypointId,
        to_keypoint: KeypointId,
        variant: TransitionVariant,
    ) -> Self {
        Self {
            id: id.into(),
            keypoints: [from_keypoint, to_keypoint],
            variant,
            parameters: HashMap::new(),
        }
    }

    /// Add a parameter to this transition
    #[inline]
    pub fn with_parameter(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.parameters.insert(key.into(), value.into());
        self
    }

    /// Get a parameter value
    #[inline]
    pub fn get_parameter(&self, key: &str) -> Option<&str> {
        self.parameters.get(key).map(|s| s.as_str())
    }

    /// Get the from keypoint ID
    #[inline]
    pub fn from_keypoint(&self) -> KeypointId {
        self.keypoints[0]
    }

    /// Get the to keypoint ID
    #[inline]
    pub fn to_keypoint(&self) -> KeypointId {
        self.keypoints[1]
    }
}
