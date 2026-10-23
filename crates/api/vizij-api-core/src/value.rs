//! The vizij value vocabulary over `arora_types::value::Value`.
//!
//! Vizij has no value enum of its own: every runtime value is an
//! [`arora_types::value::Value`], re-exported here as [`Value`]. This module
//! declares the vizij vocabulary on top of it:
//!
//! - **Type ids** for vizij's composites, which have no Arora primitive and
//!   map to `Value::Structure` with stable type/field ids. The geometry types
//!   (`vec2`/`vec3`/`vec4`/`quat`/`transform`) are Arora's own, declared by
//!   `arora-geometry` and re-exported here; `color-rgba` is declared here.
//!   The ids are UUIDs namespaced under the ASCII bytes of "vizij" so they
//!   are self-identifying and collision-free. They are shared by module
//!   codegen and Studio introspection.
//! - **Constructors** (`vec3([f32; 3]) -> Value`, ...) building the canonical
//!   encoding for each vocabulary entry.
//! - **Accessors** (`as_vec3(&Value) -> Option<[f32; 3]>`, ...) reading them
//!   back into plain Rust types. These are the kernel seam: hot code decodes
//!   a `Value` once into PODs, does its math on those, and re-encodes at the
//!   store boundary.
//! - A coarse [`ShapeKind`] classifier for dispatch.
//!
//! Primitive mapping: `f32` -> `Value::F32`, `bool` -> `Value::Boolean`,
//! text -> `Value::String`, numeric vector -> `Value::ArrayF32`. Records map
//! to `Value::KeyValue` (string-keyed; field ids derived from the key names),
//! sequences to `Value::ArrayValue` (one sequence kind; a declared
//! [`crate::Shape`] carries any array/list/tuple distinction), and enums to
//! Arora's native `Value::Enumeration` with variant ids derived from the
//! variant names via [`variant_id`].

use arora_geometry::{float_structure, read_float_structure, Kind};
use arora_types::gen_uuid_from_str;
use arora_types::keyvalue::{KeyValue, KeyValueField};
use uuid::Uuid;

pub use arora_geometry::{
    as_quat, as_transform, as_vec2, as_vec3, as_vec4, quat, transform, vec2, vec3, vec4, Transform,
    QUAT_FIELDS, QUAT_TYPE, TRANSFORM_ROTATION, TRANSFORM_SCALE, TRANSFORM_TRANSLATION,
    TRANSFORM_TYPE, VEC2_FIELDS, VEC2_TYPE, VEC3_FIELDS, VEC3_TYPE, VEC4_FIELDS, VEC4_TYPE,
};
pub use arora_types::value::{Enumeration, Structure, StructureField, StructureWithoutId, Value};

// ---- declared type / field ids ------------------------------------------------

/// Namespace for all vizij type ids: the ASCII bytes of "vizij"
/// (`76 69 7a 69 6a`) in the leading bytes, so every id is self-identifying.
/// The geometry types' ids live under it too.
pub const VIZIJ_NS: u128 = arora_geometry::NAMESPACE;

const fn id(offset: u128) -> Uuid {
    Uuid::from_u128(VIZIJ_NS | offset)
}

/// Type id of the `color-rgba` structure (linear by convention).
pub const COLOR_RGBA_TYPE: Uuid = id(0x0020);
/// Field ids of the `color-rgba` structure, in `[r, g, b, a]` order.
pub const COLOR_RGBA_FIELDS: [Uuid; 4] = [
    id(0x0020_0001),
    id(0x0020_0002),
    id(0x0020_0003),
    id(0x0020_0004),
];

/// Enumeration type id for vizij enums whose variant set is open
/// (variants identified by [`variant_id`] of their name).
pub const ENUM_TYPE: Uuid = id(0x0040);

/// KeyValue id used for vizij records (open string-keyed field sets).
pub const RECORD_TYPE: Uuid = id(0x0050);

// ---- classification ----------------------------------------------------------------

/// Coarse classification of a [`Value`] against the vizij vocabulary.
///
/// `Other` covers every Arora value the vocabulary gives no reading to
/// (integers, unit, uuid, unknown structures, ...); such values still flow
/// through the store untouched, they just have no vizij-specific semantics.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum ShapeKind {
    /// Scalar float (`F32` or `F64`).
    Float,
    /// Boolean.
    Bool,
    /// UTF-8 text.
    Text,
    /// Variable-length numeric vector (`ArrayF32`).
    Vector,
    /// `vec2` structure.
    Vec2,
    /// `vec3` structure.
    Vec3,
    /// `vec4` structure.
    Vec4,
    /// `quat` structure.
    Quat,
    /// `color-rgba` structure.
    ColorRgba,
    /// `transform` structure.
    Transform,
    /// String-keyed record (`KeyValue`).
    Record,
    /// Heterogeneous sequence (`ArrayValue`).
    Array,
    /// Native enumeration.
    Enum,
    /// No vizij reading.
    Other,
}

/// Classify a value against the vizij vocabulary.
pub fn kind(value: &Value) -> ShapeKind {
    match value {
        // The graph has one scalar kind; an integer from the store (the
        // runtime's `dt`, a frame's dimensions) is a number to its arithmetic.
        Value::F32(_)
        | Value::F64(_)
        | Value::U8(_)
        | Value::U16(_)
        | Value::U32(_)
        | Value::U64(_)
        | Value::I8(_)
        | Value::I16(_)
        | Value::I32(_)
        | Value::I64(_) => ShapeKind::Float,
        Value::Boolean(_) => ShapeKind::Bool,
        Value::String(_) => ShapeKind::Text,
        Value::ArrayF32(_) => ShapeKind::Vector,
        Value::Structure(s) => match arora_geometry::kind(value) {
            Some(Kind::Vec2) => ShapeKind::Vec2,
            Some(Kind::Vec3) => ShapeKind::Vec3,
            Some(Kind::Vec4) => ShapeKind::Vec4,
            Some(Kind::Quat) => ShapeKind::Quat,
            Some(Kind::Transform) => ShapeKind::Transform,
            None if s.id == COLOR_RGBA_TYPE => ShapeKind::ColorRgba,
            None => ShapeKind::Other,
        },
        Value::KeyValue(_) => ShapeKind::Record,
        Value::ArrayValue(_) => ShapeKind::Array,
        Value::Enumeration(_) => ShapeKind::Enum,
        _ => ShapeKind::Other,
    }
}

// ---- constructors ----------------------------------------------------------------

/// Scalar float.
pub fn float(v: f32) -> Value {
    Value::F32(v)
}

/// Boolean. (Named `bool_` because `bool` is a primitive type name.)
pub fn bool_(v: bool) -> Value {
    Value::Boolean(v)
}

/// UTF-8 text.
pub fn text(s: &str) -> Value {
    Value::String(s.to_string())
}

/// Variable-length numeric vector.
pub fn vector(xs: Vec<f32>) -> Value {
    Value::ArrayF32(xs)
}

/// RGBA color structure (linear by convention).
pub fn color_rgba(a: [f32; 4]) -> Value {
    float_structure(COLOR_RGBA_TYPE, &COLOR_RGBA_FIELDS, &a)
}

/// String-keyed record. Field ids derive deterministically from the key names.
pub fn record<'a>(entries: impl IntoIterator<Item = (&'a str, Value)>) -> Value {
    let mut kv = KeyValue::new_with_id(RECORD_TYPE);
    for (key, value) in entries {
        kv.set_field(KeyValueField::new_with_id(
            key,
            gen_uuid_from_str(key),
            value,
        ));
    }
    Value::KeyValue(kv)
}

/// Heterogeneous sequence. Any declared array/list/tuple distinction lives in
/// the path's [`crate::Shape`], not in the value.
pub fn array(items: Vec<Value>) -> Value {
    Value::ArrayValue(items)
}

/// Enumeration value of the open vizij enum type: the variant is identified
/// by [`variant_id`] of its name.
pub fn enumeration(variant: &str, payload: Value) -> Value {
    Value::Enumeration(Enumeration {
        id: ENUM_TYPE,
        variant_id: variant_id(variant),
        value: Box::new(payload),
    })
}

/// Deterministic variant id for an enum variant name. Readers compare the
/// `variant_id` of an [`as_enumeration`] result against `variant_id("name")`;
/// the name itself is not stored in the value.
pub fn variant_id(variant: &str) -> Uuid {
    gen_uuid_from_str(variant)
}

// ---- accessors --------------------------------------------------------------------

/// Read a scalar number as a float: `F32`, `F64` narrowed to `f32`, or any
/// integer widened — the store carries integers too (the runtime's `dt` in
/// nanoseconds, a frame's dimensions), and arithmetic on them is arithmetic.
pub fn as_float(value: &Value) -> Option<f32> {
    match value {
        Value::F32(f) => Some(*f),
        Value::F64(f) => Some(*f as f32),
        Value::U8(n) => Some(*n as f32),
        Value::U16(n) => Some(*n as f32),
        Value::U32(n) => Some(*n as f32),
        Value::U64(n) => Some(*n as f32),
        Value::I8(n) => Some(*n as f32),
        Value::I16(n) => Some(*n as f32),
        Value::I32(n) => Some(*n as f32),
        Value::I64(n) => Some(*n as f32),
        _ => None,
    }
}

/// Read a boolean.
pub fn as_bool(value: &Value) -> Option<bool> {
    match value {
        Value::Boolean(b) => Some(*b),
        _ => None,
    }
}

/// Read UTF-8 text.
pub fn as_text(value: &Value) -> Option<&str> {
    match value {
        Value::String(s) => Some(s),
        _ => None,
    }
}

/// Read a variable-length numeric vector.
pub fn as_vector(value: &Value) -> Option<&[f32]> {
    match value {
        Value::ArrayF32(xs) => Some(xs),
        _ => None,
    }
}

/// Read a `color-rgba` structure (`[r, g, b, a]`).
pub fn as_color_rgba(value: &Value) -> Option<[f32; 4]> {
    read_float_structure(value, COLOR_RGBA_TYPE, &COLOR_RGBA_FIELDS)
}

/// Read a record as `(name, value)` pairs, sorted by name for determinism
/// (the underlying `KeyValue` field map is unordered). Fields without a value
/// are skipped.
pub fn as_record(value: &Value) -> Option<Vec<(&str, &Value)>> {
    match value {
        Value::KeyValue(kv) => {
            let mut entries: Vec<(&str, &Value)> = kv
                .fields
                .iter()
                .filter_map(|(key, field)| {
                    field.value.as_deref().map(|value| (key.as_str(), value))
                })
                .collect();
            entries.sort_by_key(|(key, _)| *key);
            Some(entries)
        }
        _ => None,
    }
}

/// Read one field of a record by name: `None` when `value` is no record, or
/// when the record has no field `name` or that field holds no value. One map
/// lookup, where [`as_record`] lists and sorts every field.
pub fn record_field<'a>(value: &'a Value, name: &str) -> Option<&'a Value> {
    match value {
        Value::KeyValue(kv) => kv.fields.get(name)?.value.as_deref(),
        _ => None,
    }
}

/// Read a heterogeneous sequence.
pub fn as_array(value: &Value) -> Option<&[Value]> {
    match value {
        Value::ArrayValue(items) => Some(items),
        _ => None,
    }
}

/// Read an enumeration as `(variant_id, payload)`. Compare the variant id
/// against [`variant_id`] of the expected variant name.
pub fn as_enumeration(value: &Value) -> Option<(Uuid, &Value)> {
    match value {
        Value::Enumeration(e) => Some((e.variant_id, &e.value)),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn primitives_round_trip() {
        assert_eq!(as_float(&float(1.5)), Some(1.5));
        assert_eq!(as_bool(&bool_(true)), Some(true));
        assert_eq!(as_text(&text("hi")), Some("hi"));
        assert_eq!(
            as_vector(&vector(vec![1.0, 2.0, 3.0])),
            Some(&[1.0, 2.0, 3.0][..])
        );
    }

    #[test]
    fn float_reads_f64_too() {
        assert_eq!(as_float(&Value::F64(2.5)), Some(2.5));
    }

    #[test]
    fn composites_round_trip() {
        assert_eq!(as_vec2(&vec2([1.0, 2.0])), Some([1.0, 2.0]));
        assert_eq!(as_vec3(&vec3([1.0, 2.0, 3.0])), Some([1.0, 2.0, 3.0]));
        assert_eq!(
            as_vec4(&vec4([1.0, 2.0, 3.0, 4.0])),
            Some([1.0, 2.0, 3.0, 4.0])
        );
        assert_eq!(
            as_quat(&quat([0.0, 0.0, 0.0, 1.0])),
            Some([0.0, 0.0, 0.0, 1.0])
        );
        assert_eq!(
            as_color_rgba(&color_rgba([0.1, 0.2, 0.3, 1.0])),
            Some([0.1, 0.2, 0.3, 1.0])
        );

        let t = Transform {
            translation: [1.0, 2.0, 3.0],
            rotation: [0.0, 0.0, 0.0, 1.0],
            scale: [1.0, 1.0, 1.0],
        };
        assert_eq!(as_transform(&transform(t)), Some(t));
    }

    #[test]
    fn composites_are_structures_with_vizij_ids() {
        match vec3([1.0, 2.0, 3.0]) {
            Value::Structure(s) => assert_eq!(s.id, VEC3_TYPE),
            other => panic!("expected structure, got {other:?}"),
        }
    }

    #[test]
    fn accessors_reject_other_kinds() {
        assert_eq!(as_vec3(&vec4([1.0, 2.0, 3.0, 4.0])), None);
        assert_eq!(as_quat(&vec4([1.0, 2.0, 3.0, 4.0])), None);
        assert_eq!(as_float(&bool_(true)), None);
        assert_eq!(as_vector(&vec3([1.0, 2.0, 3.0])), None);
    }

    #[test]
    fn record_round_trips_and_sorts_by_name() {
        let value = record([
            ("shoulder", float(0.4)),
            ("elbow", float(-1.2)),
            ("nested", record([("x", float(1.0))])),
        ]);
        let entries = as_record(&value).expect("record");
        let names: Vec<&str> = entries.iter().map(|(name, _)| *name).collect();
        assert_eq!(names, vec!["elbow", "nested", "shoulder"]);
        assert_eq!(as_float(entries[0].1), Some(-1.2));
        let nested = as_record(entries[1].1).expect("nested record");
        assert_eq!(as_float(nested[0].1), Some(1.0));
    }

    #[test]
    fn a_record_field_reads_by_name() {
        let value = record([("x", float(1.0)), ("nested", record([("y", float(2.0))]))]);
        assert_eq!(record_field(&value, "x").and_then(as_float), Some(1.0));
        let nested = record_field(&value, "nested").expect("nested record");
        assert_eq!(record_field(nested, "y").and_then(as_float), Some(2.0));
        assert_eq!(record_field(&value, "missing"), None);
        assert_eq!(record_field(&float(1.0), "x"), None);
    }

    #[test]
    fn records_with_equal_entries_are_equal() {
        let a = record([("x", float(1.0)), ("y", float(2.0))]);
        let b = record([("y", float(2.0)), ("x", float(1.0))]);
        assert_eq!(a, b);
    }

    #[test]
    fn array_round_trips() {
        let value = array(vec![float(1.0), vec3([1.0, 2.0, 3.0]), text("mixed")]);
        let items = as_array(&value).expect("array");
        assert_eq!(items.len(), 3);
        assert_eq!(as_vec3(&items[1]), Some([1.0, 2.0, 3.0]));
    }

    #[test]
    fn enumeration_round_trips_via_variant_id() {
        let value = enumeration("grasp", record([("force", float(0.5))]));
        let (variant, payload) = as_enumeration(&value).expect("enumeration");
        assert_eq!(variant, variant_id("grasp"));
        assert_ne!(variant, variant_id("release"));
        let fields = as_record(payload).expect("payload record");
        assert_eq!(fields[0].0, "force");
    }

    #[test]
    fn kinds_classify_the_vocabulary() {
        assert_eq!(kind(&float(1.0)), ShapeKind::Float);
        assert_eq!(kind(&Value::F64(1.0)), ShapeKind::Float);
        assert_eq!(kind(&bool_(true)), ShapeKind::Bool);
        assert_eq!(kind(&text("x")), ShapeKind::Text);
        assert_eq!(kind(&vector(vec![1.0])), ShapeKind::Vector);
        assert_eq!(kind(&vec2([0.0; 2])), ShapeKind::Vec2);
        assert_eq!(kind(&vec3([0.0; 3])), ShapeKind::Vec3);
        assert_eq!(kind(&vec4([0.0; 4])), ShapeKind::Vec4);
        assert_eq!(kind(&quat([0.0, 0.0, 0.0, 1.0])), ShapeKind::Quat);
        assert_eq!(kind(&color_rgba([0.0; 4])), ShapeKind::ColorRgba);
        assert_eq!(
            kind(&transform(Transform {
                translation: [0.0; 3],
                rotation: [0.0, 0.0, 0.0, 1.0],
                scale: [1.0; 3],
            })),
            ShapeKind::Transform
        );
        assert_eq!(kind(&record([])), ShapeKind::Record);
        assert_eq!(kind(&array(vec![])), ShapeKind::Array);
        assert_eq!(kind(&enumeration("v", float(0.0))), ShapeKind::Enum);
        // Integers are scalars to the graph, like the runtime's `dt`.
        assert_eq!(kind(&Value::U32(3)), ShapeKind::Float);
        assert_eq!(kind(&Value::U64(16_000_000)), ShapeKind::Float);
        assert_eq!(as_float(&Value::U64(16_000_000)), Some(16_000_000.0));
        assert_eq!(kind(&Value::Unit), ShapeKind::Other);
        // Unknown structure ids are outside the vocabulary.
        let foreign = Value::Structure(Structure {
            id: Uuid::from_u128(0x9999),
            fields: vec![],
        });
        assert_eq!(kind(&foreign), ShapeKind::Other);
        assert_eq!(as_vec2(&foreign), None);
    }

    /// Pins the JSON wire form and the type-id strings that
    /// `@vizij/value-json` (npm) hard-codes in its arora-serde decoder —
    /// its `VIZIJ_*_TYPE` constants and `fromAroraValueJSON` tests must keep
    /// matching these exactly.
    #[test]
    fn json_wire_form_matches_the_js_decoder() {
        assert_eq!(
            VEC2_TYPE.to_string(),
            "76697a69-6a00-0000-0000-000000000002"
        );
        assert_eq!(
            VEC3_TYPE.to_string(),
            "76697a69-6a00-0000-0000-000000000003"
        );
        assert_eq!(
            VEC4_TYPE.to_string(),
            "76697a69-6a00-0000-0000-000000000004"
        );
        assert_eq!(
            QUAT_TYPE.to_string(),
            "76697a69-6a00-0000-0000-000000000010"
        );
        assert_eq!(
            COLOR_RGBA_TYPE.to_string(),
            "76697a69-6a00-0000-0000-000000000020"
        );
        assert_eq!(
            TRANSFORM_TYPE.to_string(),
            "76697a69-6a00-0000-0000-000000000030"
        );
        assert_eq!(
            ENUM_TYPE.to_string(),
            "76697a69-6a00-0000-0000-000000000040"
        );
        assert_eq!(
            RECORD_TYPE.to_string(),
            "76697a69-6a00-0000-0000-000000000050"
        );
        assert_eq!(
            TRANSFORM_TRANSLATION.to_string(),
            "76697a69-6a00-0000-0000-000000300001"
        );
        assert_eq!(
            TRANSFORM_ROTATION.to_string(),
            "76697a69-6a00-0000-0000-000000300002"
        );
        assert_eq!(
            TRANSFORM_SCALE.to_string(),
            "76697a69-6a00-0000-0000-000000300003"
        );

        assert_eq!(
            serde_json::to_string(&float(1.5)).unwrap(),
            r#"{"f32":1.5}"#
        );
        assert_eq!(
            serde_json::to_string(&bool_(true)).unwrap(),
            r#"{"bool":true}"#
        );
        assert_eq!(
            serde_json::to_string(&text("hi")).unwrap(),
            r#"{"str":"hi"}"#
        );
        assert_eq!(
            serde_json::to_string(&vector(vec![1.0, 2.0])).unwrap(),
            r#"{"f32s":[1.0,2.0]}"#
        );
        assert_eq!(
            serde_json::to_string(&vec3([1.0, 2.0, 3.0])).unwrap(),
            r#"{"struct":{"id":"76697a69-6a00-0000-0000-000000000003","fields":[{"id":"76697a69-6a00-0000-0000-000000030001","value":{"f32":1.0}},{"id":"76697a69-6a00-0000-0000-000000030002","value":{"f32":2.0}},{"id":"76697a69-6a00-0000-0000-000000030003","value":{"f32":3.0}}]}}"#
        );
    }
}
