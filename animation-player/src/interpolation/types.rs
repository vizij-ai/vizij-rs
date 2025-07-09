use crate::animation::TransitionVariant;
use serde::{Deserialize, Serialize};

/// Types of interpolation available
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum InterpolationType {
    Linear,
    Cubic,
    Step,
    Bezier,
    Spring,
    CatmullRom,
    Hermite,
    BSpline,
    Custom(u32), // Custom function ID
}

impl InterpolationType {
    /// Get the name of this interpolation type
    #[inline]
    pub fn name(&self) -> &'static str {
        match self {
            Self::Linear => "linear",
            Self::Cubic => "cubic",
            Self::Step => "step",
            Self::Bezier => "bezier",
            Self::Spring => "spring",
            Self::CatmullRom => "catmullrom",
            Self::Hermite => "hermite",
            Self::BSpline => "bspline",
            Self::Custom(_) => "custom",
        }
    }
}

impl From<&str> for InterpolationType {
    fn from(s: &str) -> Self {
        match s.to_lowercase().as_str() {
            "linear" => Self::Linear,
            "cubic" => Self::Cubic,
            "step" => Self::Step,
            "bezier" => Self::Bezier,
            "spring" => Self::Spring,
            "catmullrom" => Self::CatmullRom,
            "hermite" => Self::Hermite,
            "bspline" => Self::BSpline,
            _ => Self::Linear, // Default to linear for unknown types
        }
    }
}

impl From<TransitionVariant> for InterpolationType {
    #[inline]
    fn from(variant: TransitionVariant) -> Self {
        match variant {
            TransitionVariant::Linear => Self::Linear,
            TransitionVariant::Cubic => Self::Cubic,
            TransitionVariant::Bezier => Self::Bezier,
            TransitionVariant::Spring => Self::Spring,
            TransitionVariant::Constant
            | TransitionVariant::Step
            | TransitionVariant::StepStart
            | TransitionVariant::StepEnd
            | TransitionVariant::StepMiddle
            | TransitionVariant::StepAfter
            | TransitionVariant::StepBefore => Self::Step,
            TransitionVariant::Catmullrom => Self::CatmullRom,
            TransitionVariant::Hermite => Self::Hermite,
            TransitionVariant::BSpline => Self::BSpline,
            TransitionVariant::EaseIn
            | TransitionVariant::EaseOut
            | TransitionVariant::EaseInOut => Self::Cubic,
        }
    }
}

impl From<TransitionVariant> for String {
    #[inline]
    fn from(variant: TransitionVariant) -> Self {
        variant.name().to_string()
    }
}
