//use ::adt::{utils, ChannelStruct, ChannelStructMeta};
use arora_schema::value::{Type, Value};
use vizij_blackboard_core::adt::utils;

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
