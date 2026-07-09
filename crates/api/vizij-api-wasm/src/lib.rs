//! vizij-api-wasm: WASM JSON helpers and TS-friendly serialization for vizij-api-core.
//!
//! Values cross the JS boundary in Arora `Value` serde form (externally
//! tagged: `{"f32": 1.0}`, `{"bool": true}`, `{"str": "hi"}`,
//! `{"f32s": [...]}`, `{"struct": {...}}`, ...). Ingress goes through the
//! api-core normalizer ([`vizij_api_core::json`]), so legacy payload forms
//! (`{"vec3": [1, 2, 3]}`, `{"type": "float", "data": 1}`, bare primitives,
//! ...) are still accepted; outputs are always the canonical Arora form.

use serde_wasm_bindgen::to_value;
use vizij_api_core::{json, Value, WriteBatch};
use wasm_bindgen::prelude::*;

fn parse_value_json(value_json: &str) -> Result<Value, JsValue> {
    let raw: serde_json::Value =
        serde_json::from_str(value_json).map_err(|e| JsValue::from_str(&e.to_string()))?;
    json::parse_value(raw).map_err(|e| JsValue::from_str(&e.to_string()))
}

fn parse_writebatch_json(batch_json: &str) -> Result<WriteBatch, JsValue> {
    let raw: serde_json::Value =
        serde_json::from_str(batch_json).map_err(|e| JsValue::from_str(&e.to_string()))?;
    json::writebatch_from_json(raw).map_err(|e| JsValue::from_str(&e.to_string()))
}

/// Validate a `WriteBatch` JSON string (an array of `{ path, value, shape? }`
/// objects; values in any form the api-core normalizer accepts).
#[wasm_bindgen]
pub fn validate_writebatch_json(batch_json: &str) -> Result<(), JsValue> {
    parse_writebatch_json(batch_json).map(|_| ())
}

/// Parse a `WriteBatch` JSON string and return it as a JS object with values
/// in canonical Arora `Value` serde form.
#[wasm_bindgen]
pub fn writebatch_to_js(batch_json: &str) -> Result<JsValue, JsValue> {
    let batch = parse_writebatch_json(batch_json)?;
    to_value(&batch).map_err(|e| JsValue::from_str(&e.to_string()))
}

/// Validate a `Value` JSON string (canonical Arora serde or any legacy form
/// the api-core normalizer accepts).
#[wasm_bindgen]
pub fn validate_value_json(value_json: &str) -> Result<(), JsValue> {
    parse_value_json(value_json).map(|_| ())
}

/// Parse a `Value` JSON string and return it as a JS object in canonical
/// Arora `Value` serde form.
#[wasm_bindgen]
pub fn value_to_js(value_json: &str) -> Result<JsValue, JsValue> {
    let value = parse_value_json(value_json)?;
    to_value(&value).map_err(|e| JsValue::from_str(&e.to_string()))
}
