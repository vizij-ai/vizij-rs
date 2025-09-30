use std::collections::HashMap;

use arora_schema::value::{Type, Value};

// Public utility module for shared Value/Type operations
pub mod utils {
    use std::cmp::{max, min};

    use super::*;
    use arora_schema::keyvalue::KeyValue;
    use uuid::Uuid;

    // Get a default Value for the specified type
    pub fn default_value_for_type(value_type: &Type) -> Value {
        match value_type {
            Type::Unit => Value::Unit,
            Type::Boolean => Value::Boolean(false),
            Type::U8 => Value::U8(0),
            Type::U16 => Value::U16(0),
            Type::U32 => Value::U32(0),
            Type::U64 => Value::U64(0),
            Type::I8 => Value::I8(0),
            Type::I16 => Value::I16(0),
            Type::I32 => Value::I32(0),
            Type::I64 => Value::I64(0),
            Type::F32 => Value::F32(0.0),
            Type::F64 => Value::F64(0.0),
            Type::String => Value::String(String::new()),
            Type::Structure => Value::Structure(arora_schema::value::Structure {
                id: Uuid::nil(),
                fields: Vec::new(),
            }),
            Type::Enumeration => Value::Enumeration(arora_schema::value::Enumeration {
                id: Uuid::nil(),
                variant_id: Uuid::nil(),
                value: Box::new(Value::Unit),
            }),
            Type::ArrayBoolean => Value::ArrayBoolean(Vec::new()),
            Type::ArrayU8 => Value::ArrayU8(Vec::new()),
            Type::ArrayU16 => Value::ArrayU16(Vec::new()),
            Type::ArrayU32 => Value::ArrayU32(Vec::new()),
            Type::ArrayU64 => Value::ArrayU64(Vec::new()),
            Type::ArrayI8 => Value::ArrayI8(Vec::new()),
            Type::ArrayI16 => Value::ArrayI16(Vec::new()),
            Type::ArrayI32 => Value::ArrayI32(Vec::new()),
            Type::ArrayI64 => Value::ArrayI64(Vec::new()),
            Type::ArrayF32 => Value::ArrayF32(Vec::new()),
            Type::ArrayF64 => Value::ArrayF64(Vec::new()),
            Type::ArrayString => Value::ArrayString(Vec::new()),
            Type::ArrayStructure => Value::ArrayStructure {
                id: Uuid::nil(),
                elements: Vec::new(),
            },
            Type::ArrayEnumeration => Value::ArrayEnumeration {
                id: Uuid::nil(),
                elements: Vec::new(),
            },
            Type::KeyValue => Value::KeyValue(KeyValue {
                id: Uuid::nil(),
                fields: HashMap::new(),
            }),
            Type::Uuid => Value::Uuid(Uuid::nil()),
        }
    }

    // Determines if two values are type-compatible
    pub fn is_compatible(value1: &Value, value2: &Value) -> bool {
        std::mem::discriminant(value1) == std::mem::discriminant(value2)
    }

    // Check if a value is compatible with a given type
    pub fn is_compatible_type(value: &Value, for_type: &Type) -> bool {
        match value {
            Value::Unit => *for_type == Type::Unit,
            Value::Boolean(_) => *for_type == Type::Boolean,
            Value::U8(_) => *for_type == Type::U8,
            Value::U16(_) => *for_type == Type::U16,
            Value::U32(_) => *for_type == Type::U32,
            Value::U64(_) => *for_type == Type::U64,
            Value::I8(_) => *for_type == Type::I8,
            Value::I16(_) => *for_type == Type::I16,
            Value::I32(_) => *for_type == Type::I32,
            Value::I64(_) => *for_type == Type::I64,
            Value::F32(_) => *for_type == Type::F32,
            Value::F64(_) => *for_type == Type::F64,
            Value::String(_) => *for_type == Type::String,
            Value::Structure(_) => *for_type == Type::Structure,
            Value::Enumeration(_) => *for_type == Type::Enumeration,
            Value::ArrayBoolean(_) => *for_type == Type::ArrayBoolean,
            Value::ArrayU8(_) => *for_type == Type::ArrayU8,
            Value::ArrayU16(_) => *for_type == Type::ArrayU16,
            Value::ArrayU32(_) => *for_type == Type::ArrayU32,
            Value::ArrayU64(_) => *for_type == Type::ArrayU64,
            Value::ArrayI8(_) => *for_type == Type::ArrayI8,
            Value::ArrayI16(_) => *for_type == Type::ArrayI16,
            Value::ArrayI32(_) => *for_type == Type::ArrayI32,
            Value::ArrayI64(_) => *for_type == Type::ArrayI64,
            Value::ArrayF32(_) => *for_type == Type::ArrayF32,
            Value::ArrayF64(_) => *for_type == Type::ArrayF64,
            Value::ArrayString(_) => *for_type == Type::ArrayString,
            Value::ArrayStructure { .. } => *for_type == Type::ArrayStructure,
            Value::ArrayEnumeration { .. } => *for_type == Type::ArrayEnumeration,
            Value::KeyValue(_) => *for_type == Type::KeyValue,
            Value::Uuid(_) => *for_type == Type::Uuid,
        }
    }

    // Get the Type from a Value
    pub fn get_type_for_value(value: &Value) -> Type {
        match value {
            Value::Unit => Type::Unit,
            Value::Boolean(_) => Type::Boolean,
            Value::U8(_) => Type::U8,
            Value::U16(_) => Type::U16,
            Value::U32(_) => Type::U32,
            Value::U64(_) => Type::U64,
            Value::I8(_) => Type::I8,
            Value::I16(_) => Type::I16,
            Value::I32(_) => Type::I32,
            Value::I64(_) => Type::I64,
            Value::F32(_) => Type::F32,
            Value::F64(_) => Type::F64,
            Value::String(_) => Type::String,
            Value::Structure(_) => Type::Structure,
            Value::Enumeration(_) => Type::Enumeration,
            Value::ArrayBoolean(_) => Type::ArrayBoolean,
            Value::ArrayU8(_) => Type::ArrayU8,
            Value::ArrayU16(_) => Type::ArrayU16,
            Value::ArrayU32(_) => Type::ArrayU32,
            Value::ArrayU64(_) => Type::ArrayU64,
            Value::ArrayI8(_) => Type::ArrayI8,
            Value::ArrayI16(_) => Type::ArrayI16,
            Value::ArrayI32(_) => Type::ArrayI32,
            Value::ArrayI64(_) => Type::ArrayI64,
            Value::ArrayF32(_) => Type::ArrayF32,
            Value::ArrayF64(_) => Type::ArrayF64,
            Value::ArrayString(_) => Type::ArrayString,
            Value::ArrayStructure { .. } => Type::ArrayStructure,
            Value::ArrayEnumeration { .. } => Type::ArrayEnumeration,
            Value::KeyValue(_) => Type::KeyValue,
            Value::Uuid(_) => Type::Uuid,
        }
    }

    pub fn quick_convert_to_dst_type(v: &Value, dst: &Value, force: bool) -> Value {
        quick_convert_to_type(v, &utils::get_type_for_value(dst), force)
    }

    pub fn quick_convert_to_type(v: &Value, dst: &Type, force: bool) -> Value {
        // Automatically perform some conversions for cases where we know it's safe
        // Basically converting smaller numeric types to larger ones
        // or converting between integer and float types.
        match (v, dst) {
            // Convert to U64
            (Value::U32(x), Type::U64) => Value::U64(*x as u64),
            (Value::U16(x), Type::U64) => Value::U64(*x as u64),
            (Value::U8(x), Type::U64) => Value::U64(*x as u64),
            // Convert to U32
            (Value::U16(x), Type::U32) => Value::U32(*x as u32),
            (Value::U8(x), Type::U32) => Value::U32(*x as u32),
            // Convert to U16
            (Value::U8(x), Type::U16) => Value::U16(*x as u16),

            // Convert to I64
            (Value::U32(x), Type::I64) => Value::I64(*x as i64),
            (Value::U16(x), Type::I64) => Value::I64(*x as i64),
            (Value::U8(x), Type::I64) => Value::I64(*x as i64),
            (Value::I32(x), Type::I64) => Value::I64(*x as i64),
            (Value::I16(x), Type::I64) => Value::I64(*x as i64),
            (Value::I8(x), Type::I64) => Value::I64(*x as i64),
            // Convert to I32
            (Value::U16(x), Type::I32) => Value::I32(*x as i32),
            (Value::U8(x), Type::I32) => Value::I32(*x as i32),
            (Value::I16(x), Type::I32) => Value::I32(*x as i32),
            (Value::I8(x), Type::I32) => Value::I32(*x as i32),
            // Convert to I16
            (Value::U8(x), Type::I16) => Value::I16(*x as i16),
            (Value::I8(x), Type::I16) => Value::I16(*x as i16),

            // to F64
            (Value::F32(x), Type::F64) => Value::F64(*x as f64),
            (Value::I32(x), Type::F64) => Value::F64(*x as f64),
            (Value::I16(x), Type::F64) => Value::F64(*x as f64),
            (Value::I8(x), Type::F64) => Value::F64(*x as f64),
            (Value::U32(x), Type::F64) => Value::F64(*x as f64),
            (Value::U16(x), Type::F64) => Value::F64(*x as f64),
            (Value::U8(x), Type::F64) => Value::F64(*x as f64),

            // to F32
            (Value::I16(x), Type::F32) => Value::F32(*x as f32),
            (Value::I8(x), Type::F32) => Value::F32(*x as f32),
            (Value::U16(x), Type::F32) => Value::F32(*x as f32),
            (Value::U8(x), Type::F32) => Value::F32(*x as f32),

            _ => {
                if force {
                    // If force is true, we can accept any numeric type and convert it to the destination type
                    // In this case we will clamp the value to ensure it fits within the destination type's range.
                    // This also converts strings to Uuids
                    match dst {
                        Type::Uuid => match v {
                            Value::String(s) => {
                                if let Ok(uuid) = Uuid::parse_str(s) {
                                    Value::Uuid(uuid)
                                } else {
                                    Value::Uuid(Uuid::nil())
                                }
                            }
                            _ => v.clone(),
                        },
                        Type::U64 => match v {
                            Value::I8(x) => Value::U64((0_i8.max(*x)) as u64),
                            Value::I16(x) => Value::U64((0_i16.max(*x)) as u64),
                            Value::I32(x) => Value::U64((0_i32.max(*x)) as u64),
                            Value::I64(x) => Value::U64((0_i64.max(*x)) as u64),
                            Value::F32(x) => Value::U64((0.0_f32.max(*x)) as u64),
                            Value::F64(x) => Value::U64((0.0_f64.max(*x)) as u64),
                            _ => v.clone(),
                        },
                        Type::U32 => match v {
                            Value::U64(x) => Value::U32(max(0, min(*x, u32::MAX as u64) as u32)),
                            Value::I8(x) => Value::U32((0_i8.max(*x)).min(u32::MAX as i8) as u32),
                            Value::I16(x) => Value::U32((0_i16.max(*x)).min(u8::MAX as i16) as u32),
                            Value::I32(x) => {
                                Value::U32((0_i32.max(*x)).min(u16::MAX as i32) as u32)
                            }
                            Value::I64(x) => {
                                Value::U32((0_i64.max(*x)).min(u32::MAX as i64) as u32)
                            }
                            Value::F32(x) => {
                                Value::U32((0.0_f32.max(*x)).min(u32::MAX as f32) as u32)
                            }
                            Value::F64(x) => {
                                Value::U32((0.0_f64.max(*x)).min(u32::MAX as f64) as u32)
                            }
                            _ => v.clone(),
                        },
                        Type::I64 => match v {
                            Value::U64(x) => {
                                Value::I64(max(*x, i64::MIN as u64).min(i64::MAX as u64) as i64)
                            }
                            Value::F32(x) => {
                                Value::I64((*x).max(i64::MIN as f32).min(i64::MAX as f32) as i64)
                            }
                            Value::F64(x) => {
                                Value::I64((*x).max(i64::MIN as f64).min(i64::MAX as f64) as i64)
                            }
                            _ => v.clone(),
                        },
                        Type::I32 => match v {
                            Value::U32(x) => {
                                Value::I32(max(*x, i32::MIN as u32).min(i32::MAX as u32) as i32)
                            }
                            Value::U64(x) => {
                                Value::I32(max(*x, i32::MIN as u64).min(i32::MAX as u64) as i32)
                            }
                            Value::I64(x) => {
                                Value::I32(max(*x, i32::MIN as i64).min(i32::MAX as i64) as i32)
                            }
                            Value::F32(x) => {
                                Value::I32((*x).max(i32::MIN as f32).min(i32::MAX as f32) as i32)
                            }
                            Value::F64(x) => {
                                Value::I32((*x).max(i32::MIN.into()).min(i32::MAX.into()) as i32)
                            }
                            _ => v.clone(),
                        },

                        // For floats it is possible to perform more efficient conversions using bit patters as in:
                        // Value::U32(x) => {
                        //      const F32_MAX_AS_U32: u32 = 0x7F7F_FFFF; // bit pattern of f32::MAX
                        //      Value::F32((*x).min(F32_MAX_AS_U32) as f32)
                        // }
                        // but I need to be properly study and test it to ensure correctness.
                        Type::F32 => match v {
                            // U32 -> F32 can be performed through an f64
                            Value::U32(x) => Value::F32((*x as f64).min(f32::MAX as f64) as f32),
                            // U64 -> F32 cannot be performed through f64 so we clamp in the U64 domain
                            Value::U64(x) => Value::F32(if *x > f32::MAX as u64 {
                                f32::MAX
                            } else {
                                *x as f32
                            }),
                            Value::I32(x) => Value::F32(
                                (*x as f64).max(f32::MIN as f64).min(f32::MAX as f64) as f32,
                            ),
                            Value::I64(x) => Value::F32(if *x > f32::MAX as i64 {
                                f32::MAX
                            } else if *x < f32::MIN as i64 {
                                f32::MIN
                            } else {
                                *x as f32
                            }),
                            Value::F64(x) => Value::F32(if x.is_nan() {
                                f32::NAN
                            } else {
                                x.max(f32::MIN as f64).min(f32::MAX as f64) as f32
                            }),
                            _ => v.clone(),
                        },
                        _ => {
                            // Default case, return original value
                            v.clone()
                        }
                    }
                } else {
                    v.clone() // Default case, return original value
                }
            }
        }
    }

    // Helper methods for type-specific value extraction
    pub fn as_bool(value: &Value) -> Option<bool> {
        match value {
            Value::Boolean(v) => Some(*v),
            _ => None,
        }
    }

    pub fn as_int(value: &Value) -> Option<i32> {
        match value {
            Value::I32(v) => Some(*v),
            _ => None,
        }
    }

    pub fn as_float(value: &Value) -> Option<f64> {
        match value {
            Value::F64(v) => Some(*v),
            _ => None,
        }
    }

    pub fn as_string(value: &Value) -> Option<&String> {
        match value {
            Value::String(v) => Some(v),
            _ => None,
        }
    }

    pub fn as_array_bool(value: &Value) -> Option<&Vec<bool>> {
        match value {
            Value::ArrayBoolean(v) => Some(v),
            _ => None,
        }
    }

    pub fn as_array_int(value: &Value) -> Option<&Vec<i32>> {
        match value {
            Value::ArrayI32(v) => Some(v),
            _ => None,
        }
    }

    pub fn as_array_float(value: &Value) -> Option<&Vec<f64>> {
        match value {
            Value::ArrayF64(v) => Some(v),
            _ => None,
        }
    }

    pub fn as_array_string(value: &Value) -> Option<&Vec<String>> {
        match value {
            Value::ArrayString(v) => Some(v),
            _ => None,
        }
    }

    // Check if the value is any array type
    pub fn is_array(value: &Value) -> bool {
        matches!(
            value,
            Value::ArrayBoolean(_)
                | Value::ArrayU8(_)
                | Value::ArrayU16(_)
                | Value::ArrayU32(_)
                | Value::ArrayU64(_)
                | Value::ArrayI8(_)
                | Value::ArrayI16(_)
                | Value::ArrayI32(_)
                | Value::ArrayI64(_)
                | Value::ArrayF32(_)
                | Value::ArrayF64(_)
                | Value::ArrayString(_)
                | Value::ArrayStructure { .. }
                | Value::ArrayEnumeration { .. }
        )
    }
}
