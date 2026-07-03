//! Conversion between Vizij values (`vizij_api_core::Value`) and the Arora
//! `Value` vocabulary (`arora_types::value::Value`).
//!
//! Vizij's graphics-flavoured composites (`Vec2`/`Vec3`/`Vec4`/`Quat`/
//! `ColorRgba`/`Transform`) have no Arora primitive, so they map to
//! `Value::Structure` with stable type/field ids — the same shape the
//! `arora-module-cli` generator emits for the declared Arora types (see
//! VIZ-39 / arora-sdk#100). Primitives map directly (`Float`->`F32`, ...).
//!
//! `Value` carries no metadata, so a value's `Shape.meta` (unit/space/range/
//! color_space) rides a **sidecar key** `"meta/<path>"` -- see [`meta_key`].
//!
//! Vizij's *recursive* composites convert structurally, to arbitrary depth:
//! `Record` <-> `Value::KeyValue` (string-keyed, field ids derived from the
//! key names), `List`/`Array`/`Tuple` -> `Value::ArrayValue`, and `Enum` <->
//! a tagged `Value::Structure`. Arora's `ArrayValue` has a single sequence
//! kind, so `Array` and `Tuple` come back as `List`; the declared `Shape` of
//! the path, not the wire value, is what preserves that distinction.
//!
//! The ids below are **Vizij type ids** — UUIDs namespaced under the ASCII
//! bytes of "vizij" so they are self-identifying and collision-free. They are
//! the source of truth for Vizij's structured values. An equivalent
//! Arora-declared type (identical structural shape) would be linked by mapping
//! its id onto the matching Vizij id here; nothing in vizij-rs needs to run the
//! Arora generator for that.

use std::collections::HashMap;

use arora_types::gen_uuid_from_str;
use arora_types::keyvalue::{KeyValue, KeyValueField};
use arora_types::value::{Structure, StructureField, Value as AValue};
use uuid::Uuid;
use vizij_api_core::{Shape, Value as VValue};

/// Failures converting between the two `Value` vocabularies.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum ConversionError {
    /// The Arora value has no Vizij mapping.
    #[error("no Vizij mapping for this Arora value")]
    UnsupportedArora,
    /// A `Value::Structure` carried a type id we do not recognise.
    #[error("unknown Arora structure type id {0}")]
    UnknownStructure(Uuid),
    /// A structure was missing an expected field.
    #[error("structure {ty} is missing field {field}")]
    MissingField { ty: Uuid, field: Uuid },
    /// A structure field held an unexpected value kind.
    #[error("structure field {field} has an unexpected value kind")]
    FieldKind { field: Uuid },
}

// ---- declared type / field ids ------------------------------------------------

/// Namespace for all Vizij type ids: the ASCII bytes of "vizij"
/// (`76 69 7a 69 6a`) in the leading bytes, so every id is self-identifying.
const VIZIJ_NS: u128 = 0x7669_7a69_6a00_0000_0000_0000_0000_0000;

const fn id(offset: u128) -> Uuid {
    Uuid::from_u128(VIZIJ_NS | offset)
}

const VEC2_TYPE: Uuid = id(0x0002);
const VEC2_FIELDS: [Uuid; 2] = [id(0x0002_0001), id(0x0002_0002)];

const VEC3_TYPE: Uuid = id(0x0003);
const VEC3_FIELDS: [Uuid; 3] = [id(0x0003_0001), id(0x0003_0002), id(0x0003_0003)];

const VEC4_TYPE: Uuid = id(0x0004);
const VEC4_FIELDS: [Uuid; 4] = [
    id(0x0004_0001),
    id(0x0004_0002),
    id(0x0004_0003),
    id(0x0004_0004),
];

const QUAT_TYPE: Uuid = id(0x0010);
const QUAT_FIELDS: [Uuid; 4] = [
    id(0x0010_0001),
    id(0x0010_0002),
    id(0x0010_0003),
    id(0x0010_0004),
];

const COLOR_TYPE: Uuid = id(0x0020);
const COLOR_FIELDS: [Uuid; 4] = [
    id(0x0020_0001),
    id(0x0020_0002),
    id(0x0020_0003),
    id(0x0020_0004),
];

const ENUM_TYPE: Uuid = id(0x0040);
const ENUM_TAG: Uuid = id(0x0040_0001);
const ENUM_VALUE: Uuid = id(0x0040_0002);

const RECORD_TYPE: Uuid = id(0x0050);

const TRANSFORM_TYPE: Uuid = id(0x0030);
const TRANSFORM_TRANSLATION: Uuid = id(0x0030_0001);
const TRANSFORM_ROTATION: Uuid = id(0x0030_0002);
const TRANSFORM_SCALE: Uuid = id(0x0030_0003);

// ---- Vizij -> Arora -----------------------------------------------------------

/// Convert a Vizij value into the Arora `Value` vocabulary.
pub fn to_arora(value: &VValue) -> Result<AValue, ConversionError> {
    Ok(match value {
        VValue::Float(f) => AValue::F32(*f),
        VValue::Bool(b) => AValue::Boolean(*b),
        VValue::Text(s) => AValue::String(s.clone()),
        VValue::Vector(xs) => AValue::ArrayF32(xs.clone()),
        VValue::Vec2(a) => vec_struct(VEC2_TYPE, &VEC2_FIELDS, a),
        VValue::Vec3(a) => vec_struct(VEC3_TYPE, &VEC3_FIELDS, a),
        VValue::Vec4(a) => vec_struct(VEC4_TYPE, &VEC4_FIELDS, a),
        VValue::Quat(a) => vec_struct(QUAT_TYPE, &QUAT_FIELDS, a),
        VValue::ColorRgba(a) => vec_struct(COLOR_TYPE, &COLOR_FIELDS, a),
        VValue::Transform {
            translation,
            rotation,
            scale,
        } => structure(
            TRANSFORM_TYPE,
            vec![
                (
                    TRANSFORM_TRANSLATION,
                    vec_struct(VEC3_TYPE, &VEC3_FIELDS, translation),
                ),
                (
                    TRANSFORM_ROTATION,
                    vec_struct(QUAT_TYPE, &QUAT_FIELDS, rotation),
                ),
                (TRANSFORM_SCALE, vec_struct(VEC3_TYPE, &VEC3_FIELDS, scale)),
            ],
        ),
        VValue::Enum(tag, payload) => structure(
            ENUM_TYPE,
            vec![
                (ENUM_TAG, AValue::String(tag.clone())),
                (ENUM_VALUE, to_arora(payload)?),
            ],
        ),
        VValue::Record(entries) => {
            let mut fields = HashMap::with_capacity(entries.len());
            for (key, entry) in entries {
                fields.insert(
                    key.clone(),
                    KeyValueField {
                        id: gen_uuid_from_str(key),
                        name: key.clone(),
                        value: Some(Box::new(to_arora(entry)?)),
                    },
                );
            }
            AValue::KeyValue(KeyValue {
                id: RECORD_TYPE,
                fields,
            })
        }
        VValue::Array(items) | VValue::List(items) | VValue::Tuple(items) => {
            AValue::ArrayValue(items.iter().map(to_arora).collect::<Result<Vec<_>, _>>()?)
        }
    })
}

fn vec_struct(ty: Uuid, fields: &[Uuid], comps: &[f32]) -> AValue {
    structure(
        ty,
        fields
            .iter()
            .zip(comps)
            .map(|(id, c)| (*id, AValue::F32(*c)))
            .collect(),
    )
}

fn structure(id: Uuid, fields: Vec<(Uuid, AValue)>) -> AValue {
    AValue::Structure(Structure {
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

// ---- Arora -> Vizij -----------------------------------------------------------

/// Convert an Arora value into a Vizij value.
pub fn from_arora(value: &AValue) -> Result<VValue, ConversionError> {
    Ok(match value {
        AValue::F32(f) => VValue::Float(*f),
        AValue::F64(f) => VValue::Float(*f as f32),
        AValue::Boolean(b) => VValue::Bool(*b),
        AValue::String(s) => VValue::Text(s.clone()),
        AValue::ArrayF32(xs) => VValue::Vector(xs.clone()),
        AValue::Structure(s) => structure_to_vizij(s)?,
        AValue::ArrayValue(items) => VValue::List(
            items
                .iter()
                .map(from_arora)
                .collect::<Result<Vec<_>, _>>()?,
        ),
        AValue::KeyValue(kv) => VValue::Record(
            kv.fields
                .iter()
                .map(|(key, field)| {
                    let value = field
                        .value
                        .as_deref()
                        .ok_or(ConversionError::FieldKind { field: field.id })?;
                    Ok((key.clone(), from_arora(value)?))
                })
                .collect::<Result<_, ConversionError>>()?,
        ),
        _ => return Err(ConversionError::UnsupportedArora),
    })
}

fn structure_to_vizij(s: &Structure) -> Result<VValue, ConversionError> {
    let ty = s.id;
    Ok(if ty == VEC2_TYPE {
        VValue::Vec2(read_array(s, &VEC2_FIELDS)?)
    } else if ty == VEC3_TYPE {
        VValue::Vec3(read_array(s, &VEC3_FIELDS)?)
    } else if ty == VEC4_TYPE {
        VValue::Vec4(read_array(s, &VEC4_FIELDS)?)
    } else if ty == QUAT_TYPE {
        VValue::Quat(read_array(s, &QUAT_FIELDS)?)
    } else if ty == COLOR_TYPE {
        VValue::ColorRgba(read_array(s, &COLOR_FIELDS)?)
    } else if ty == TRANSFORM_TYPE {
        VValue::Transform {
            translation: read_array(read_struct(s, TRANSFORM_TRANSLATION)?, &VEC3_FIELDS)?,
            rotation: read_array(read_struct(s, TRANSFORM_ROTATION)?, &QUAT_FIELDS)?,
            scale: read_array(read_struct(s, TRANSFORM_SCALE)?, &VEC3_FIELDS)?,
        }
    } else if ty == ENUM_TYPE {
        let tag = match read_field(s, ENUM_TAG)? {
            AValue::String(tag) => tag.clone(),
            _ => return Err(ConversionError::FieldKind { field: ENUM_TAG }),
        };
        let payload = from_arora(read_field(s, ENUM_VALUE)?)?;
        VValue::Enum(tag, Box::new(payload))
    } else {
        return Err(ConversionError::UnknownStructure(ty));
    })
}

fn read_array<const N: usize>(
    s: &Structure,
    fields: &[Uuid; N],
) -> Result<[f32; N], ConversionError> {
    let mut out = [0.0f32; N];
    for (slot, field_id) in out.iter_mut().zip(fields) {
        *slot = read_f32(s, *field_id)?;
    }
    Ok(out)
}

fn read_f32(s: &Structure, field: Uuid) -> Result<f32, ConversionError> {
    let entry = s
        .fields
        .iter()
        .find(|f| f.id == field)
        .ok_or(ConversionError::MissingField { ty: s.id, field })?;
    match entry.value.as_ref() {
        AValue::F32(v) => Ok(*v),
        AValue::F64(v) => Ok(*v as f32),
        _ => Err(ConversionError::FieldKind { field }),
    }
}

fn read_field(s: &Structure, field: Uuid) -> Result<&AValue, ConversionError> {
    s.fields
        .iter()
        .find(|f| f.id == field)
        .map(|f| f.value.as_ref())
        .ok_or(ConversionError::MissingField { ty: s.id, field })
}

fn read_struct(s: &Structure, field: Uuid) -> Result<&Structure, ConversionError> {
    let entry = s
        .fields
        .iter()
        .find(|f| f.id == field)
        .ok_or(ConversionError::MissingField { ty: s.id, field })?;
    match entry.value.as_ref() {
        AValue::Structure(inner) => Ok(inner),
        _ => Err(ConversionError::FieldKind { field }),
    }
}

// ---- /meta sidecar ------------------------------------------------------------

/// The sidecar key carrying the metadata for the value stored at `data_path`.
///
/// Arora's `Value` has no place for `Shape.meta` (unit/space/range/color_space),
/// so it travels the same store under a reserved `"meta/"` namespace. (No leading slash: `TypedPath` rejects an empty first segment, and the sidecar must be storable in a `BlackboardStore`.)
pub fn meta_key(data_path: &str) -> String {
    format!("meta/{}", data_path.trim_start_matches('/'))
}

/// Encode a value's shape metadata for the sidecar key, if any is present.
pub fn encode_shape_meta(shape: &Shape) -> Option<AValue> {
    if shape.meta.is_empty() {
        return None;
    }
    serde_json::to_string(&shape.meta).ok().map(AValue::String)
}

/// Decode shape metadata previously written to a sidecar key.
pub fn decode_shape_meta(value: &AValue) -> Option<HashMap<String, String>> {
    match value {
        AValue::String(s) => serde_json::from_str(s).ok(),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use vizij_api_core::ShapeId;

    fn round_trip(v: VValue) {
        let a = to_arora(&v).expect("to_arora");
        let back = from_arora(&a).expect("from_arora");
        assert_eq!(back, v);
    }

    #[test]
    fn primitives_round_trip() {
        round_trip(VValue::Float(1.5));
        round_trip(VValue::Bool(true));
        round_trip(VValue::Text("hi".into()));
        round_trip(VValue::Vector(vec![1.0, 2.0, 3.0]));
    }

    #[test]
    fn composites_round_trip() {
        round_trip(VValue::Vec2([1.0, 2.0]));
        round_trip(VValue::Vec3([1.0, 2.0, 3.0]));
        round_trip(VValue::Vec4([1.0, 2.0, 3.0, 4.0]));
        round_trip(VValue::Quat([0.0, 0.0, 0.0, 1.0]));
        round_trip(VValue::ColorRgba([0.1, 0.2, 0.3, 1.0]));
        round_trip(VValue::Transform {
            translation: [1.0, 2.0, 3.0],
            rotation: [0.0, 0.0, 0.0, 1.0],
            scale: [1.0, 1.0, 1.0],
        });
    }

    #[test]
    fn vec3_becomes_structure() {
        let a = to_arora(&VValue::Vec3([1.0, 2.0, 3.0])).unwrap();
        assert!(matches!(a, AValue::Structure(_)));
    }

    #[test]
    fn record_of_records_round_trips() {
        // The URDF IK/FK shape: a record of joint -> angle, here nested one
        // level deeper to prove arbitrary depth.
        let joints = VValue::Record(
            [
                ("shoulder".to_string(), VValue::Float(0.4)),
                ("elbow".to_string(), VValue::Float(-1.2)),
            ]
            .into_iter()
            .collect(),
        );
        let record = VValue::Record(
            [
                ("left_arm".to_string(), joints.clone()),
                ("confidence".to_string(), VValue::Float(0.9)),
            ]
            .into_iter()
            .collect(),
        );

        let arora = to_arora(&record).unwrap();
        assert!(matches!(arora, AValue::KeyValue(_)));
        assert_eq!(from_arora(&arora).unwrap(), record);
    }

    #[test]
    fn list_round_trips_and_sequences_collapse_to_list() {
        let list = VValue::List(vec![
            VValue::Float(1.0),
            VValue::Vec3([1.0, 2.0, 3.0]),
            VValue::Text("mixed".into()),
        ]);
        let arora = to_arora(&list).unwrap();
        assert!(matches!(arora, AValue::ArrayValue(_)));
        assert_eq!(from_arora(&arora).unwrap(), list);

        // Tuple and Array share ArrayValue on the wire, so they come back as
        // List: the path's declared Shape carries the distinction instead.
        let tuple = VValue::Tuple(vec![VValue::Float(1.0), VValue::Bool(true)]);
        let back = from_arora(&to_arora(&tuple).unwrap()).unwrap();
        assert_eq!(
            back,
            VValue::List(vec![VValue::Float(1.0), VValue::Bool(true)])
        );
    }

    #[test]
    fn enum_round_trips() {
        let value = VValue::Enum(
            "grasp".to_string(),
            Box::new(VValue::Record(
                [("force".to_string(), VValue::Float(0.5))]
                    .into_iter()
                    .collect(),
            )),
        );
        let arora = to_arora(&value).unwrap();
        assert_eq!(from_arora(&arora).unwrap(), value);
    }

    #[test]
    fn unknown_structure_is_reported() {
        let s = AValue::Structure(Structure {
            id: Uuid::from_u128(0x9999),
            fields: vec![],
        });
        assert!(matches!(
            from_arora(&s),
            Err(ConversionError::UnknownStructure(_))
        ));
    }

    #[test]
    fn meta_sidecar_round_trips() {
        assert_eq!(
            meta_key("standard/semio/mouth.x"),
            "meta/standard/semio/mouth.x"
        );
        let shape = Shape::new(ShapeId::Vec3)
            .with_meta("unit", "radians")
            .with_meta("space", "head");
        let encoded = encode_shape_meta(&shape).expect("some meta");
        let decoded = decode_shape_meta(&encoded).expect("decoded");
        assert_eq!(decoded.get("unit").map(String::as_str), Some("radians"));
        assert_eq!(decoded.get("space").map(String::as_str), Some("head"));
        assert!(encode_shape_meta(&Shape::new(ShapeId::Scalar)).is_none());
    }
}
