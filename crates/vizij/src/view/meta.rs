//! Vizij face metadata, read from the GLB's JSON chunk.
//!
//! Bevy loads the same GLB for meshes/materials/morphs; this module reads what
//! Bevy's loader does not surface: the per-node `RobotData` glTF extension
//! (the animatables — UUID-identified features the runtime drives) and the
//! scene-root node's `VIZIJ_bundle` extension (the face's graphs, poses,
//! clips). The two worlds join on the glTF node name, which Bevy preserves as
//! the spawned entity's `Name`.

use std::collections::HashMap;

use anyhow::{bail, Context, Result};
use serde::Deserialize;
use serde_json::Value as Json;
use uuid::Uuid;
use vizij_arora_host::Bundle;

/// What one animated feature drives on a scene element.
#[derive(Debug, Clone, PartialEq)]
pub enum FeatureKind {
    /// `translation` — vector3 onto the node transform.
    Translation,
    /// `rotation` — euler (ZYX, three.js convention) onto the node transform.
    Rotation,
    /// `scale` — vector3 (or scalar broadcast) onto the node transform.
    Scale,
    /// `color` — rgb onto the node's material base color.
    Color,
    /// `opacity` — number onto the material; <1 enables alpha blending.
    Opacity,
    /// `metalness` — number onto the material: a metal has no diffuse term,
    /// so under the ambient-only model it darkens the base color to black.
    Metalness,
    /// `roughness` — number onto the material. Accepted for the standard's
    /// sake; with no direct light and no environment map it changes nothing.
    Roughness,
    /// `emissive` — rgb added to the material's output, times the intensity.
    Emissive,
    /// `emissiveIntensity` — number scaling the emissive color.
    EmissiveIntensity,
    /// A morph target influence, by target name (resolved to an index at join).
    Morph(String),
}

/// One binding: a store write to the animatable moves `feature` of the
/// element (glTF node) called `node_name`.
#[derive(Debug, Clone)]
pub struct Binding {
    pub node_name: String,
    pub feature: FeatureKind,
}

/// A scene element as declared by its `RobotData` extension.
#[derive(Debug, Clone)]
pub struct Element {
    /// The element's own id — what the authoring app and Studio name it by,
    /// and what a pick reports.
    pub id: Uuid,
    pub node_name: String,
    /// `shape` (mesh-bearing) or `group`; unused until the UI groups elements.
    #[allow(dead_code)]
    pub kind: String,
    /// Web material kind: `standard` (ambient-Lambert shaded) or `basic`
    /// (unlit — three's MeshBasicMaterial ignores lights, full albedo).
    pub material: Option<String>,
    /// Names of the element's morph targets, in glTF order.
    pub morph_targets: Vec<String>,
}

/// The face metadata joined from `RobotData` + `VIZIJ_bundle`. The `RobotData`
/// half (elements, animatables, bounds) drives the Bevy renderer; the bundle is
/// the portable host glue shared with the browser runtime.
#[derive(Debug, Clone, Default)]
pub struct FaceMeta {
    pub elements: Vec<Element>,
    /// animatable id → what it drives.
    pub animatables: HashMap<Uuid, Binding>,
    /// Authored view bounds on the root element: (center_x, center_y, size_x, size_y).
    pub root_bounds: Option<(f32, f32, f32, f32)>,
    /// The face's `VIZIJ_bundle` — its graphs, programs, and neutral pose. The
    /// device composition and neutral staging are its methods
    /// ([`Bundle::compose`], [`Bundle::neutral_stage_writes`]).
    pub bundle: Bundle,
}

/// Raw `RobotData` feature entry (only what the app needs).
#[derive(Deserialize)]
struct RawFeature {
    #[serde(default)]
    animated: bool,
    value: Option<RawAnimatable>,
}

#[derive(Deserialize)]
struct RawAnimatable {
    id: Uuid,
}

#[derive(Deserialize)]
struct RawRobotData {
    id: Uuid,
    #[serde(default)]
    name: String,
    material: Option<String>,
    #[serde(rename = "type", default)]
    kind: String,
    #[serde(rename = "morphTargets", default)]
    morph_targets: Vec<String>,
    #[serde(default)]
    features: HashMap<String, RawFeature>,
    #[serde(rename = "rootBounds")]
    root_bounds: Option<RawBounds>,
}

#[derive(Deserialize)]
struct RawBounds {
    center: RawVec2,
    size: RawVec2,
}

#[derive(Deserialize)]
struct RawVec2 {
    x: f32,
    y: f32,
}

impl FaceMeta {
    /// Read a face's metadata from its GLB bytes — the one way in, on every
    /// target; reading a file is the caller's.
    pub fn from_glb_bytes(bytes: &[u8]) -> Result<Self> {
        let json = glb_json_chunk(bytes)?;
        Self::from_gltf_json(&json)
    }

    fn from_gltf_json(gltf: &Json) -> Result<Self> {
        let nodes = gltf
            .get("nodes")
            .and_then(Json::as_array)
            .context("glTF has no nodes")?;

        let mut elements = Vec::new();
        let mut animatables = HashMap::new();
        let mut root_bounds = None;

        for node in nodes {
            let exts = node.get("extensions");
            let Some(rd) = exts.and_then(|e| e.get("RobotData")) else {
                continue;
            };
            let rd: RawRobotData = serde_json::from_value(rd.clone())
                .with_context(|| format!("bad RobotData on node {:?}", node.get("name")))?;
            // The glTF node name is the join key with the spawned Bevy scene.
            let node_name = node
                .get("name")
                .and_then(Json::as_str)
                .unwrap_or(&rd.name)
                .to_string();

            if let Some(b) = &rd.root_bounds {
                root_bounds = Some((b.center.x, b.center.y, b.size.x, b.size.y));
            }

            for (feature_name, feature) in &rd.features {
                if !feature.animated {
                    continue;
                }
                let Some(value) = &feature.value else {
                    continue;
                };
                let kind = match feature_name.as_str() {
                    "translation" => FeatureKind::Translation,
                    "rotation" => FeatureKind::Rotation,
                    "scale" => FeatureKind::Scale,
                    "color" => FeatureKind::Color,
                    "opacity" => FeatureKind::Opacity,
                    "metalness" => FeatureKind::Metalness,
                    "roughness" => FeatureKind::Roughness,
                    "emissive" => FeatureKind::Emissive,
                    "emissiveIntensity" => FeatureKind::EmissiveIntensity,
                    // Any other feature is a morph influence iff the node
                    // declares a morph target of that name.
                    other if rd.morph_targets.iter().any(|m| m == other) => {
                        FeatureKind::Morph(other.to_string())
                    }
                    other => {
                        log::debug!("{node_name}: unmapped feature {other:?} — skipped");
                        continue;
                    }
                };
                animatables.insert(
                    value.id,
                    Binding {
                        node_name: node_name.clone(),
                        feature: kind,
                    },
                );
            }

            elements.push(Element {
                id: rd.id,
                node_name,
                kind: rd.kind,
                material: rd.material,
                morph_targets: rd.morph_targets,
            });
        }

        // The `VIZIJ_bundle` half — graphs, programs, neutral pose — is the
        // portable host glue, read (and composed/staged) by the shared crate.
        let bundle = Bundle::from_gltf_json(gltf).unwrap_or_default();

        Ok(Self {
            elements,
            animatables,
            root_bounds,
            bundle,
        })
    }
}

/// Extracts the JSON chunk from a GLB container.
fn glb_json_chunk(bytes: &[u8]) -> Result<Json> {
    if bytes.len() < 20 || &bytes[0..4] != b"glTF" {
        bail!("not a GLB container");
    }
    let chunk_len = u32::from_le_bytes(bytes[12..16].try_into().unwrap()) as usize;
    if &bytes[16..20] != b"JSON" {
        bail!("first GLB chunk is not JSON");
    }
    if bytes.len() < 20 + chunk_len {
        bail!("GLB truncated: JSON chunk overruns the file");
    }
    serde_json::from_slice(&bytes[20..20 + chunk_len]).context("GLB JSON chunk does not parse")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_non_glb() {
        assert!(FaceMeta::from_glb_bytes(b"not a glb at all....").is_err());
    }

    /// Every material feature the authoring app publishes binds — the four
    /// beyond color and opacity included, since a face's look may live in
    /// them alone (a metallic plate reads black, its features are emissive).
    #[test]
    fn every_material_feature_binds() {
        let feature = |ty: &str, default: serde_json::Value| {
            serde_json::json!({
                "animated": true,
                "value": {
                    "id": Uuid::new_v4().to_string(),
                    "name": "x",
                    "type": ty,
                    "default": default,
                }
            })
        };
        let gltf = serde_json::json!({
            "nodes": [{
                "name": "Plate",
                "extensions": { "RobotData": {
                    "id": Uuid::new_v4().to_string(),
                    "name": "Plate",
                    "type": "shape",
                    "material": "standard",
                    "features": {
                        "color": feature("rgb", serde_json::json!({"r": 1, "g": 1, "b": 1})),
                        "opacity": feature("number", serde_json::json!(1)),
                        "metalness": feature("number", serde_json::json!(1)),
                        "roughness": feature("number", serde_json::json!(1)),
                        "emissive": feature("rgb", serde_json::json!({"r": 0.3, "g": 0.3, "b": 0})),
                        "emissiveIntensity": feature("number", serde_json::json!(1)),
                    }
                } }
            }]
        });
        let meta = FaceMeta::from_gltf_json(&gltf).expect("parses");
        let mut kinds: Vec<String> = meta
            .animatables
            .values()
            .map(|b| format!("{:?}", b.feature))
            .collect();
        kinds.sort();
        assert_eq!(
            kinds,
            [
                "Color",
                "Emissive",
                "EmissiveIntensity",
                "Metalness",
                "Opacity",
                "Roughness"
            ]
        );
    }
}
