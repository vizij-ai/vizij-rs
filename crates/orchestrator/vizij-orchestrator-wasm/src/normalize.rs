use vizij_api_core::json;

/// Normalize a full GraphSpec JSON string into a serde_json::Value with
/// all shorthand normalized.
pub fn normalize_graph_spec_json(json_str: &str) -> Result<String, String> {
    json::normalize_graph_spec_json_string(json_str).map_err(|e| e.to_string())
}
