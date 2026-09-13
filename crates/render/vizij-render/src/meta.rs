//! Vizij face metadata, read from the GLB's JSON chunk.
//!
//! Bevy loads the same GLB for meshes/materials/morphs; this module reads what
//! Bevy's loader does not surface: the per-node `RobotData` glTF extension —
//! the animatables, UUID-identified features a runtime drives. The two worlds
//! join on the glTF node name, which Bevy preserves as the spawned entity's
//! `Name`.

use std::collections::HashMap;
use std::path::Path;

use anyhow::{bail, Context, Result};
use serde::{Deserialize, Serialize};
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

/// The face metadata joined from `RobotData` + `VIZIJ_bundle`. The `RobotData`
/// What the renderer needs from a face GLB: the drawable elements, the
/// animatable bindings that address them, and the authored view bounds.
///
/// A GLB's other Vizij payload — the `VIZIJ_bundle` extension carrying graphs,
/// programs and the neutral pose — is a host concern; read it from the same
/// JSON with [`glb_json_chunk`].
#[derive(Debug, Clone)]
pub struct FaceMeta {
    pub elements: Vec<Element>,
    /// animatable UUID (string form) → everything it drives.
    ///
    /// A list, because one animatable may address several elements at once: a
    /// face binds one colour to the inner face and all four eyelids. Keyed to
    /// a single binding, the last element parsed would win and the rest would
    /// never move.
    pub animatables: HashMap<String, Vec<Binding>>,
    /// Authored view bounds on the root element: (center_x, center_y, size_x, size_y).
    pub root_bounds: Option<(f32, f32, f32, f32)>,
}

/// One animatable, described for a host that has to present it.
///
/// The animatable UUID alone is unusable in an interface — it says nothing
/// about what it drives. This pairs it with the element and the feature, which
/// is what a host needs to label a control, group controls by element, or pick
/// a sensible range for one.
#[derive(Clone, Debug, Serialize)]
pub struct AnimatableInfo {
    /// The UUID a value is written under.
    pub id: String,
    /// The element this drives.
    pub node: String,
    /// One of `translation`, `rotation`, `scale`, `color`, `opacity`, `morph`.
    pub feature: &'static str,
    /// The morph target's name, when `feature` is `morph`.
    pub morph_target: Option<String>,
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

    /// Every animatable, described for presentation, ordered by element then
    /// feature so a host can group them without sorting again.
    pub fn animatable_info(&self) -> Vec<AnimatableInfo> {
        let mut infos: Vec<AnimatableInfo> = self
            .animatables
            .iter()
            .flat_map(|(id, bindings)| bindings.iter().map(move |binding| (id, binding)))
            .map(|(id, binding)| {
                let (feature, morph_target) = match &binding.feature {
                    FeatureKind::Translation => ("translation", None),
                    FeatureKind::Rotation => ("rotation", None),
                    FeatureKind::Scale => ("scale", None),
                    FeatureKind::Color => ("color", None),
                    FeatureKind::Opacity => ("opacity", None),
                    FeatureKind::Morph(target) => ("morph", Some(target.clone())),
                };
                AnimatableInfo {
                    id: id.clone(),
                    node: binding.node_name.clone(),
                    feature,
                    morph_target,
                }
            })
            .collect();
        infos.sort_by(|a, b| {
            (&a.node, a.feature, &a.morph_target).cmp(&(&b.node, b.feature, &b.morph_target))
        });
        infos
    }

    /// Reads the render half out of an already-parsed glTF document.
    pub fn from_gltf_json(gltf: &Json) -> Result<Self> {
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
                animatables
                    .entry(value.id.clone())
                    .or_insert_with(Vec::new)
                    .push(Binding {
                        node_name: node_name.clone(),
                        feature: kind,
                    });
            }

            elements.push(Element {
                node_name,
                kind: rd.kind,
                material: rd.material,
                morph_targets: rd.morph_targets,
            });
        }

        Ok(Self {
            elements,
            animatables,
            root_bounds,
        })
    }
}

/// Extracts the JSON chunk from a GLB container.
pub fn glb_json_chunk(bytes: &[u8]) -> Result<Json> {
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
