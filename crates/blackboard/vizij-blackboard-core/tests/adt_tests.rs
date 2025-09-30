//use ::adt::{utils, ChannelStruct, ChannelStructMeta};
use arora_schema::value::{Type, Value};
use vizij_blackboard_core::adt::utils;

#[test]
fn test_value_getters() {
    let bool_value = Value::Boolean(true);
    assert_eq!(utils::as_bool(&bool_value), Some(true));
    assert_eq!(utils::as_int(&bool_value), None);

    let int_value = Value::I32(42);
    assert_eq!(utils::as_int(&int_value), Some(42));
    assert_eq!(utils::as_float(&int_value), None);

    let float_value = Value::F64(3.14);
    assert_eq!(utils::as_float(&float_value), Some(3.14));
    assert_eq!(utils::as_string(&float_value), None);

    let string_value = Value::String("test".to_string());
    assert_eq!(utils::as_string(&string_value), Some(&"test".to_string()));
    assert_eq!(utils::as_array_bool(&string_value), None);
}

#[test]
fn test_default_for_type() {
    assert_eq!(
        utils::default_value_for_type(&Type::Boolean),
        Value::Boolean(false)
    );
    assert_eq!(utils::default_value_for_type(&Type::I32), Value::I32(0));
    assert_eq!(utils::default_value_for_type(&Type::F64), Value::F64(0.0));
    assert_eq!(
        utils::default_value_for_type(&Type::String),
        Value::String(String::new())
    );
}
