//! WebAssembly JSON helpers for Vizij `Value` and `WriteBatch` payloads.
//!
//! These bindings wrap `vizij-api-core` so JS/TS tools can validate payloads, normalize
//! shorthand JSON, and receive canonical `{ type, data }` objects without pulling the
//! heavier engine runtimes. The exposed API is stateless and safe to call repeatedly.
//!
//! See also: the higher-level wasm crates (`vizij-animation-wasm`, `vizij-graph-wasm`,
//! `vizij-orchestrator-wasm`) reuse these helpers to normalize their inputs.

use serde_wasm_bindgen::to_value;
use vizij_api_core::{Value, WriteBatch};
use wasm_bindgen::prelude::*;

#[wasm_bindgen]
/// Validates a `WriteBatch` JSON string.
///
/// Returns `Ok(())` when the payload parses successfully; otherwise returns a JS error
/// containing the Rust parse message.
///
/// # Examples (JS)
/// ```javascript
/// import { validate_writebatch_json } from "@vizij/api-wasm";
///
/// validate_writebatch_json('{"writes":[{"path":"rig/hip.x","value":{"float":1.0}}]}');
/// ```
///
/// # Errors
/// Returns a JS error string if the JSON does not match the `WriteBatch` schema.
pub fn validate_writebatch_json(batch_json: &str) -> Result<(), JsValue> {
    let parsed: Result<WriteBatch, _> = serde_json::from_str(batch_json);
    parsed
        .map(|_| ())
        .map_err(|e| JsValue::from_str(&e.to_string()))
}

#[wasm_bindgen]
/// Normalizes a `WriteBatch` JSON string into a JS object.
///
/// This accepts the same shorthand JSON as the Rust parser and returns an object with
/// a canonical `{ writes: [...] }` layout.
///
/// # Examples (JS)
/// ```javascript
/// import { writebatch_to_js } from "@vizij/api-wasm";
///
/// const batch = writebatch_to_js('{"writes":[{"path":"demo/x","value":{"float":1}}]}');
/// console.log(batch.writes.length);
/// ```
///
/// # Errors
/// Returns a JS error string if parsing or serialization fails.
pub fn writebatch_to_js(batch_json: &str) -> Result<JsValue, JsValue> {
    let parsed: Result<WriteBatch, _> = serde_json::from_str(batch_json);
    match parsed {
        Ok(b) => to_value(&b).map_err(|e| JsValue::from_str(&e.to_string())),
        Err(e) => Err(JsValue::from_str(&e.to_string())),
    }
}

#[wasm_bindgen]
/// Validates a `Value` JSON string.
///
/// # Examples (JS)
/// ```javascript
/// import { validate_value_json } from "@vizij/api-wasm";
///
/// validate_value_json('{"vec3":[0,1,2]}');
/// ```
///
/// # Errors
/// Returns a JS error string if the JSON does not match the `Value` schema.
pub fn validate_value_json(value_json: &str) -> Result<(), JsValue> {
    let parsed: Result<Value, _> = serde_json::from_str(value_json);
    parsed
        .map(|_| ())
        .map_err(|e| JsValue::from_str(&e.to_string()))
}

#[wasm_bindgen]
/// Normalizes a `Value` JSON string into a JS object.
///
/// The returned object uses the canonical `{ type, data }` shape, even if the input
/// uses shorthand syntax like `{"vec3":[0,1,2]}`.
///
/// # Examples (JS)
/// ```javascript
/// import { value_to_js } from "@vizij/api-wasm";
///
/// const value = value_to_js('{"vec3":[0,1,2]}');
/// console.log(value.type);
/// ```
///
/// # Errors
/// Returns a JS error string if parsing or serialization fails.
pub fn value_to_js(value_json: &str) -> Result<JsValue, JsValue> {
    let parsed: Result<Value, _> = serde_json::from_str(value_json);
    match parsed {
        Ok(v) => to_value(&v).map_err(|e| JsValue::from_str(&e.to_string())),
        Err(e) => Err(JsValue::from_str(&e.to_string())),
    }
}
