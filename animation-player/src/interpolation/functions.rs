use crate::animation::data::AnimationData;
use crate::interpolation::context::InterpolationContext;
use crate::interpolation::parameters::InterpolationParams;
use crate::interpolation::schema::{InterpolationParameterSchema, ParameterDefinition};
use crate::interpolation::spline_helpers::{bezier_curve, catmull_rom_spline, hermite_spline};
use crate::interpolation::types::InterpolationType;
use crate::value::{Value, ValueType};
use crate::AnimationError;

use std::collections::HashMap;

/// Trait for interpolation functions
pub trait Interpolator: Send + Sync {
    /// Get the name of this interpolation function
    fn name(&self) -> &str;

    /// Interpolate between two values
    fn interpolate(
        &self,
        start: &Value,
        end: &Value,
        context: &InterpolationContext,
        animation: &AnimationData,
    ) -> Result<Value, AnimationError>;

    /// Get the interpolation type
    fn interpolation_type(&self) -> InterpolationType;

    /// Validate that this function can interpolate between the given value types
    #[inline]
    fn can_interpolate(&self, start: &Value, end: &Value) -> bool {
        start.can_interpolate_with(end)
    }

    /// Get the parameter schema for this interpolation function
    fn parameter_schema(&self) -> InterpolationParameterSchema;
}

/// Linear interpolation function
#[derive(Debug, Clone)]
pub struct LinearInterpolation;

impl Interpolator for LinearInterpolation {
    fn name(&self) -> &str {
        "linear"
    }

    fn interpolation_type(&self) -> InterpolationType {
        InterpolationType::Linear
    }

    fn interpolate(
        &self,
        start: &Value,
        end: &Value,
        context: &InterpolationContext,
        _animation: &AnimationData,
    ) -> Result<Value, AnimationError> {
        if !self.can_interpolate(start, end) {
            return Err(AnimationError::InterpolationError {
                reason: format!(
                    "Cannot interpolate between {:?} and {:?}",
                    start.value_type().name(),
                    end.value_type().name()
                ),
            });
        }

        let start_components = start.interpolatable_components();
        let end_components = end.interpolatable_components();

        if start_components.len() != end_components.len() {
            return Err(AnimationError::InterpolationError {
                reason: "Component count mismatch".to_string(),
            });
        }

        if let (Value::Transform(start_transform), Value::Transform(end_transform)) = (start, end) {
            // Interpolate position and scale linearly
            let interpolated_position: Vec<f64> = vec![
                start_transform.position.x
                    + (end_transform.position.x - start_transform.position.x) * context.t,
                start_transform.position.y
                    + (end_transform.position.y - start_transform.position.y) * context.t,
                start_transform.position.z
                    + (end_transform.position.z - start_transform.position.z) * context.t,
            ];
            let interpolated_scale: Vec<f64> = vec![
                start_transform.scale.x
                    + (end_transform.scale.x - start_transform.scale.x) * context.t,
                start_transform.scale.y
                    + (end_transform.scale.y - start_transform.scale.y) * context.t,
                start_transform.scale.z
                    + (end_transform.scale.z - start_transform.scale.z) * context.t,
            ];

            // Interpolate rotation using SLERP
            let start_rotation_arr: [f64; 4] = [
                start_transform.rotation.x,
                start_transform.rotation.y,
                start_transform.rotation.z,
                start_transform.rotation.w,
            ];
            let end_rotation_arr: [f64; 4] = [
                end_transform.rotation.x,
                end_transform.rotation.y,
                end_transform.rotation.z,
                end_transform.rotation.w,
            ];
            let interpolated_rotation_arr = crate::value::transform::slerp_quaternion(
                &start_rotation_arr,
                &end_rotation_arr,
                context.t,
            );

            // Combine all components
            let mut interpolated_components = Vec::new();
            interpolated_components.extend_from_slice(&interpolated_position);
            interpolated_components.extend_from_slice(&interpolated_rotation_arr);
            interpolated_components.extend_from_slice(&interpolated_scale);

            Value::from_components(start.value_type(), &interpolated_components)
        } else {
            // Fallback to generic linear interpolation for other types
            let interpolated: Vec<f64> = start_components
                .iter()
                .zip(end_components.iter())
                .map(|(s, e)| s + (e - s) * context.t)
                .collect();

            Value::from_components(start.value_type(), &interpolated)
        }
    }

    fn parameter_schema(&self) -> InterpolationParameterSchema {
        InterpolationParameterSchema {
            parameters: HashMap::new(),
        }
    }
}

/// Cubic interpolation function (smooth acceleration and deceleration)
#[derive(Debug, Clone)]
pub struct CubicInterpolation;

impl Interpolator for CubicInterpolation {
    fn name(&self) -> &str {
        "cubic"
    }

    fn interpolation_type(&self) -> InterpolationType {
        InterpolationType::Cubic
    }

    fn interpolate(
        &self,
        start: &Value,
        end: &Value,
        context: &InterpolationContext,
        _animation: &AnimationData,
    ) -> Result<Value, AnimationError> {
        if !self.can_interpolate(start, end) {
            return Err(AnimationError::InterpolationError {
                reason: format!(
                    "Cannot interpolate between {:?} and {:?}",
                    start.value_type().name(),
                    end.value_type().name()
                ),
            });
        }

        // Cubic easing: t^3 * (3 - 2*t)
        let cubic_t = context.t * context.t * (3.0 - 2.0 * context.t);

        let start_components = start.interpolatable_components();
        let end_components = end.interpolatable_components();

        if start_components.len() != end_components.len() {
            return Err(AnimationError::InterpolationError {
                reason: "Component count mismatch".to_string(),
            });
        }

        let interpolated: Vec<f64> = start_components
            .iter()
            .zip(end_components.iter())
            .map(|(s, e)| s + (e - s) * cubic_t)
            .collect();

        Value::from_components(start.value_type(), &interpolated)
    }

    fn parameter_schema(&self) -> InterpolationParameterSchema {
        InterpolationParameterSchema {
            parameters: HashMap::new(),
        }
    }
}

/// Step interpolation function (no interpolation, jump to end value at t=1)
#[derive(Debug, Clone)]
pub struct StepInterpolation;

impl Interpolator for StepInterpolation {
    fn name(&self) -> &str {
        "step"
    }

    fn interpolation_type(&self) -> InterpolationType {
        InterpolationType::Step
    }

    fn interpolate(
        &self,
        start: &Value,
        end: &Value,
        context: &InterpolationContext,
        animation: &AnimationData,
    ) -> Result<Value, AnimationError> {
        if !self.can_interpolate(start, end) {
            return Err(AnimationError::InterpolationError {
                reason: format!(
                    "Cannot interpolate between {:?} and {:?}",
                    start.value_type().name(),
                    end.value_type().name()
                ),
            });
        }

        // Step threshold - when to jump to end value
        let threshold = context.get_property("threshold").unwrap_or_else(|| {
            if let Some(params) = animation
                .default_interpolation
                .get(&self.interpolation_type())
            {
                if let InterpolationParams::Step(step_params) = params {
                    return step_params.point as f64;
                }
            }
            1.0
        });

        if context.t >= threshold {
            Ok(end.clone())
        } else {
            Ok(start.clone())
        }
    }

    fn parameter_schema(&self) -> InterpolationParameterSchema {
        let mut params = HashMap::new();
        params.insert(
            "threshold".to_string(),
            ParameterDefinition {
                name: "threshold".to_string(),
                value_type: ValueType::Float,
                default_value: Some(Value::Float(1.0)),
                min_value: Some(Value::Float(0.0)),
                max_value: Some(Value::Float(1.0)),
                description: "The point (0.0-1.0) at which the value snaps from start to end."
                    .to_string(),
            },
        );
        InterpolationParameterSchema { parameters: params }
    }
}

/// Bezier interpolation function (cubic bezier curve)
#[derive(Debug, Clone)]
pub struct BezierInterpolation {
    control_points: [f64; 4], // x1, y1, x2, y2
}

impl BezierInterpolation {
    pub fn new() -> Self {
        Self {
            control_points: [0.25, 0.1, 0.25, 1.0], // Default ease
        }
    }

    pub fn with_control_points(p1: (f64, f64), p2: (f64, f64)) -> Self {
        Self {
            control_points: [p1.0, p1.1, p2.0, p2.1],
        }
    }
}

impl Interpolator for BezierInterpolation {
    fn name(&self) -> &str {
        "bezier"
    }

    fn interpolation_type(&self) -> InterpolationType {
        InterpolationType::Bezier
    }

    fn interpolate(
        &self,
        start: &Value,
        end: &Value,
        context: &InterpolationContext,
        animation: &AnimationData,
    ) -> Result<Value, AnimationError> {
        if !self.can_interpolate(start, end) {
            return Err(AnimationError::InterpolationError {
                reason: format!(
                    "Cannot interpolate between {:?} and {:?}",
                    start.value_type().name(),
                    end.value_type().name()
                ),
            });
        }

        let control_points = if let (Some(x1), Some(y1), Some(x2), Some(y2)) = (
            context.get_property("x1"),
            context.get_property("y1"),
            context.get_property("x2"),
            context.get_property("y2"),
        ) {
            [x1, y1, x2, y2]
        } else if let Some(params) = animation
            .default_interpolation
            .get(&self.interpolation_type())
        {
            if let InterpolationParams::Bezier(bezier_params) = params {
                [
                    bezier_params.x1 as f64,
                    bezier_params.y1 as f64,
                    bezier_params.x2 as f64,
                    bezier_params.y2 as f64,
                ]
            } else {
                self.control_points
            }
        } else {
            self.control_points
        };

        let eased_t = cubic_bezier_easing(context.t.clamp(0.0, 1.0), &control_points);

        let start_components = start.interpolatable_components();
        let end_components = end.interpolatable_components();

        if start_components.len() != end_components.len() {
            return Err(AnimationError::InterpolationError {
                reason: "Component count mismatch".to_string(),
            });
        }

        let interpolated: Vec<f64> = start_components
            .iter()
            .zip(end_components.iter())
            .map(|(s, e)| s + (e - s) * eased_t)
            .collect();

        Value::from_components(start.value_type(), &interpolated)
    }

    fn parameter_schema(&self) -> InterpolationParameterSchema {
        let mut params = HashMap::new();
        params.insert(
            "x1".to_string(),
            ParameterDefinition {
                name: "x1".to_string(),
                value_type: ValueType::Float,
                default_value: Some(Value::Float(0.25)),
                min_value: Some(Value::Float(0.0)),
                max_value: Some(Value::Float(1.0)),
                description: "First control point X coordinate".to_string(),
            },
        );
        params.insert(
            "y1".to_string(),
            ParameterDefinition {
                name: "y1".to_string(),
                value_type: ValueType::Float,
                default_value: Some(Value::Float(0.1)),
                min_value: None,
                max_value: None,
                description: "First control point Y coordinate".to_string(),
            },
        );
        params.insert(
            "x2".to_string(),
            ParameterDefinition {
                name: "x2".to_string(),
                value_type: ValueType::Float,
                default_value: Some(Value::Float(0.25)),
                min_value: Some(Value::Float(0.0)),
                max_value: Some(Value::Float(1.0)),
                description: "Second control point X coordinate".to_string(),
            },
        );
        params.insert(
            "y2".to_string(),
            ParameterDefinition {
                name: "y2".to_string(),
                value_type: ValueType::Float,
                default_value: Some(Value::Float(1.0)),
                min_value: None,
                max_value: None,
                description: "Second control point Y coordinate".to_string(),
            },
        );

        InterpolationParameterSchema { parameters: params }
    }
}

fn cubic_bezier_easing(t: f64, control_points: &[f64; 4]) -> f64 {
    let [x1, y1, x2, y2] = *control_points;

    // Binary search for the correct t value
    let mut lower = 0.0;
    let mut upper = 1.0;
    let mut current_t = t;

    for _ in 0..10 {
        // 10 iterations should be sufficient
        let current_x = bezier_curve(0.0, x1, x2, 1.0, current_t) as f64;

        if (current_x - t).abs() < 0.001 {
            break;
        }

        if current_x < t {
            lower = current_t;
        } else {
            upper = current_t;
        }

        current_t = (lower + upper) / 2.0;
    }

    bezier_curve(0.0, y1, y2, 1.0, current_t) as f64
}

/// Spring interpolation function (bouncy/elastic effect)
#[derive(Debug, Clone)]
pub struct SpringInterpolation {
    damping: f64,
    stiffness: f64,
}

impl SpringInterpolation {
    pub fn new() -> Self {
        Self {
            damping: 20.0,
            stiffness: 100.0,
        }
    }

    pub fn with_params(damping: f64, stiffness: f64) -> Self {
        Self { damping, stiffness }
    }
}

impl Interpolator for SpringInterpolation {
    fn name(&self) -> &str {
        "spring"
    }

    fn interpolation_type(&self) -> InterpolationType {
        InterpolationType::Spring
    }

    fn interpolate(
        &self,
        start: &Value,
        end: &Value,
        context: &InterpolationContext,
        animation: &AnimationData,
    ) -> Result<Value, AnimationError> {
        if !self.can_interpolate(start, end) {
            return Err(AnimationError::InterpolationError {
                reason: format!(
                    "Cannot interpolate between {:?} and {:?}",
                    start.value_type().name(),
                    end.value_type().name()
                ),
            });
        }

        let damping = context.get_property("damping").unwrap_or_else(|| {
            if let Some(params) = animation
                .default_interpolation
                .get(&self.interpolation_type())
            {
                if let InterpolationParams::Spring(spring_params) = params {
                    return spring_params.damping as f64;
                }
            }
            self.damping
        });

        let stiffness = context.get_property("stiffness").unwrap_or_else(|| {
            if let Some(params) = animation
                .default_interpolation
                .get(&self.interpolation_type())
            {
                if let InterpolationParams::Spring(spring_params) = params {
                    return spring_params.stiffness as f64;
                }
            }
            self.stiffness
        });

        let spring_t = spring_ease(context.t.clamp(0.0, 1.0), damping, stiffness);

        let start_components = start.interpolatable_components();
        let end_components = end.interpolatable_components();

        if start_components.len() != end_components.len() {
            return Err(AnimationError::InterpolationError {
                reason: "Component count mismatch".to_string(),
            });
        }

        let interpolated: Vec<f64> = start_components
            .iter()
            .zip(end_components.iter())
            .map(|(s, e)| s + (e - s) * spring_t)
            .collect();

        Value::from_components(start.value_type(), &interpolated)
    }

    fn parameter_schema(&self) -> InterpolationParameterSchema {
        let mut params = HashMap::new();
        params.insert(
            "damping".to_string(),
            ParameterDefinition {
                name: "damping".to_string(),
                value_type: ValueType::Float,
                default_value: Some(Value::Float(0.8)),
                min_value: Some(Value::Float(0.0)),
                max_value: Some(Value::Float(2.0)),
                description: "Damping coefficient for the spring (0.0-2.0)".to_string(),
            },
        );
        params.insert(
            "stiffness".to_string(),
            ParameterDefinition {
                name: "stiffness".to_string(),
                value_type: ValueType::Float,
                default_value: Some(Value::Float(100.0)),
                min_value: Some(Value::Float(1.0)),
                max_value: Some(Value::Float(1000.0)),
                description: "Spring stiffness".to_string(),
            },
        );

        InterpolationParameterSchema { parameters: params }
    }
}

fn spring_ease(t: f64, damping: f64, stiffness: f64) -> f64 {
    if t == 0.0 || t == 1.0 {
        return t;
    }

    let m = 1.0; // mass
    let c = damping;
    let k = stiffness;

    // Calculate natural frequency and damping ratio
    let w0 = (k / m).sqrt();
    let zeta = c / (2.0 * (k * m).sqrt());

    if zeta < 1.0 {
        // Underdamped
        let wd = w0 * (1.0 - zeta * zeta).sqrt();
        1.0 - ((-zeta * w0 * t).exp() * (wd * t).cos())
    } else if zeta == 1.0 {
        // Critically damped
        1.0 - ((-w0 * t).exp() * (1.0 + w0 * t))
    } else {
        // Overdamped
        let r1 = w0 * (-zeta + (zeta * zeta - 1.0).sqrt());
        let r2 = w0 * (-zeta - (zeta * zeta - 1.0).sqrt());
        let c1 = 1.0;
        let c2 = -1.0;
        1.0 - (c1 * (r1 * t).exp() + c2 * (r2 * t).exp())
    }
}

/// Ease-in interpolation function (quadratic acceleration)
#[derive(Debug, Clone)]
pub struct EaseInInterpolation;

impl Interpolator for EaseInInterpolation {
    fn name(&self) -> &str {
        "ease_in"
    }

    fn interpolation_type(&self) -> InterpolationType {
        InterpolationType::Cubic
    }

    fn interpolate(
        &self,
        start: &Value,
        end: &Value,
        context: &InterpolationContext,
        _animation: &AnimationData,
    ) -> Result<Value, AnimationError> {
        if !self.can_interpolate(start, end) {
            return Err(AnimationError::InterpolationError {
                reason: format!(
                    "Cannot interpolate between {:?} and {:?}",
                    start.value_type().name(),
                    end.value_type().name()
                ),
            });
        }

        // Ease-in: t^2
        let eased_t = context.t * context.t;

        let start_components = start.interpolatable_components();
        let end_components = end.interpolatable_components();

        if start_components.len() != end_components.len() {
            return Err(AnimationError::InterpolationError {
                reason: "Component count mismatch".to_string(),
            });
        }

        let interpolated: Vec<f64> = start_components
            .iter()
            .zip(end_components.iter())
            .map(|(s, e)| s + (e - s) * eased_t)
            .collect();

        Value::from_components(start.value_type(), &interpolated)
    }

    fn parameter_schema(&self) -> InterpolationParameterSchema {
        InterpolationParameterSchema {
            parameters: HashMap::new(),
        }
    }
}

/// Ease-out interpolation function (quadratic deceleration)
#[derive(Debug, Clone)]
pub struct EaseOutInterpolation;

impl Interpolator for EaseOutInterpolation {
    fn name(&self) -> &str {
        "ease_out"
    }

    fn interpolation_type(&self) -> InterpolationType {
        InterpolationType::Cubic
    }

    fn interpolate(
        &self,
        start: &Value,
        end: &Value,
        context: &InterpolationContext,
        _animation: &AnimationData,
    ) -> Result<Value, AnimationError> {
        if !self.can_interpolate(start, end) {
            return Err(AnimationError::InterpolationError {
                reason: format!(
                    "Cannot interpolate between {:?} and {:?}",
                    start.value_type().name(),
                    end.value_type().name()
                ),
            });
        }

        // Ease-out: 1 - (1 - t)^2
        let eased_t = 1.0 - (1.0 - context.t) * (1.0 - context.t);

        let start_components = start.interpolatable_components();
        let end_components = end.interpolatable_components();

        if start_components.len() != end_components.len() {
            return Err(AnimationError::InterpolationError {
                reason: "Component count mismatch".to_string(),
            });
        }

        let interpolated: Vec<f64> = start_components
            .iter()
            .zip(end_components.iter())
            .map(|(s, e)| s + (e - s) * eased_t)
            .collect();

        Value::from_components(start.value_type(), &interpolated)
    }

    fn parameter_schema(&self) -> InterpolationParameterSchema {
        InterpolationParameterSchema {
            parameters: HashMap::new(),
        }
    }
}

/// Ease-in-out interpolation function (quadratic acceleration then deceleration)
#[derive(Debug, Clone)]
pub struct EaseInOutInterpolation;

impl Interpolator for EaseInOutInterpolation {
    fn name(&self) -> &str {
        "ease_in_out"
    }

    fn interpolation_type(&self) -> InterpolationType {
        InterpolationType::Cubic
    }

    fn interpolate(
        &self,
        start: &Value,
        end: &Value,
        context: &InterpolationContext,
        _animation: &AnimationData,
    ) -> Result<Value, AnimationError> {
        if !self.can_interpolate(start, end) {
            return Err(AnimationError::InterpolationError {
                reason: format!(
                    "Cannot interpolate between {:?} and {:?}",
                    start.value_type().name(),
                    end.value_type().name()
                ),
            });
        }

        // Ease-in-out: first half accelerates, second half decelerates
        let eased_t = if context.t < 0.5 {
            2.0 * context.t * context.t
        } else {
            1.0 - 2.0 * (1.0 - context.t) * (1.0 - context.t)
        };

        let start_components = start.interpolatable_components();
        let end_components = end.interpolatable_components();

        if start_components.len() != end_components.len() {
            return Err(AnimationError::InterpolationError {
                reason: "Component count mismatch".to_string(),
            });
        }

        let interpolated: Vec<f64> = start_components
            .iter()
            .zip(end_components.iter())
            .map(|(s, e)| s + (e - s) * eased_t)
            .collect();

        Value::from_components(start.value_type(), &interpolated)
    }

    fn parameter_schema(&self) -> InterpolationParameterSchema {
        InterpolationParameterSchema {
            parameters: HashMap::new(),
        }
    }
}

/// Hermite interpolation function
#[derive(Debug, Clone)]
pub struct HermiteInterpolation;

impl Interpolator for HermiteInterpolation {
    fn name(&self) -> &str {
        "hermite"
    }

    fn interpolation_type(&self) -> InterpolationType {
        InterpolationType::Hermite
    }

    fn interpolate(
        &self,
        start: &Value,
        end: &Value,
        context: &InterpolationContext,
        _animation: &AnimationData,
    ) -> Result<Value, AnimationError> {
        if !self.can_interpolate(start, end) {
            return Err(AnimationError::InterpolationError {
                reason: format!(
                    "Cannot interpolate between {:?} and {:?}",
                    start.value_type().name(),
                    end.value_type().name()
                ),
            });
        }

        let p0 = start;
        let p1 = end;

        // Get tangents from context or calculate them by finite difference
        let m0_val = context.get_property("tangent_start").unwrap_or_else(|| {
            let p0_components = p0.interpolatable_components();
            let p1_components = p1.interpolatable_components();
            let m0_components: Vec<f64> = p0_components
                .iter()
                .zip(p1_components.iter())
                .map(|(c0, c1)| (c1 - c0) * 0.5)
                .collect();
            Value::from_components(p0.value_type(), &m0_components)
                .unwrap_or_else(|_| Value::Float(0.0))
        });
        let m1_val = context.get_property("tangent_end").unwrap_or_else(|| {
            let p0_components = p0.interpolatable_components();
            let p1_components = p1.interpolatable_components();
            let m1_components: Vec<f64> = p0_components
                .iter()
                .zip(p1_components.iter())
                .map(|(c0, c1)| (c1 - c0) * 0.5)
                .collect();
            Value::from_components(p0.value_type(), &m1_components)
                .unwrap_or_else(|_| Value::Float(0.0))
        });

        let p0_components = p0.interpolatable_components();
        let p1_components = p1.interpolatable_components();
        let m0_components = m0_val.interpolatable_components();
        let m1_components = m1_val.interpolatable_components();

        if !(p0_components.len() == p1_components.len()
            && p0_components.len() == m0_components.len()
            && p0_components.len() == m1_components.len())
        {
            return Err(AnimationError::InterpolationError {
                reason: "Component count mismatch for Hermite interpolation".to_string(),
            });
        }

        let interpolated_components: Vec<f64> = (0..p0_components.len())
            .map(|i| {
                hermite_spline(
                    p0_components[i],
                    p1_components[i],
                    m0_components[i],
                    m1_components[i],
                    context.t,
                )
            })
            .collect();

        Value::from_components(start.value_type(), &interpolated_components)
    }

    fn parameter_schema(&self) -> InterpolationParameterSchema {
        let mut params = HashMap::new();
        params.insert(
            "tangent_start".to_string(),
            ParameterDefinition {
                name: "tangent_start".to_string(),
                value_type: ValueType::Float, // Placeholder, actual type depends on value
                default_value: None,
                min_value: None,
                max_value: None,
                description: "The tangent vector at the start point.".to_string(),
            },
        );
        params.insert(
            "tangent_end".to_string(),
            ParameterDefinition {
                name: "tangent_end".to_string(),
                value_type: ValueType::Float, // Placeholder, actual type depends on value
                default_value: None,
                min_value: None,
                max_value: None,
                description: "The tangent vector at the end point.".to_string(),
            },
        );
        InterpolationParameterSchema { parameters: params }
    }
}

/// Catmull-Rom interpolation function
#[derive(Debug, Clone)]
pub struct CatmullRomInterpolation;

impl Interpolator for CatmullRomInterpolation {
    fn name(&self) -> &str {
        "catmullrom"
    }

    fn interpolation_type(&self) -> InterpolationType {
        InterpolationType::CatmullRom
    }

    fn interpolate(
        &self,
        start: &Value, // p1
        end: &Value,   // p2
        context: &InterpolationContext,
        _animation: &AnimationData,
    ) -> Result<Value, AnimationError> {
        if !self.can_interpolate(start, end) {
            return Err(AnimationError::InterpolationError {
                reason: format!(
                    "Cannot interpolate between {:?} and {:?}",
                    start.value_type().name(),
                    end.value_type().name()
                ),
            });
        }

        // Get the surrounding points from the context
        let p0 = context.get_point(-1).unwrap_or_else(|| start.clone());
        let p3 = context.get_point(2).unwrap_or_else(|| end.clone());

        // Handle Transform separately for SLERP on rotation
        if let (Value::Transform(p1_transform), Value::Transform(p2_transform)) = (start, end) {
            let p0_transform = p0.as_transform().unwrap_or(p1_transform); // Fallback
            let p3_transform = p3.as_transform().unwrap_or(p2_transform); // Fallback

            // Interpolate rotation with SLERP (splines are not great for quaternions)
            let rot = crate::value::transform::slerp_quaternion(
                &p1_transform.rotation.to_array(),
                &p2_transform.rotation.to_array(),
                context.t,
            );

            // Interpolate position and scale with Catmull-Rom
            let pos_x = catmull_rom_spline(
                p0_transform.position.x,
                p1_transform.position.x,
                p2_transform.position.x,
                p3_transform.position.x,
                context.t,
            );
            let pos_y = catmull_rom_spline(
                p0_transform.position.y,
                p1_transform.position.y,
                p2_transform.position.y,
                p3_transform.position.y,
                context.t,
            );
            let pos_z = catmull_rom_spline(
                p0_transform.position.z,
                p1_transform.position.z,
                p2_transform.position.z,
                p3_transform.position.z,
                context.t,
            );

            let scale_x = catmull_rom_spline(
                p0_transform.scale.x,
                p1_transform.scale.x,
                p2_transform.scale.x,
                p3_transform.scale.x,
                context.t,
            );
            let scale_y = catmull_rom_spline(
                p0_transform.scale.y,
                p1_transform.scale.y,
                p2_transform.scale.y,
                p3_transform.scale.y,
                context.t,
            );
            let scale_z = catmull_rom_spline(
                p0_transform.scale.z,
                p1_transform.scale.z,
                p2_transform.scale.z,
                p3_transform.scale.z,
                context.t,
            );

            let mut components = Vec::new();
            components.extend_from_slice(&[pos_x, pos_y, pos_z]);
            components.extend_from_slice(&rot);
            components.extend_from_slice(&[scale_x, scale_y, scale_z]);

            return Value::from_components(start.value_type(), &components);
        }

        // Generic component-wise interpolation for other types
        let p0_components = p0.interpolatable_components();
        let p1_components = start.interpolatable_components();
        let p2_components = end.interpolatable_components();
        let p3_components = p3.interpolatable_components();

        if !(p1_components.len() == p2_components.len()
            && p1_components.len() == p0_components.len()
            && p1_components.len() == p3_components.len())
        {
            return Err(AnimationError::InterpolationError {
                reason: "Component count mismatch for Catmull-Rom interpolation".to_string(),
            });
        }

        let interpolated_components: Vec<f64> = (0..p1_components.len())
            .map(|i| {
                catmull_rom_spline(
                    p0_components[i],
                    p1_components[i],
                    p2_components[i],
                    p3_components[i],
                    context.t,
                )
            })
            .collect();

        Value::from_components(start.value_type(), &interpolated_components)
    }

    fn parameter_schema(&self) -> InterpolationParameterSchema {
        // Control points are implicit from the keyframe sequence
        InterpolationParameterSchema {
            parameters: HashMap::new(),
        }
    }
}
