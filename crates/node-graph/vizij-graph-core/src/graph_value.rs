//! The value abstraction the graph evaluator is generic over.
//!
//! The node graph never inspects concrete value variants: every value read or
//! constructed goes through the vizij *vocabulary* surface (constructors,
//! accessors, and lossy coercions). [`GraphValue`] reifies exactly that surface
//! as a trait so the evaluator can be parameterized over any value type that
//! implements it. The default parameter throughout the crate is
//! [`vizij_api_core::Value`], whose [`GraphValue`] impl simply delegates to the
//! existing free functions — so concrete value semantics live entirely outside
//! graph-core.

use serde::de::DeserializeOwned;
use serde::Serialize;
use std::fmt::Debug;
use uuid::Uuid;

pub use vizij_api_core::value::{Transform, VizijKind};

use vizij_api_core::value as vocab;
use vizij_api_core::{coercion, Value};

/// A value the node graph can read, construct, and flow between ports.
///
/// The method set mirrors the vizij vocabulary one-for-one: constructors build
/// a value of a given kind, accessors read a value's typed content when it
/// matches, and the `to_*` coercions give every value a numeric reading. Types
/// that appear in signatures ([`Transform`], [`VizijKind`], `Uuid`) come from
/// `vizij_api_core` and are *not* the value type itself.
pub trait GraphValue: Clone + Debug + Serialize + DeserializeOwned {
    // ---- constructors ----------------------------------------------------------------

    /// Scalar float.
    fn float(v: f32) -> Self;
    /// Boolean.
    fn bool_(v: bool) -> Self;
    /// UTF-8 text.
    fn text(s: &str) -> Self;
    /// Variable-length numeric vector.
    fn vector(xs: Vec<f32>) -> Self;
    /// 2D vector structure.
    fn vec2(a: [f32; 2]) -> Self;
    /// 3D vector structure.
    fn vec3(a: [f32; 3]) -> Self;
    /// 4D vector structure.
    fn vec4(a: [f32; 4]) -> Self;
    /// Quaternion structure (`[x, y, z, w]`).
    fn quat(a: [f32; 4]) -> Self;
    /// RGBA color structure.
    fn color_rgba(a: [f32; 4]) -> Self;
    /// Transform structure.
    fn transform(t: Transform) -> Self;
    /// String-keyed record.
    fn record<'a, I>(entries: I) -> Self
    where
        I: IntoIterator<Item = (&'a str, Self)>;
    /// Heterogeneous sequence.
    fn array(items: Vec<Self>) -> Self;
    /// Enumeration value identified by [`GraphValue::variant_id`] of `variant`.
    fn enumeration(variant: &str, payload: Self) -> Self;

    /// Deterministic variant id for an enum variant name.
    fn variant_id(variant: &str) -> Uuid;

    // ---- accessors --------------------------------------------------------------------

    /// Classify this value against the vizij vocabulary.
    fn kind(&self) -> VizijKind;
    /// Read a scalar float.
    fn as_float(&self) -> Option<f32>;
    /// Read a boolean.
    fn as_bool(&self) -> Option<bool>;
    /// Read UTF-8 text.
    fn as_text(&self) -> Option<&str>;
    /// Read a variable-length numeric vector.
    fn as_vector(&self) -> Option<&[f32]>;
    /// Read a `vec2` structure.
    fn as_vec2(&self) -> Option<[f32; 2]>;
    /// Read a `vec3` structure.
    fn as_vec3(&self) -> Option<[f32; 3]>;
    /// Read a `vec4` structure.
    fn as_vec4(&self) -> Option<[f32; 4]>;
    /// Read a `quat` structure.
    fn as_quat(&self) -> Option<[f32; 4]>;
    /// Read a `color-rgba` structure.
    fn as_color_rgba(&self) -> Option<[f32; 4]>;
    /// Read a `transform` structure.
    fn as_transform(&self) -> Option<Transform>;
    /// Read a record as `(name, value)` pairs, sorted by name.
    fn as_record(&self) -> Option<Vec<(&str, &Self)>>;
    /// Read a heterogeneous sequence.
    fn as_array(&self) -> Option<&[Self]>;
    /// Read an enumeration as `(variant_id, payload)`.
    fn as_enumeration(&self) -> Option<(Uuid, &Self)>;

    // ---- coercions --------------------------------------------------------------------

    /// Coerce this value into a scalar `f32`.
    fn to_float(&self) -> f32;
    /// Coerce this value into a generic `Vec<f32>`.
    fn to_vector(&self) -> Vec<f32>;
    /// Coerce this value into a `[f32; 3]`.
    fn to_vec3(&self) -> [f32; 3];
}

impl GraphValue for Value {
    fn float(v: f32) -> Self {
        vocab::float(v)
    }
    fn bool_(v: bool) -> Self {
        vocab::bool_(v)
    }
    fn text(s: &str) -> Self {
        vocab::text(s)
    }
    fn vector(xs: Vec<f32>) -> Self {
        vocab::vector(xs)
    }
    fn vec2(a: [f32; 2]) -> Self {
        vocab::vec2(a)
    }
    fn vec3(a: [f32; 3]) -> Self {
        vocab::vec3(a)
    }
    fn vec4(a: [f32; 4]) -> Self {
        vocab::vec4(a)
    }
    fn quat(a: [f32; 4]) -> Self {
        vocab::quat(a)
    }
    fn color_rgba(a: [f32; 4]) -> Self {
        vocab::color_rgba(a)
    }
    fn transform(t: Transform) -> Self {
        vocab::transform(t)
    }
    fn record<'a, I>(entries: I) -> Self
    where
        I: IntoIterator<Item = (&'a str, Self)>,
    {
        vocab::record(entries)
    }
    fn array(items: Vec<Self>) -> Self {
        vocab::array(items)
    }
    fn enumeration(variant: &str, payload: Self) -> Self {
        vocab::enumeration(variant, payload)
    }
    fn variant_id(variant: &str) -> Uuid {
        vocab::variant_id(variant)
    }

    fn kind(&self) -> VizijKind {
        vocab::kind(self)
    }
    fn as_float(&self) -> Option<f32> {
        vocab::as_float(self)
    }
    fn as_bool(&self) -> Option<bool> {
        vocab::as_bool(self)
    }
    fn as_text(&self) -> Option<&str> {
        vocab::as_text(self)
    }
    fn as_vector(&self) -> Option<&[f32]> {
        vocab::as_vector(self)
    }
    fn as_vec2(&self) -> Option<[f32; 2]> {
        vocab::as_vec2(self)
    }
    fn as_vec3(&self) -> Option<[f32; 3]> {
        vocab::as_vec3(self)
    }
    fn as_vec4(&self) -> Option<[f32; 4]> {
        vocab::as_vec4(self)
    }
    fn as_quat(&self) -> Option<[f32; 4]> {
        vocab::as_quat(self)
    }
    fn as_color_rgba(&self) -> Option<[f32; 4]> {
        vocab::as_color_rgba(self)
    }
    fn as_transform(&self) -> Option<Transform> {
        vocab::as_transform(self)
    }
    fn as_record(&self) -> Option<Vec<(&str, &Self)>> {
        vocab::as_record(self)
    }
    fn as_array(&self) -> Option<&[Self]> {
        vocab::as_array(self)
    }
    fn as_enumeration(&self) -> Option<(Uuid, &Self)> {
        vocab::as_enumeration(self)
    }

    fn to_float(&self) -> f32 {
        coercion::to_float(self)
    }
    fn to_vector(&self) -> Vec<f32> {
        coercion::to_vector(self)
    }
    fn to_vec3(&self) -> [f32; 3] {
        coercion::to_vec3(self)
    }
}
