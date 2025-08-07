//! A collection of cubic interpolation functions, including Bezier, Hermite, Catmull-Rom, and B‑spline.
//!
//! These functions are generic over any type `T` that supports basic arithmetic operations with `f64` scalars.
use std::ops::{Add, Mul, Sub};

/// Compute a point on a cubic Bezier curve at parameter `t`.
///
/// # Parameters
/// - `control_point_start`, `control_point_handle1`, `control_point_handle2`, `control_point_end`:
///   The four control points defining the curve.  
///   - `control_point_start` is where the curve begins (t = 0).  
///   - `control_point_handle1` and `control_point_handle2` influence the tangents at the start and end.  
///   - `control_point_end` is where the curve finishes (t = 1).
/// - `t`: Parameter in [0, 1], where 0 returns `control_point_start` and 1 returns `control_point_end`.
///
/// # Formula
/// B(t) = (1 - t)^3 P0 + 3 (1 - t)^2 t P1 + 3 (1 - t) t^2 P2 + t^3 P3
pub fn bezier_curve<T>(
    control_point_start: T,
    control_point_handle1: T,
    control_point_handle2: T,
    control_point_end: T,
    t: f64,
) -> T
where
    T: Copy + Add<Output = T> + Mul<f64, Output = T>,
{
    let one_minus_t = 1.0 - t;
    let blend_start = one_minus_t.powi(3);
    let blend_handle1 = 3.0 * one_minus_t.powi(2) * t;
    let blend_handle2 = 3.0 * one_minus_t * t.powi(2);
    let blend_end = t.powi(3);

    control_point_start * blend_start
        + control_point_handle1 * blend_handle1
        + control_point_handle2 * blend_handle2
        + control_point_end * blend_end
}

//------------------------------------------------------------------------------
// Hermite spline basis functions
fn hermite_basis_h00(t: f64) -> f64 {
    2.0 * t.powi(3) - 3.0 * t.powi(2) + 1.0
}
fn hermite_basis_h10(t: f64) -> f64 {
    t.powi(3) - 2.0 * t.powi(2) + t
}
fn hermite_basis_h01(t: f64) -> f64 {
    -2.0 * t.powi(3) + 3.0 * t.powi(2)
}
fn hermite_basis_h11(t: f64) -> f64 {
    t.powi(3) - t.powi(2)
}

/// Compute a point on a cubic Hermite spline at parameter `t`.
///
/// # Parameters
/// - `point_start`, `point_end`: The end positions of the spline segment.  
///   - `point_start` corresponds to t = 0.
///   - `point_end` corresponds to t = 1.
/// - `tangent_start`, `tangent_end`: Tangent (derivative) vectors at the start and end points.  
///   These control points represent the instantaneous rate of change (direction and speed) at each endpoint:
///   - `tangent_start` is the derivative at `point_start`.  
///   - `tangent_end` is the derivative at `point_end`.
/// - `t`: Parameter in [0, 1], where 0 returns `point_start` and 1 returns `point_end`.
///
/// # Formula
/// H(t) = h00(t) * P0 + h10(t) * M0 + h01(t) * P1 + h11(t) * M1
pub fn hermite_spline<T>(
    point_start: T,
    point_end: T,
    tangent_start: T,
    tangent_end: T,
    t: f64,
) -> T
where
    T: Copy + Add<Output = T> + Mul<f64, Output = T>,
{
    point_start * hermite_basis_h00(t)
        + tangent_start * hermite_basis_h10(t)
        + point_end * hermite_basis_h01(t)
        + tangent_end * hermite_basis_h11(t)
}

/// Compute a point on a Catmull-Rom spline at parameter `t`.
///
/// # Parameters
/// - `prev_point`, `current_point`, `next_point`, `next_next_point`: Four consecutive points along the path.
/// - `t`: Parameter in [0, 1] between `current_point` (t = 0) and `next_point` (t = 1).
///
/// # Tangent Approximation
/// We approximate the tangent vectors at the endpoints by finite differences:
/// - `tangent_start` = 0.5 * (next_point - prev_point)
/// - `tangent_end`   = 0.5 * (next_next_point - current_point)
///
/// Internally this reuses the Hermite spline form with those tangents.
pub fn catmull_rom_spline<T>(
    prev_point: T,
    current_point: T,
    next_point: T,
    next_next_point: T,
    t: f64,
) -> T
where
    T: Copy + Add<Output = T> + Sub<Output = T> + Mul<f64, Output = T>,
{
    let tangent_start = (next_point - prev_point) * 0.5;
    let tangent_end = (next_next_point - current_point) * 0.5;
    hermite_spline(current_point, next_point, tangent_start, tangent_end, t)
}

/// Compute a point on a uniform cubic B‑spline at parameter `t`.
///
/// # Parameters
/// - `control_point_before`, `control_point_start`, `control_point_end`, `control_point_after`:
///   Four consecutive control points defining the B-spline segment.  
///   The curve segment lies between `control_point_start` (t = 0) and `control_point_end` (t = 1).
/// - `t`: Parameter in [0, 1] along the segment.
///
/// # Uniform cubic B‑spline basis functions
/// B0(t) = (-t^3 + 3t^2 - 3t + 1) / 6
/// B1(t) = ( 3t^3 - 6t^2 + 4      ) / 6
/// B2(t) = (-3t^3 + 3t^2 + 3t + 1) / 6
/// B3(t) = ( t^3                  ) / 6
pub fn b_spline_curve<T>(
    control_point_before: T,
    control_point_start: T,
    control_point_end: T,
    control_point_after: T,
    t: f64,
) -> T
where
    T: Copy + Add<Output = T> + Mul<f64, Output = T>,
{
    let t2 = t * t;
    let t3 = t2 * t;
    let basis0 = (-t3 + 3.0 * t2 - 3.0 * t + 1.0) / 6.0;
    let basis1 = (3.0 * t3 - 6.0 * t2 + 4.0) / 6.0;
    let basis2 = (-3.0 * t3 + 3.0 * t2 + 3.0 * t + 1.0) / 6.0;
    let basis3 = t3 / 6.0;

    control_point_before * basis0
        + control_point_start * basis1
        + control_point_end * basis2
        + control_point_after * basis3
}
