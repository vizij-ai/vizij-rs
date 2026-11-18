//! wasm-bindgen interface for [`vizij_import`], shipped as the
//! `@vizij/import` npm package so the authoring app can run import passes
//! in the browser before it parses a face.

use serde_json::{json, Value};
use vizij_import::morph_triangles::{Outcome, Report};
use wasm_bindgen::prelude::*;

/// The version of this interface: the exported functions and the report
/// JSON's shape. The npm wrapper refuses bindings reporting another
/// version, which catches a stale `pkg/` after a rebuild of one side only.
#[wasm_bindgen]
pub fn abi_version() -> u32 {
    1
}

/// The outcome of [`optimize_morph_triangles`].
#[wasm_bindgen]
pub struct MorphTrianglesResult {
    bytes: Option<Vec<u8>>,
    report: String,
}

#[wasm_bindgen]
impl MorphTrianglesResult {
    /// The rewritten GLB, or `undefined` when the pass changed nothing.
    #[wasm_bindgen(getter)]
    pub fn bytes(&self) -> Option<Vec<u8>> {
        self.bytes.clone()
    }

    /// The pass's report as JSON: `{ parts: [{ mesh, meshName, primitive,
    /// vertices, outcome, skipReason, folds: [{ shape, before, after }] }] }`,
    /// where `outcome` is `retriangulated`, `kept` or `skipped`, and a fold
    /// is the share of the shape's area drawn with reversed winding (`null`
    /// for a shape with no area).
    #[wasm_bindgen(getter, js_name = reportJson)]
    pub fn report_json(&self) -> String {
        self.report.clone()
    }
}

/// Re-triangulates every part of `glb` modelled as one large polygon so its
/// triangles stay unfolded across its morph targets; see
/// [`vizij_import::morph_triangles`].
#[wasm_bindgen(js_name = optimizeMorphTriangles)]
pub fn optimize_morph_triangles(glb: &[u8]) -> Result<MorphTrianglesResult, JsError> {
    let optimized =
        vizij_import::optimize_morph_triangles(glb).map_err(|e| JsError::new(&e.to_string()))?;
    Ok(MorphTrianglesResult {
        bytes: optimized.bytes,
        report: report_json(&optimized.report).to_string(),
    })
}

fn report_json(report: &Report) -> Value {
    let parts: Vec<Value> = report
        .parts
        .iter()
        .map(|part| {
            let (outcome, skip_reason) = match &part.outcome {
                Outcome::Retriangulated => ("retriangulated", None),
                Outcome::Kept => ("kept", None),
                Outcome::Skipped(why) => ("skipped", Some(why.to_string())),
            };
            let folds: Vec<Value> = part
                .folds
                .iter()
                .map(|f| json!({ "shape": f.shape, "before": f.before, "after": f.after }))
                .collect();
            json!({
                "mesh": part.mesh,
                "meshName": part.mesh_name,
                "primitive": part.primitive,
                "vertices": part.vertices,
                "outcome": outcome,
                "skipReason": skip_reason,
                "folds": folds,
            })
        })
        .collect();
    json!({ "parts": parts })
}

#[cfg(test)]
mod tests {
    use super::*;
    use vizij_import::morph_triangles::{Part, ShapeFold, Skip};

    #[test]
    fn the_report_serializes_in_the_documented_shape() {
        let report = Report {
            parts: vec![
                Part {
                    mesh: 2,
                    mesh_name: Some("mouth".into()),
                    primitive: 0,
                    vertices: 65,
                    outcome: Outcome::Retriangulated,
                    folds: vec![ShapeFold {
                        shape: "smile_shape".into(),
                        before: Some(0.4),
                        after: None,
                    }],
                },
                Part {
                    mesh: 3,
                    mesh_name: None,
                    primitive: 1,
                    vertices: 64,
                    outcome: Outcome::Skipped(Skip::NotAPolygon),
                    folds: Vec::new(),
                },
            ],
        };
        assert_eq!(
            report_json(&report),
            json!({ "parts": [
                {
                    "mesh": 2, "meshName": "mouth", "primitive": 0, "vertices": 65,
                    "outcome": "retriangulated", "skipReason": null,
                    "folds": [{ "shape": "smile_shape", "before": 0.4, "after": null }]
                },
                {
                    "mesh": 3, "meshName": null, "primitive": 1, "vertices": 64,
                    "outcome": "skipped",
                    "skipReason": "is not a single polygon (vertices inside its outline, a hole, or several outlines)",
                    "folds": []
                }
            ]})
        );
    }
}
