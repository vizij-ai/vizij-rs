use vizij_api_core::json;

/// Normalize a GraphSpec JSON string into canonical JSON.
///
/// # Errors
/// Returns an error if the JSON cannot be parsed or normalized.
pub fn normalize_graph_spec_json(json_str: &str) -> Result<String, String> {
    json::normalize_graph_spec_json_string(json_str).map_err(|e| e.to_string())
}
