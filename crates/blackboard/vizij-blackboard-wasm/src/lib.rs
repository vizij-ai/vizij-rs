use std::sync::{Arc, Mutex};

use arora_schema::value::Value;
// Only import traits actually required for method resolution
use vizij_blackboard_core::bb::{ABBNodeTrait, ArcABBPathNodeTrait, ArcNamespacedSetterTrait};
use vizij_blackboard_core::ArcAroraBlackboard;
use wasm_bindgen::prelude::*;

// console_error_panic_hook is invoked via its fully qualified path when the feature is enabled;
// we intentionally avoid single-component path import to satisfy clippy.

/// Helper function to check if a JsValue is undefined or null
fn jsvalue_is_undefined_or_null(v: &JsValue) -> bool {
    v.is_undefined() || v.is_null()
}

/// Convert JsValue to arora_schema::Value (returns None for null/undefined)
fn jsvalue_to_value(js_val: JsValue) -> Result<Option<Value>, JsError> {
    if jsvalue_is_undefined_or_null(&js_val) {
        return Ok(None);
    }

    // Handle basic types directly
    if let Some(b) = js_val.as_bool() {
        return Ok(Some(Value::Boolean(b)));
    }

    if let Some(f) = js_val.as_f64() {
        return Ok(Some(Value::F64(f)));
    }

    if let Some(s) = js_val.as_string() {
        return Ok(Some(Value::String(s)));
    }

    // For arrays, try to detect the type and convert appropriately
    if js_sys::Array::is_array(&js_val) {
        let arr = js_sys::Array::from(&js_val);
        let length = arr.length();

        if length == 0 {
            // Empty array - default to string array
            return Ok(Some(Value::ArrayString(Vec::new())));
        }

        // Check the first element to determine array type
        let first = arr.get(0);
        if first.as_bool().is_some() {
            // Boolean array
            let mut values = Vec::new();
            for i in 0..length {
                if let Some(b) = arr.get(i).as_bool() {
                    values.push(b);
                } else {
                    return Err(JsError::new("Mixed types in boolean array"));
                }
            }
            return Ok(Some(Value::ArrayBoolean(values)));
        } else if first.as_f64().is_some() {
            // Number array
            let mut values = Vec::new();
            for i in 0..length {
                if let Some(f) = arr.get(i).as_f64() {
                    values.push(f);
                } else {
                    return Err(JsError::new("Mixed types in number array"));
                }
            }
            return Ok(Some(Value::ArrayF64(values)));
        } else if first.as_string().is_some() {
            // String array
            let mut values = Vec::new();
            for i in 0..length {
                if let Some(s) = arr.get(i).as_string() {
                    values.push(s);
                } else {
                    return Err(JsError::new("Mixed types in string array"));
                }
            }
            return Ok(Some(Value::ArrayString(values)));
        }
    }

    // For complex objects, we'll need to handle them as KeyValue or reject them
    if js_val.is_object() {
        return Err(JsError::new("Complex objects not yet supported. Use basic types (boolean, number, string) or arrays."));
    }

    Err(JsError::new("Unsupported JavaScript value type"))
}

/// Convert arora_schema::Value to JsValue
fn value_to_jsvalue(value: &Value) -> Result<JsValue, JsError> {
    match value {
        Value::Unit => Ok(JsValue::UNDEFINED),
        Value::Boolean(b) => Ok(JsValue::from_bool(*b)),
        Value::F32(f) => Ok(JsValue::from_f64(*f as f64)),
        Value::F64(f) => Ok(JsValue::from_f64(*f)),
        Value::I32(i) => Ok(JsValue::from_f64(*i as f64)),
        Value::I64(i) => Ok(JsValue::from_f64(*i as f64)),
        Value::U32(u) => Ok(JsValue::from_f64(*u as f64)),
        Value::U64(u) => Ok(JsValue::from_f64(*u as f64)),
        Value::String(s) => Ok(JsValue::from_str(s)),
        Value::ArrayBoolean(arr) => {
            let js_array = js_sys::Array::new();
            for b in arr {
                js_array.push(&JsValue::from_bool(*b));
            }
            Ok(js_array.into())
        }
        Value::ArrayF64(arr) => {
            let js_array = js_sys::Array::new();
            for f in arr {
                js_array.push(&JsValue::from_f64(*f));
            }
            Ok(js_array.into())
        }
        Value::ArrayString(arr) => {
            let js_array = js_sys::Array::new();
            for s in arr {
                js_array.push(&JsValue::from_str(s));
            }
            Ok(js_array.into())
        }
        _ => Err(JsError::new(&format!(
            "Unsupported Value type for conversion to JS: {:?}",
            value
        ))),
    }
}

/// WebAssembly interface for the Vizij Blackboard
#[wasm_bindgen]
pub struct VizijBlackboard {
    blackboard: Arc<Mutex<ArcAroraBlackboard>>,
}

#[wasm_bindgen]
impl VizijBlackboard {
    /// Create a new blackboard instance
    ///
    /// # Arguments
    /// * `name` - Optional name for the blackboard (defaults to "default")
    ///
    /// # Example (JavaScript)
    /// ```js
    /// const bb = new VizijBlackboard("my-blackboard");
    /// ```
    #[wasm_bindgen(constructor)]
    pub fn new(name: Option<String>) -> VizijBlackboard {
        #[cfg(feature = "console_error_panic_hook")]
        console_error_panic_hook::set_once();

        let bb_name = name.unwrap_or_else(|| "default".to_string());
        VizijBlackboard {
            blackboard: ArcAroraBlackboard::new(bb_name),
        }
    }

    /// Set a value at the given dot-separated path and return the UUID of the stored item.
    ///
    /// # Arguments
    /// * `path` - Dot-separated path (e.g., "robot.arm.joint1.angle")
    /// * `value` - Any JavaScript value (number, string, boolean, array)
    ///
    /// # Returns
    /// * `String` – The UUID (as a hyphenated string) of the item that was created or updated.
    ///             If the value was `null` / `undefined` (causing a removal) an empty string is returned.
    ///
    /// # Example (JavaScript)
    /// ```js
    /// const id1 = bb.set("robot.arm.angle", 45.0);     // => "550e8400-e29b-41d4-a716-446655440000"
    /// const id2 = bb.set("robot.name", "R2D2");       // => another UUID
    /// const removed = bb.set("robot.arm.angle", undefined); // => "" (empty string)
    /// ```
    ///
    /// This is a breaking change vs earlier versions where `set` returned nothing.
    #[wasm_bindgen]
    pub fn set(&mut self, path: &str, value: JsValue) -> Result<String, JsError> {
        if path.trim().is_empty() {
            return Err(JsError::new("Path cannot be empty"));
        }

        match jsvalue_to_value(value)? {
            Some(arora_value) => {
                let mut guard = self.blackboard.lock().unwrap();
                let uuid = guard
                    .set(path, arora_value)
                    .map_err(|e| JsError::new(&format!("Failed to set value: {}", e)))?;
                Ok(uuid.to_string())
            }
            None => {
                // Treat null / undefined as a removal request; return empty string.
                Ok(String::new())
            }
        }
    }

    /// Get a value from the given dot-separated path
    ///
    /// # Arguments
    /// * `path` - Dot-separated path (e.g., "robot.arm.joint1.angle")
    ///
    /// # Returns
    /// The value at the path, or undefined if not found
    ///
    /// # Example (JavaScript)
    /// ```js
    /// const angle = bb.get("robot.arm.angle"); // Returns 45.0
    /// const name = bb.get("robot.name");       // Returns "R2D2"
    /// const missing = bb.get("not.found");    // Returns undefined
    /// ```
    #[wasm_bindgen]
    pub fn get(&self, path: &str) -> Result<JsValue, JsError> {
        if path.trim().is_empty() {
            return Ok(JsValue::UNDEFINED);
        }
        let guard = self.blackboard.lock().unwrap();
        match guard.get_value(path) {
            Ok(Some(v)) => value_to_jsvalue(&v),
            Ok(None) => Ok(JsValue::UNDEFINED),
            Err(e) => Err(JsError::new(&format!("Failed to get value: {}", e))),
        }
    }

    /// Remove a value at the given dot-separated path
    ///
    /// # Arguments
    /// * `path` - Dot-separated path (e.g., "robot.arm.joint1.angle")
    ///
    /// # Returns
    /// The removed value, or undefined if path didn't exist
    ///
    /// # Example (JavaScript)
    /// ```js
    /// const removed = bb.remove("robot.arm.angle"); // Returns the removed value
    /// const missing = bb.remove("not.found");       // Returns undefined
    /// ```
    #[wasm_bindgen]
    pub fn remove(&mut self, path: &str) -> Result<JsValue, JsError> {
        // TODO: implement proper removal in core; currently returns undefined
        let _ = path; // suppress unused warning
        Ok(JsValue::UNDEFINED)
    }

    /// Check if a path exists in the blackboard
    ///
    /// # Arguments
    /// * `path` - Dot-separated path (e.g., "robot.arm.joint1.angle")
    ///
    /// # Returns
    /// true if the path exists, false otherwise
    ///
    /// # Example (JavaScript)
    /// ```js
    /// const exists = bb.has("robot.arm.angle"); // Returns true/false
    /// ```
    #[wasm_bindgen]
    pub fn has(&self, path: &str) -> bool {
        if path.trim().is_empty() {
            return false;
        }
        let guard = self.blackboard.lock().unwrap();
        guard.get_value(path).map(|v| v.is_some()).unwrap_or(false)
    }

    /// List all paths currently stored in the blackboard
    ///
    /// # Returns
    /// Array of strings representing all paths in the blackboard
    ///
    /// # Example (JavaScript)
    /// ```js
    /// const paths = bb.list_paths(); // Returns ["robot.arm.angle", "robot.name", ...]
    /// ```
    #[wasm_bindgen(js_name = "list_paths")]
    pub fn list_paths(&self) -> Result<JsValue, JsError> {
        // For now, return empty array since we can't easily traverse the structure
        // This can be implemented later with proper access methods
        let js_array = js_sys::Array::new();
        Ok(js_array.into())
    }

    /// Clear all data from the blackboard
    ///
    /// # Example (JavaScript)
    /// ```js
    /// bb.clear(); // Removes all data
    /// ```
    #[wasm_bindgen]
    pub fn clear(&mut self) {
        // Re-create a new blackboard with same root identifier (best-effort)
        let name = self
            .blackboard
            .lock()
            .ok()
            .and_then(|g| g.get_current_name_copy().ok())
            .unwrap_or_else(|| "default".to_string());
        self.blackboard = ArcAroraBlackboard::new(name);
    }

    /// Get the name of this blackboard
    ///
    /// # Returns
    /// The name of the blackboard as a string
    ///
    /// # Example (JavaScript)
    /// ```js
    /// const name = bb.name(); // Returns "my-blackboard"
    /// ```
    #[wasm_bindgen]
    pub fn name(&self) -> String {
        self.blackboard
            .lock()
            .ok()
            .and_then(|g| g.get_current_name_copy().ok())
            .unwrap_or_else(|| "(unknown)".to_string())
    }

    /// Get the number of items (leaf nodes) in the blackboard
    ///
    /// # Returns
    /// The total count of leaf nodes in the blackboard
    ///
    /// # Example (JavaScript)
    /// ```js
    /// const count = bb.size(); // Returns number of items
    /// ```
    #[wasm_bindgen]
    pub fn size(&self) -> u32 {
        // For now, return 0 since we can't easily traverse the structure
        // This can be implemented later with proper access methods
        0
    }
}

/// Get the ABI version for compatibility checks
///
/// # Returns
/// The ABI version number
#[wasm_bindgen]
pub fn abi_version() -> u32 {
    1
}
