//! The vizij value vocabulary over `arora_types::value::Value`.
//!
//! Vizij has no value enum of its own: every runtime value is an
//! [`arora_types::value::Value`], re-exported here as [`Value`]. This module
//! declares the vizij vocabulary on top of it:
//!
//! - **Type ids** for vizij's graphics-flavoured composites (`vec2`/`vec3`/
//!   `vec4`/`quat`/`color-rgba`/`transform`), which have no Arora primitive
//!   and map to `Value::Structure` with stable type/field ids. The ids are
//!   UUIDs namespaced under the ASCII bytes of "vizij" so they are
//!   self-identifying and collision-free. They are the source of truth for
//!   vizij's structured values, shared by module codegen and Studio
//!   introspection.
//! - **Constructors** (`vec3([f32; 3]) -> Value`, ...) building the canonical
//!   encoding for each vocabulary entry.
//! - **Accessors** (`as_vec3(&Value) -> Option<[f32; 3]>`, ...) reading them
//!   back into plain Rust types. These are the kernel seam: hot code decodes
//!   a `Value` once into PODs, does its math on those, and re-encodes at the
//!   store boundary.
//! - A coarse [`VizijKind`] classifier for dispatch.
//!
//! Primitive mapping: `f32` -> `Value::F32`, `bool` -> `Value::Boolean`,
//! text -> `Value::String`, numeric vector -> `Value::ArrayF32`. Records map
//! to `Value::KeyValue` (string-keyed; field ids derived from the key names),
//! sequences to `Value::ArrayValue` (one sequence kind; a declared
//! [`crate::Shape`] carries any array/list/tuple distinction), and enums to
//! Arora's native `Value::Enumeration` with variant ids derived from the
//! variant names via [`variant_id`].

use arora_types::gen_uuid_from_str;
use arora_types::keyvalue::{KeyValue, KeyValueField};
use arora_types::value::{Enumeration, Structure, StructureField};
use uuid::Uuid;

pub use arora_types::value::Value;

// ---- declared type / field ids ------------------------------------------------

/// Namespace for all vizij type ids: the ASCII bytes of "vizij"
/// (`76 69 7a 69 6a`) in the leading bytes, so every id is self-identifying.
pub const VIZIJ_NS: u128 = 0x7669_7a69_6a00_0000_0000_0000_0000_0000;

const fn id(offset: u128) -> Uuid {
    Uuid::from_u128(VIZIJ_NS | offset)
}

/// Type id of the `vec2` structure.
pub const VEC2_TYPE: Uuid = id(0x0002);
/// Field ids of the `vec2` structure, in `[x, y]` order.
pub const VEC2_FIELDS: [Uuid; 2] = [id(0x0002_0001), id(0x0002_0002)];

/// Type id of the `vec3` structure.
pub const VEC3_TYPE: Uuid = id(0x0003);
/// Field ids of the `vec3` structure, in `[x, y, z]` order.
pub const VEC3_FIELDS: [Uuid; 3] = [id(0x0003_0001), id(0x0003_0002), id(0x0003_0003)];

/// Type id of the `vec4` structure.
pub const VEC4_TYPE: Uuid = id(0x0004);
/// Field ids of the `vec4` structure, in `[x, y, z, w]` order.
pub const VEC4_FIELDS: [Uuid; 4] = [
    id(0x0004_0001),
    id(0x0004_0002),
    id(0x0004_0003),
    id(0x0004_0004),
];

/// Type id of the `quat` structure (`[x, y, z, w]`).
pub const QUAT_TYPE: Uuid = id(0x0010);
/// Field ids of the `quat` structure, in `[x, y, z, w]` order.
pub const QUAT_FIELDS: [Uuid; 4] = [
    id(0x0010_0001),
    id(0x0010_0002),
    id(0x0010_0003),
    id(0x0010_0004),
];

/// Type id of the `color-rgba` structure (linear by convention).
pub const COLOR_RGBA_TYPE: Uuid = id(0x0020);
/// Field ids of the `color-rgba` structure, in `[r, g, b, a]` order.
pub const COLOR_RGBA_FIELDS: [Uuid; 4] = [
    id(0x0020_0001),
    id(0x0020_0002),
    id(0x0020_0003),
    id(0x0020_0004),
];

/// Type id of the `transform` structure.
pub const TRANSFORM_TYPE: Uuid = id(0x0030);
/// Field id of the `transform` translation (a `vec3` structure).
pub const TRANSFORM_TRANSLATION: Uuid = id(0x0030_0001);
/// Field id of the `transform` rotation (a `quat` structure).
pub const TRANSFORM_ROTATION: Uuid = id(0x0030_0002);
/// Field id of the `transform` scale (a `vec3` structure).
pub const TRANSFORM_SCALE: Uuid = id(0x0030_0003);

/// Enumeration type id for vizij enums whose variant set is open
/// (variants identified by [`variant_id`] of their name).
pub const ENUM_TYPE: Uuid = id(0x0040);

/// KeyValue id used for vizij records (open string-keyed field sets).
pub const RECORD_TYPE: Uuid = id(0x0050);

// ---- POD forms ------------------------------------------------------------------

/// Plain-Rust form of the `transform` structure.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Transform {
    /// Translation as `[x, y, z]`.
    pub translation: [f32; 3],
    /// Rotation quaternion as `[x, y, z, w]`.
    pub rotation: [f32; 4],
    /// Per-axis scale as `[x, y, z]`.
    pub scale: [f32; 3],
}

/// Coarse classification of a [`Value`] against the vizij vocabulary.
///
/// `Other` covers every Arora value the vocabulary gives no reading to
/// (integers, unit, uuid, unknown structures, ...); such values still flow
/// through the store untouched, they just have no vizij-specific semantics.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum VizijKind {
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
pub fn kind(value: &Value) -> VizijKind {
    match value {
        Value::F32(_) | Value::F64(_) => VizijKind::Float,
        Value::Boolean(_) => VizijKind::Bool,
        Value::String(_) => VizijKind::Text,
        Value::ArrayF32(_) => VizijKind::Vector,
        Value::Structure(s) => match s.id {
            t if t == VEC2_TYPE => VizijKind::Vec2,
            t if t == VEC3_TYPE => VizijKind::Vec3,
            t if t == VEC4_TYPE => VizijKind::Vec4,
            t if t == QUAT_TYPE => VizijKind::Quat,
            t if t == COLOR_RGBA_TYPE => VizijKind::ColorRgba,
            t if t == TRANSFORM_TYPE => VizijKind::Transform,
            _ => VizijKind::Other,
        },
        Value::KeyValue(_) => VizijKind::Record,
        Value::ArrayValue(_) => VizijKind::Array,
        Value::Enumeration(_) => VizijKind::Enum,
        _ => VizijKind::Other,
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

/// 2D vector structure.
pub fn vec2(a: [f32; 2]) -> Value {
    float_structure(VEC2_TYPE, &VEC2_FIELDS, &a)
}

/// 3D vector structure.
pub fn vec3(a: [f32; 3]) -> Value {
    float_structure(VEC3_TYPE, &VEC3_FIELDS, &a)
}

/// 4D vector structure.
pub fn vec4(a: [f32; 4]) -> Value {
    float_structure(VEC4_TYPE, &VEC4_FIELDS, &a)
}

/// Quaternion structure (`[x, y, z, w]`).
pub fn quat(a: [f32; 4]) -> Value {
    float_structure(QUAT_TYPE, &QUAT_FIELDS, &a)
}

/// RGBA color structure (linear by convention).
pub fn color_rgba(a: [f32; 4]) -> Value {
    float_structure(COLOR_RGBA_TYPE, &COLOR_RGBA_FIELDS, &a)
}

/// Transform structure (translation `vec3`, rotation `quat`, scale `vec3`).
pub fn transform(t: Transform) -> Value {
    structure(
        TRANSFORM_TYPE,
        vec![
            (TRANSFORM_TRANSLATION, vec3(t.translation)),
            (TRANSFORM_ROTATION, quat(t.rotation)),
            (TRANSFORM_SCALE, vec3(t.scale)),
        ],
    )
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

fn float_structure(ty: Uuid, fields: &[Uuid], comps: &[f32]) -> Value {
    structure(
        ty,
        fields
            .iter()
            .zip(comps)
            .map(|(id, c)| (*id, Value::F32(*c)))
            .collect(),
    )
}

fn structure(id: Uuid, fields: Vec<(Uuid, Value)>) -> Value {
    Value::Structure(Structure {
        id,
        fields: fields
            .into_iter()
            .map(|(id, value)| StructureField {
                id,
                value: Box::new(value),
            })
            .collect(),
    })
}

// ---- accessors --------------------------------------------------------------------

/// Read a scalar float (`F32`, or `F64` narrowed to `f32`).
pub fn as_float(value: &Value) -> Option<f32> {
    match value {
        Value::F32(f) => Some(*f),
        Value::F64(f) => Some(*f as f32),
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

/// Read a `vec2` structure.
pub fn as_vec2(value: &Value) -> Option<[f32; 2]> {
    read_float_structure(value, VEC2_TYPE, &VEC2_FIELDS)
}

/// Read a `vec3` structure.
pub fn as_vec3(value: &Value) -> Option<[f32; 3]> {
    read_float_structure(value, VEC3_TYPE, &VEC3_FIELDS)
}

/// Read a `vec4` structure.
pub fn as_vec4(value: &Value) -> Option<[f32; 4]> {
    read_float_structure(value, VEC4_TYPE, &VEC4_FIELDS)
}

/// Read a `quat` structure (`[x, y, z, w]`).
pub fn as_quat(value: &Value) -> Option<[f32; 4]> {
    read_float_structure(value, QUAT_TYPE, &QUAT_FIELDS)
}

/// Read a `color-rgba` structure (`[r, g, b, a]`).
pub fn as_color_rgba(value: &Value) -> Option<[f32; 4]> {
    read_float_structure(value, COLOR_RGBA_TYPE, &COLOR_RGBA_FIELDS)
}

/// Read a `transform` structure into its POD form.
pub fn as_transform(value: &Value) -> Option<Transform> {
    let s = as_structure(value, TRANSFORM_TYPE)?;
    Some(Transform {
        translation: as_vec3(read_field(s, TRANSFORM_TRANSLATION)?)?,
        rotation: as_quat(read_field(s, TRANSFORM_ROTATION)?)?,
        scale: as_vec3(read_field(s, TRANSFORM_SCALE)?)?,
    })
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

fn as_structure(value: &Value, ty: Uuid) -> Option<&Structure> {
    match value {
        Value::Structure(s) if s.id == ty => Some(s),
        _ => None,
    }
}

fn read_field(s: &Structure, field: Uuid) -> Option<&Value> {
    s.fields
        .iter()
        .find(|f| f.id == field)
        .map(|f| f.value.as_ref())
}

fn read_float_structure<const N: usize>(
    value: &Value,
    ty: Uuid,
    fields: &[Uuid; N],
) -> Option<[f32; N]> {
    let s = as_structure(value, ty)?;
    let mut out = [0.0f32; N];
    for (slot, field_id) in out.iter_mut().zip(fields) {
        *slot = as_float(read_field(s, *field_id)?)?;
    }
    Some(out)
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
        assert_eq!(kind(&float(1.0)), VizijKind::Float);
        assert_eq!(kind(&Value::F64(1.0)), VizijKind::Float);
        assert_eq!(kind(&bool_(true)), VizijKind::Bool);
        assert_eq!(kind(&text("x")), VizijKind::Text);
        assert_eq!(kind(&vector(vec![1.0])), VizijKind::Vector);
        assert_eq!(kind(&vec2([0.0; 2])), VizijKind::Vec2);
        assert_eq!(kind(&vec3([0.0; 3])), VizijKind::Vec3);
        assert_eq!(kind(&vec4([0.0; 4])), VizijKind::Vec4);
        assert_eq!(kind(&quat([0.0, 0.0, 0.0, 1.0])), VizijKind::Quat);
        assert_eq!(kind(&color_rgba([0.0; 4])), VizijKind::ColorRgba);
        assert_eq!(
            kind(&transform(Transform {
                translation: [0.0; 3],
                rotation: [0.0, 0.0, 0.0, 1.0],
                scale: [1.0; 3],
            })),
            VizijKind::Transform
        );
        assert_eq!(kind(&record([])), VizijKind::Record);
        assert_eq!(kind(&array(vec![])), VizijKind::Array);
        assert_eq!(kind(&enumeration("v", float(0.0))), VizijKind::Enum);
        assert_eq!(kind(&Value::U32(3)), VizijKind::Other);
        assert_eq!(kind(&Value::Unit), VizijKind::Other);
        // Unknown structure ids are outside the vocabulary.
        let foreign = Value::Structure(Structure {
            id: Uuid::from_u128(0x9999),
            fields: vec![],
        });
        assert_eq!(kind(&foreign), VizijKind::Other);
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
