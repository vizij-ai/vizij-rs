//! Vizij face metadata, read from the GLB's JSON chunk.
//!
//! Bevy loads the same GLB for meshes/materials/morphs; this module reads what
//! Bevy's loader does not surface: the per-node `RobotData` glTF extension
//! (the animatables — UUID-identified features the runtime drives) and the
//! scene-root node's `VIZIJ_bundle` extension (the face's graphs, poses,
//! clips). The two worlds join on the glTF node name, which Bevy preserves as
//! the spawned entity's `Name`.

use std::collections::HashMap;
use std::path::Path;

use anyhow::{bail, Context, Result};
use serde::Deserialize;
use serde_json::Value as Json;

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

/// The face metadata joined from `RobotData` + `VIZIJ_bundle`.
#[derive(Debug)]
pub struct FaceMeta {
    pub elements: Vec<Element>,
    /// animatable UUID (string form) → what it drives.
    pub animatables: HashMap<String, Binding>,
    /// Authored view bounds on the root element: (center_x, center_y, size_x, size_y).
    pub root_bounds: Option<(f32, f32, f32, f32)>,
    /// Graph entries from the bundle: (kind, spec JSON).
    pub bundle_graphs: Vec<(String, Json)>,
    /// `poses.config.neutralInputs` — input id → neutral value; staged once
    /// input staging lands (the rig inputs all carry defaults meanwhile).
    #[allow(dead_code)]
    pub neutral_inputs: HashMap<String, f64>,
    /// Bundle metadata (faceId, activeMotionGraphId, …); read once program
    /// autoplay lands.
    #[allow(dead_code)]
    pub metadata: Json,
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
    id: String,
}

#[derive(Deserialize)]
struct RawRobotData {
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
    pub fn from_glb_file(path: &Path) -> Result<Self> {
        let bytes =
            std::fs::read(path).with_context(|| format!("cannot read GLB {}", path.display()))?;
        Self::from_glb_bytes(&bytes)
    }

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
        let mut bundle: Option<&Json> = None;

        for node in nodes {
            let exts = node.get("extensions");
            if bundle.is_none() {
                if let Some(b) = exts.and_then(|e| e.get("VIZIJ_bundle")) {
                    bundle = Some(b);
                }
            }
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
                    value.id.clone(),
                    Binding {
                        node_name: node_name.clone(),
                        feature: kind,
                    },
                );
            }

            elements.push(Element {
                node_name,
                kind: rd.kind,
                material: rd.material,
                morph_targets: rd.morph_targets,
            });
        }

        let mut bundle_graphs = Vec::new();
        let mut neutral_inputs = HashMap::new();
        let mut metadata = Json::Null;
        if let Some(bundle) = bundle {
            if let Some(graphs) = bundle.get("graphs").and_then(Json::as_array) {
                for entry in graphs {
                    let kind = entry
                        .get("kind")
                        .and_then(Json::as_str)
                        .unwrap_or("unknown")
                        .to_string();
                    if let Some(spec) = entry.get("spec") {
                        bundle_graphs.push((kind, spec.clone()));
                    }
                }
            }
            if let Some(neutral) = bundle
                .pointer("/poses/config/neutralInputs")
                .and_then(Json::as_object)
            {
                for (k, v) in neutral {
                    if let Some(n) = v.as_f64() {
                        neutral_inputs.insert(k.clone(), n);
                    }
                }
            }
            metadata = bundle.get("metadata").cloned().unwrap_or(Json::Null);
        }

        Ok(Self {
            elements,
            animatables,
            root_bounds,
            bundle_graphs,
            neutral_inputs,
            metadata,
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
}
