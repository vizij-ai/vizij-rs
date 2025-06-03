use crate::value::ValueType;
use crate::Value;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Parameter schema for interpolation functions
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InterpolationParameterSchema {
    pub parameters: HashMap<String, ParameterDefinition>,
}

/// Definition of a single parameter for an interpolation function
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ParameterDefinition {
    pub name: String,
    pub value_type: ValueType,
    pub default_value: Option<Value>,
    pub min_value: Option<Value>,
    pub max_value: Option<Value>,
    pub description: String,
}
