//! The JSON-chunk migration: walks a glTF document and rewrites the Value
//! payloads it carries into canonical arora `Value` serde.
//!
//! Two extensions carry Values:
//!
//! - `scenes[*].extensions.VIZIJ_bundle` and `nodes[*].extensions.
//!   VIZIJ_bundle`: each `graphs[*].spec` / `graphs[*].ir` entry (an inline
//!   object or an embedded JSON string) is a node-graph document, normalized
//!   with [`vizij_api_core::json::normalize_graph_spec_value`]; inline edge
//!   `default` / `default_value` payloads (which that normalizer does not
//!   cover) are normalized value-by-value. The bundle's `poses.config` and
//!   animation keyframe values are plain scalars and are not visited.
//! - `nodes[*].extensions.RobotData`: each `features.<id>.value.default`
//!   payload is a web raw value (bare primitive, `{x,y[,z]}` components,
//!   `{r,g,b[,a]}` color). `default` is the only Value-bearing field of a
//!   feature `value`; the sibling `constraints` hold plain numeric bounds
//!   and are not visited.
//!
//! Unrecognized payloads are left unchanged and reported as warnings.

use serde_json::Value as Json;
use thiserror::Error;
use vizij_api_core::json::{normalize_graph_spec_value, parse_value, JsonError};

#[derive(Debug, Error)]
pub enum MigrateError {
    #[error("{context}: {source}")]
    GraphSpec {
        context: String,
        #[source]
        source: JsonError,
    },
    #[error("{context}: embedded graph document is not valid JSON: {source}")]
    GraphString {
        context: String,
        #[source]
        source: serde_json::Error,
    },
}

/// What a migration pass did (or would do) to one glTF JSON document.
#[derive(Debug, Default)]
pub struct Report {
    pub graph_docs_seen: usize,
    pub graph_docs_changed: usize,
    pub robot_defaults_seen: usize,
    pub robot_defaults_changed: usize,
    /// Value payloads that could not be recognized and were left unchanged.
    pub warnings: Vec<String>,
}

impl Report {
    pub fn changed(&self) -> bool {
        self.graph_docs_changed > 0 || self.robot_defaults_changed > 0
    }

    pub fn summary(&self) -> String {
        format!(
            "{}/{} graph document(s), {}/{} RobotData default(s)",
            self.graph_docs_changed,
            self.graph_docs_seen,
            self.robot_defaults_changed,
            self.robot_defaults_seen
        )
    }
}

/// Migrate one glTF JSON document in place.
pub fn migrate_gltf_json(root: &mut Json) -> Result<Report, MigrateError> {
    let mut report = Report::default();
    for scope in ["scenes", "nodes"] {
        let Some(items) = root.get_mut(scope).and_then(Json::as_array_mut) else {
            continue;
        };
        for (index, item) in items.iter_mut().enumerate() {
            let context = format!("{scope}[{index}]");
            if let Some(bundle) = item.pointer_mut("/extensions/VIZIJ_bundle") {
                migrate_bundle(bundle, &context, &mut report)?;
            }
            if scope == "nodes" {
                if let Some(robot_data) = item.pointer_mut("/extensions/RobotData") {
                    migrate_robot_data(robot_data, &context, &mut report);
                }
            }
        }
    }
    Ok(report)
}

fn migrate_bundle(
    bundle: &mut Json,
    context: &str,
    report: &mut Report,
) -> Result<(), MigrateError> {
    let Some(graphs) = bundle.get_mut("graphs").and_then(Json::as_array_mut) else {
        return Ok(());
    };
    for (index, graph) in graphs.iter_mut().enumerate() {
        for key in ["spec", "ir"] {
            let Some(doc) = graph.get_mut(key) else {
                continue;
            };
            if doc.is_null() {
                continue;
            }
            let doc_context = format!("{context}.extensions.VIZIJ_bundle.graphs[{index}].{key}");
            report.graph_docs_seen += 1;
            if let Json::String(text) = doc {
                let mut parsed: Json =
                    serde_json::from_str(text).map_err(|source| MigrateError::GraphString {
                        context: doc_context.clone(),
                        source,
                    })?;
                let before = parsed.clone();
                normalize_graph_doc(&mut parsed, &doc_context, report)?;
                if parsed != before {
                    *doc = Json::String(
                        serde_json::to_string(&parsed)
                            .expect("a graph document re-serializes to JSON"),
                    );
                    report.graph_docs_changed += 1;
                }
            } else {
                let before = doc.clone();
                normalize_graph_doc(doc, &doc_context, report)?;
                if *doc != before {
                    report.graph_docs_changed += 1;
                }
            }
        }
    }
    Ok(())
}

fn normalize_graph_doc(
    doc: &mut Json,
    context: &str,
    report: &mut Report,
) -> Result<(), MigrateError> {
    if !doc.is_object() {
        report.warnings.push(format!(
            "{context}: graph document is not a JSON object; left unchanged"
        ));
        return Ok(());
    }
    normalize_graph_spec_value(doc).map_err(|source| MigrateError::GraphSpec {
        context: context.to_string(),
        source,
    })?;
    // Inline edge defaults predate the `input_defaults` split and are not
    // covered by the spec normalizer.
    if let Some(edges) = doc.get_mut("edges").and_then(Json::as_array_mut) {
        for (index, edge) in edges.iter_mut().enumerate() {
            for key in ["default", "default_value"] {
                if let Some(payload) = edge.get_mut(key) {
                    if !payload.is_null() {
                        normalize_value_payload(
                            payload,
                            &format!("{context}.edges[{index}].{key}"),
                            report,
                        );
                    }
                }
            }
        }
    }
    Ok(())
}

fn migrate_robot_data(robot_data: &mut Json, context: &str, report: &mut Report) {
    let Some(features) = robot_data.get_mut("features") else {
        return;
    };
    match features {
        Json::Object(map) => {
            for (key, feature) in map.iter_mut() {
                let feature_context = format!("{context}.extensions.RobotData.features.{key}");
                migrate_feature(feature, &feature_context, report);
            }
        }
        Json::Array(items) => {
            for (index, feature) in items.iter_mut().enumerate() {
                let feature_context = format!("{context}.extensions.RobotData.features[{index}]");
                migrate_feature(feature, &feature_context, report);
            }
        }
        _ => {}
    }
}

fn migrate_feature(feature: &mut Json, context: &str, report: &mut Report) {
    let Some(default) = feature.pointer_mut("/value/default") else {
        return;
    };
    if default.is_null() {
        return;
    }
    report.robot_defaults_seen += 1;
    if normalize_value_payload(default, &format!("{context}.value.default"), report) {
        report.robot_defaults_changed += 1;
    }
}

/// Rewrite one Value payload to canonical arora serde in place. Returns
/// whether it changed; unrecognized payloads are reported and left as they
/// are.
fn normalize_value_payload(payload: &mut Json, context: &str, report: &mut Report) -> bool {
    match parse_value(payload.clone()) {
        Ok(value) => {
            let canonical =
                serde_json::to_value(&value).expect("an arora Value serializes to JSON");
            if canonical != *payload {
                *payload = canonical;
                true
            } else {
                false
            }
        }
        Err(error) => {
            report.warnings.push(format!(
                "{context}: unrecognized value payload left unchanged ({error})"
            ));
            false
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use vizij_api_core::value::{as_color_rgba, as_vec3, VEC3_TYPE};
    use vizij_api_core::Value;

    fn sample_gltf() -> Json {
        json!({
            "asset": { "version": "2.0" },
            "scenes": [{
                "nodes": [0],
                "extensions": { "VIZIJ_bundle": {
                    "graphs": [{
                        "id": "g1",
                        "spec": {
                            "nodes": [
                                {
                                    "id": "const",
                                    "type": "constant",
                                    "params": { "value": { "float": 1.0 } }
                                },
                                {
                                    "id": "sum",
                                    "type": "add",
                                    "input_defaults": {
                                        "operand_1": { "value": { "vec3": [1, 2, 3] } }
                                    }
                                }
                            ],
                            "edges": [{
                                "from": { "node_id": "const" },
                                "to": { "node_id": "sum", "input": "operand_2" },
                                "default": { "float": 0.5 }
                            }]
                        }
                    }],
                    "poses": { "config": { "neutral": 0.25 } },
                    "animations": [{
                        "clip": { "tracks": [{
                            "keyframes": [{ "stamp": 0.0, "value": 0.5 }]
                        }]}
                    }]
                }}
            }],
            "nodes": [{
                "name": "face",
                "extensions": { "RobotData": {
                    "id": "robot",
                    "features": {
                        "brightness": {
                            "animated": true,
                            "value": {
                                "id": "b", "type": "number",
                                "default": 0.75,
                                "constraints": { "min": 0.0, "max": 1.0 }
                            }
                        },
                        "gaze": {
                            "value": {
                                "id": "g", "type": "vector3",
                                "default": { "x": 1.0, "y": 2.0, "z": 3.0 },
                                "constraints": {}
                            }
                        },
                        "tint": {
                            "value": {
                                "id": "t", "type": "rgb",
                                "default": { "r": 0.25, "g": 0.5, "b": 0.75 },
                                "constraints": {}
                            }
                        }
                    }
                }}
            }]
        })
    }

    fn value_at(root: &Json, pointer: &str) -> Value {
        serde_json::from_value(root.pointer(pointer).expect(pointer).clone())
            .unwrap_or_else(|e| panic!("{pointer}: not a canonical Value: {e}"))
    }

    #[test]
    fn rewrites_bundle_graph_values() {
        let mut root = sample_gltf();
        let report = migrate_gltf_json(&mut root).expect("migrate");
        assert_eq!(report.graph_docs_seen, 1);
        assert_eq!(report.graph_docs_changed, 1);
        assert!(report.warnings.is_empty(), "{:?}", report.warnings);

        let spec = "/scenes/0/extensions/VIZIJ_bundle/graphs/0/spec";
        assert_eq!(
            root.pointer(&format!("{spec}/nodes/0/params/value")),
            Some(&json!({ "f32": 1.0 }))
        );
        let operand = root
            .pointer(&format!("{spec}/nodes/1/input_defaults/operand_1/value"))
            .expect("operand default");
        assert_eq!(operand["struct"]["id"], VEC3_TYPE.to_string());
        assert_eq!(
            root.pointer(&format!("{spec}/edges/0/default")),
            Some(&json!({ "f32": 0.5 }))
        );
    }

    #[test]
    fn leaves_poses_and_animation_keyframes_alone() {
        let mut root = sample_gltf();
        migrate_gltf_json(&mut root).expect("migrate");
        let bundle = "/scenes/0/extensions/VIZIJ_bundle";
        assert_eq!(
            root.pointer(&format!("{bundle}/poses/config/neutral")),
            Some(&json!(0.25))
        );
        assert_eq!(
            root.pointer(&format!(
                "{bundle}/animations/0/clip/tracks/0/keyframes/0/value"
            )),
            Some(&json!(0.5))
        );
    }

    #[test]
    fn rewrites_robot_data_defaults() {
        let mut root = sample_gltf();
        let report = migrate_gltf_json(&mut root).expect("migrate");
        assert_eq!(report.robot_defaults_seen, 3);
        assert_eq!(report.robot_defaults_changed, 3);

        let features = "/nodes/0/extensions/RobotData/features";
        assert_eq!(
            root.pointer(&format!("{features}/brightness/value/default")),
            Some(&json!({ "f32": 0.75 }))
        );
        assert_eq!(
            as_vec3(&value_at(&root, &format!("{features}/gaze/value/default"))),
            Some([1.0, 2.0, 3.0])
        );
        assert_eq!(
            as_color_rgba(&value_at(&root, &format!("{features}/tint/value/default"))),
            Some([0.25, 0.5, 0.75, 1.0])
        );
        // Constraints are not Values and stay as they are.
        assert_eq!(
            root.pointer(&format!("{features}/brightness/value/constraints")),
            Some(&json!({ "min": 0.0, "max": 1.0 }))
        );
    }

    #[test]
    fn string_graph_documents_are_reserialized() {
        let spec = r#"{"nodes":[{"id":"c","type":"constant","params":{"value":{"float":2.0}}}]}"#;
        let mut root = json!({
            "nodes": [{
                "extensions": { "VIZIJ_bundle": {
                    "graphs": [{ "id": "g", "spec": spec }]
                }}
            }]
        });
        let report = migrate_gltf_json(&mut root).expect("migrate");
        assert_eq!(report.graph_docs_changed, 1);

        let stored = root
            .pointer("/nodes/0/extensions/VIZIJ_bundle/graphs/0/spec")
            .and_then(Json::as_str)
            .expect("spec stays a string");
        let parsed: Json = serde_json::from_str(stored).expect("stored spec parses");
        assert_eq!(
            parsed.pointer("/nodes/0/params/value"),
            Some(&json!({ "f32": 2.0 }))
        );
        assert_eq!(parsed.pointer("/edges"), Some(&json!([])));
    }

    #[test]
    fn migration_is_idempotent() {
        let mut root = sample_gltf();
        migrate_gltf_json(&mut root).expect("first pass");
        let migrated = root.clone();
        let report = migrate_gltf_json(&mut root).expect("second pass");
        assert!(!report.changed(), "second pass must be a no-op");
        assert_eq!(root, migrated);
    }

    #[test]
    fn unrecognized_defaults_warn_and_pass_through() {
        let mut root = json!({
            "nodes": [{
                "extensions": { "RobotData": {
                    "features": {
                        "hue": {
                            "value": {
                                "id": "h", "type": "hsl",
                                "default": { "h": 0.1, "s": 0.2, "l": 0.3 },
                                "constraints": {}
                            }
                        }
                    }
                }}
            }]
        });
        let report = migrate_gltf_json(&mut root).expect("migrate");
        assert_eq!(report.robot_defaults_seen, 1);
        assert_eq!(report.robot_defaults_changed, 0);
        assert!(!report.changed());
        assert_eq!(report.warnings.len(), 1);
        assert!(report.warnings[0].contains("features.hue"));
        assert_eq!(
            root.pointer("/nodes/0/extensions/RobotData/features/hue/value/default"),
            Some(&json!({ "h": 0.1, "s": 0.2, "l": 0.3 }))
        );
    }

    #[test]
    fn rejects_legacy_links_field_with_context() {
        let mut root = json!({
            "scenes": [{
                "extensions": { "VIZIJ_bundle": {
                    "graphs": [{ "spec": { "nodes": [], "links": [] } }]
                }}
            }]
        });
        let error = migrate_gltf_json(&mut root).expect_err("links must be rejected");
        let message = error.to_string();
        assert!(message.contains("scenes[0]"), "{message}");
        assert!(message.contains("links"), "{message}");
    }
}
