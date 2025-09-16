//! vizij-api-wasm: WASM JSON helpers and TS-friendly serialization for vizij-api-core.
//! Minimal glue: expose helpers to validate/parse Value and WriteBatch JSON and return JS objects.

use serde_wasm_bindgen::to_value;
use vizij_api_core::{Value, WriteBatch};
use wasm_bindgen::prelude::*;

#[wasm_bindgen]
pub fn validate_writebatch_json(batch_json: &str) -> Result<(), JsValue> {
    let parsed: Result<WriteBatch, _> = serde_json::from_str(batch_json);
    parsed
        .map(|_| ())
        .map_err(|e| JsValue::from_str(&e.to_string()))
}

#[wasm_bindgen]
pub fn writebatch_to_js(batch_json: &str) -> Result<JsValue, JsValue> {
    let parsed: Result<WriteBatch, _> = serde_json::from_str(batch_json);
    match parsed {
        Ok(b) => to_value(&b).map_err(|e| JsValue::from_str(&e.to_string())),
        Err(e) => Err(JsValue::from_str(&e.to_string())),
    }
}

#[wasm_bindgen]
pub fn validate_value_json(value_json: &str) -> Result<(), JsValue> {
    let parsed: Result<Value, _> = serde_json::from_str(value_json);
    parsed
        .map(|_| ())
        .map_err(|e| JsValue::from_str(&e.to_string()))
}

#[wasm_bindgen]
pub fn value_to_js(value_json: &str) -> Result<JsValue, JsValue> {
    let parsed: Result<Value, _> = serde_json::from_str(value_json);
    match parsed {
        Ok(v) => to_value(&v).map_err(|e| JsValue::from_str(&e.to_string())),
        Err(e) => Err(JsValue::from_str(&e.to_string())),
    }
}
