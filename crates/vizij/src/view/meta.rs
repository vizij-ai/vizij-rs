//! Vizij face metadata, read from the GLB's JSON chunk.
//!
//! Bevy loads the same GLB for meshes/materials/morphs; this module reads what
//! Bevy's loader does not surface: the per-node `RobotData` glTF extension
//! (the animatables — UUID-identified features the runtime drives) and the
//! scene-root node's `VIZIJ_bundle` extension (the face's graphs, poses,
//! animations). The two worlds join on the glTF node name, which Bevy
//! preserves as the spawned entity's `Name`.
//!
//! RobotData comes in two feature models, both read here. The web's
//! (vizij-web's authoring app) names a feature whole — `translation`,
//! `rotation`, `color` — driven by one compound animatable. Semio Studio's
//! names it per axis — `translation.x`, `rotation.r`, `color.g` — each axis
//! its own scalar animatable, or a component `<id>.<axis>` of a compound
//! animatable `<id>` it split on load ([`Binding::axis`]).

use std::collections::HashMap;

use anyhow::{bail, Context, Result};
use bevy::math::{Mat4, Quat, Vec3};
use serde::Deserialize;
use serde_json::Value as Json;
use uuid::Uuid;
use vizij_arora_host::Bundle;

/// What one animated feature drives on a scene element.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
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
    /// `shininess` — number onto a `phong` material. Accepted for the web's
    /// sake; its specular highlight needs a direct light, so under the
    /// ambient-only model it changes nothing.
    Shininess,
    /// `specular` — rgb onto a `phong` material; changes nothing, as
    /// [`FeatureKind::Shininess`].
    Specular,
    /// A morph target influence, by target name (resolved to an index at join).
    Morph(String),
}

impl FeatureKind {
    /// The feature's name in RobotData — what [`FaceMeta`] parsed it from;
    /// a morph's is its target's.
    pub fn name(&self) -> &str {
        match self {
            FeatureKind::Translation => "translation",
            FeatureKind::Rotation => "rotation",
            FeatureKind::Scale => "scale",
            FeatureKind::Color => "color",
            FeatureKind::Opacity => "opacity",
            FeatureKind::Metalness => "metalness",
            FeatureKind::Roughness => "roughness",
            FeatureKind::Emissive => "emissive",
            FeatureKind::EmissiveIntensity => "emissiveIntensity",
            FeatureKind::Shininess => "shininess",
            FeatureKind::Specular => "specular",
            FeatureKind::Morph(target) => target,
        }
    }

    /// The names of the feature's three components in Studio's per-axis
    /// model, in the order of the triple the feature takes; `None` for a
    /// scalar feature.
    pub fn axes(&self) -> Option<[&'static str; 3]> {
        match self {
            FeatureKind::Translation | FeatureKind::Scale => Some(["x", "y", "z"]),
            // Roll, pitch, yaw: the euler's x, y and z.
            FeatureKind::Rotation => Some(["r", "p", "y"]),
            FeatureKind::Color | FeatureKind::Emissive | FeatureKind::Specular => {
                Some(["r", "g", "b"])
            }
            _ => None,
        }
    }

    /// The feature and, for a per-axis name (`translation.x`, `rotation.r`,
    /// `color.g`), the index of the component it names. A whole name wins
    /// over a split one, so a morph target whose name holds a dot stays a
    /// morph.
    pub fn parse(name: &str, morph_targets: &[String]) -> Option<(Self, Option<usize>)> {
        if let Some(kind) = Self::from_name(name, morph_targets) {
            return Some((kind, None));
        }
        let (base, axis) = name.split_once('.')?;
        let kind = Self::from_name(base, &[])?;
        let index = kind.axes()?.iter().position(|a| *a == axis)?;
        Some((kind, Some(index)))
    }

    /// The feature RobotData names `name` on an element whose morph targets
    /// are `morph_targets`: a name that is none of the transform or material
    /// features is a morph influence iff the element declares a target of
    /// that name.
    pub fn from_name(name: &str, morph_targets: &[String]) -> Option<Self> {
        Some(match name {
            "translation" => FeatureKind::Translation,
            "rotation" => FeatureKind::Rotation,
            "scale" => FeatureKind::Scale,
            "color" => FeatureKind::Color,
            "opacity" => FeatureKind::Opacity,
            "metalness" => FeatureKind::Metalness,
            "roughness" => FeatureKind::Roughness,
            "emissive" => FeatureKind::Emissive,
            "emissiveIntensity" => FeatureKind::EmissiveIntensity,
            "shininess" => FeatureKind::Shininess,
            "specular" => FeatureKind::Specular,
            other if morph_targets.iter().any(|m| m == other) => {
                FeatureKind::Morph(other.to_string())
            }
            _ => return None,
        })
    }
}

/// One binding: a store write to the animatable moves `feature` of the
/// element (glTF node) called `node_name` — the whole feature, or the one
/// component `axis` names.
#[derive(Debug, Clone)]
pub struct Binding {
    pub node_name: String,
    pub feature: FeatureKind,
    /// The component a scalar animatable drives alone, by its index in the
    /// feature's triple: Studio's per-axis features (`translation.x`). `None`
    /// for an animatable that carries the whole feature.
    pub axis: Option<usize>,
    /// The value the animatable takes and where it rests, as RobotData
    /// states its default.
    pub rest: Rest,
}

/// An animatable's value shape and resting value, from its RobotData
/// default: one number, or the three components of a compound feature in
/// the order of its axes ([`FeatureKind::axes`]). This is also how Studio
/// writes it: a number under the animatable's id, a compound per component,
/// under `<id>.<axis>`.
///
/// A default that states no number for a component rests it at 0, as Studio
/// does when it splits a compound animatable.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Rest {
    Scalar(f32),
    Components([f32; 3]),
}

impl Binding {
    /// The feature's name in RobotData: `translation`, or `translation.x`
    /// for a per-axis binding.
    pub fn feature_name(&self) -> String {
        match (self.axis, self.feature.axes()) {
            (Some(i), Some(axes)) => format!("{}.{}", self.feature.name(), axes[i]),
            _ => self.feature.name().to_string(),
        }
    }
}

/// A scene element as declared by its `RobotData` extension.
#[derive(Debug, Clone)]
pub struct Element {
    /// The element's own id — what the authoring app and Studio name it by,
    /// and what a pick reports.
    pub id: Uuid,
    pub node_name: String,
    /// `shape` (mesh-bearing) or `group` — an `ellipse` or `rectangle` reads
    /// as the `shape` it converts to ([`Element::primitive`]); unused until
    /// the UI groups elements.
    #[allow(dead_code)]
    pub kind: String,
    /// Web material kind: `standard`, `phong` or `lambert` (shaded by the
    /// ambient light), or `basic` (unlit — three's MeshBasicMaterial ignores
    /// lights, full albedo).
    pub material: Option<String>,
    /// Names of the element's morph targets, in glTF order.
    pub morph_targets: Vec<String>,
    /// For an element converted from an `ellipse` or `rectangle`: the unit
    /// mesh it is drawn with where its node carries none.
    pub primitive: Option<Primitive>,
}

/// The unit mesh of an element converted from a RobotData `ellipse` or
/// `rectangle` — kinds whose geometry the web renderer generates rather than
/// reads from the GLB — scaled on X and Y by its `width` and `height`, as the
/// web renderer scales its own.
///
/// Its look is what its RobotData states — a static feature's value or an
/// animated one's default — since the GLB carries no material for a mesh it
/// does not carry.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Primitive {
    pub shape: PrimitiveShape,
    /// `width` and `height`; 1 where unstated.
    pub size: [f32; 2],
    /// `fillColor`, linear RGB; white where unstated.
    pub fill: [f32; 3],
    /// `fillOpacity`; 1 where unstated.
    pub opacity: f32,
}

/// The shape of a [`Primitive`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PrimitiveShape {
    /// A circle of radius 1 in the XY plane, from an `ellipse`.
    Circle,
    /// A 1×1 square in the XY plane, from a `rectangle`.
    Plane,
}

impl PrimitiveShape {
    fn of(kind: &str) -> Option<Self> {
        match kind {
            "ellipse" => Some(PrimitiveShape::Circle),
            "rectangle" => Some(PrimitiveShape::Plane),
            _ => None,
        }
    }
}

/// The features of an `ellipse` or `rectangle` that are not a shape's, read
/// as the shape's they amount to: the size is the scale on X and Y, the fill
/// is the material. `None` for a feature the conversion drops.
fn primitive_feature(name: &str) -> Option<Option<(FeatureKind, Option<usize>)>> {
    Some(match name {
        "width" => Some((FeatureKind::Scale, Some(0))),
        "height" => Some((FeatureKind::Scale, Some(1))),
        "fillColor" => Some((FeatureKind::Color, None)),
        "fillOpacity" => Some((FeatureKind::Opacity, None)),
        // The stroke is a line around the fill; a shape has none.
        "strokeColor" | "strokeOpacity" | "strokeWidth" | "strokeOffset" => None,
        _ => return None,
    })
}

/// The face metadata joined from `RobotData` + `VIZIJ_bundle`. The `RobotData`
/// half (elements, animatables, bounds) drives the Bevy renderer; the bundle is
/// the portable host glue shared with the browser runtime.
#[derive(Debug, Clone, Default)]
pub struct FaceMeta {
    pub elements: Vec<Element>,
    /// animatable id → what it drives.
    pub animatables: HashMap<Uuid, Binding>,
    /// The view bounds: (center_x, center_y, size_x, size_y). The authored
    /// `rootBounds` of the root element, or for a GLB that declares none, the
    /// bounding box of its scene in the XY plane ([`scene_bounds`]) — what
    /// the web's import derives for it.
    pub root_bounds: Option<(f32, f32, f32, f32)>,
    /// The RobotData animatables bound to nothing the view draws (a joint's
    /// value, a stroke), by the keys Studio writes them under: a number's
    /// id, and each component of a vector, euler or colour, `<id>.<axis>`.
    pub unbound: Vec<String>,
    /// The face's `VIZIJ_bundle` — its graphs, programs, and neutral pose. The
    /// device composition and neutral staging are its methods
    /// ([`Bundle::compose`], [`Bundle::neutral_stage_writes`]).
    pub bundle: Bundle,
}

/// Raw `RobotData` feature entry (only what the app needs): an animated
/// feature's value is its animatable (`{ id, … }`), a static feature's is the
/// value itself (a number, `{x, y, z}`, `{r, g, b}`, …), which the GLB's
/// node and material already carry.
#[derive(Deserialize)]
struct RawFeature {
    #[serde(default)]
    animated: bool,
    value: Option<Json>,
}

impl RawFeature {
    /// The id of the animatable an animated feature is driven through.
    fn animatable(&self) -> Option<&str> {
        if !self.animated {
            return None;
        }
        self.value.as_ref()?.get("id")?.as_str()
    }

    /// The value a static feature states, or an animated one's default.
    fn stated(&self) -> Option<&Json> {
        let value = self.value.as_ref()?;
        if self.animated {
            value.get("default")
        } else {
            Some(value)
        }
    }

    /// The keys Studio writes the animatable `id` of this feature under:
    /// the id of a number, one key per component of a vector, euler or
    /// colour, which Studio splits by the animatable's type on load.
    fn studio_keys(&self, id: &str) -> Vec<String> {
        let ty = self.value.as_ref().and_then(|v| v.get("type")?.as_str());
        let components: &[&str] = match ty {
            Some("vector2") => &["x", "y"],
            Some("vector3") => &["x", "y", "z"],
            Some("euler") => &["r", "p", "y"],
            Some("rgb") => &["r", "g", "b"],
            Some("hsl") => &["h", "s", "l"],
            _ => return vec![id.to_string()],
        };
        components.iter().map(|c| format!("{id}.{c}")).collect()
    }

    fn number(&self) -> Option<f32> {
        self.stated()?.as_f64().map(|n| n as f32)
    }

    /// Where the animatable this feature is driven through rests: a number,
    /// or — for an animatable carrying a compound feature whole, `axes` its
    /// components — the components its default names. The animatable's
    /// `type` decides when the default is absent: Studio splits a `vector3`,
    /// `euler` or `rgb` animatable per component. A whole rotation's default
    /// may name its components `x`, `y`, `z` rather than roll, pitch, yaw.
    fn rest(&self, axes: Option<[&str; 3]>) -> Rest {
        let Some(axes) = axes else {
            return Rest::Scalar(self.number().unwrap_or(0.0));
        };
        match self.stated() {
            Some(Json::Object(components)) => {
                let names = if components.contains_key(axes[0]) {
                    axes
                } else {
                    ["x", "y", "z"]
                };
                let component = |i: usize| {
                    components
                        .get(names[i])
                        .and_then(Json::as_f64)
                        .map_or(0.0, |n| n as f32)
                };
                Rest::Components([component(0), component(1), component(2)])
            }
            None if matches!(
                self.value.as_ref().and_then(|v| v.get("type")?.as_str()),
                Some("vector3" | "euler" | "rgb")
            ) =>
            {
                Rest::Components([0.0; 3])
            }
            _ => Rest::Scalar(self.number().unwrap_or(0.0)),
        }
    }

    /// An `{r, g, b}` color.
    fn rgb(&self) -> Option<[f32; 3]> {
        let value = self.stated()?;
        let c = |k: &str| value.get(k)?.as_f64().map(|n| n as f32);
        Some([c("r")?, c("g")?, c("b")?])
    }
}

#[derive(Deserialize)]
struct RawRobotData {
    id: Uuid,
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
        let mut unbound = Vec::new();
        let mut root_bounds = None;

        for (index, node) in nodes.iter().enumerate() {
            let exts = node.get("extensions");
            let Some(rd) = exts.and_then(|e| e.get("RobotData")) else {
                continue;
            };
            let rd: RawRobotData = serde_json::from_value(rd.clone())
                .with_context(|| format!("bad RobotData on node {:?}", node.get("name")))?;
            // The glTF node name is the join key with the spawned Bevy scene;
            // Bevy's glTF loader names an unnamed node `GltfNode<index>`.
            let node_name = node
                .get("name")
                .and_then(Json::as_str)
                .map_or_else(|| format!("GltfNode{index}"), str::to_string);

            if let Some(b) = &rd.root_bounds {
                root_bounds = Some((b.center.x, b.center.y, b.size.x, b.size.y));
            }

            let shape = PrimitiveShape::of(&rd.kind);
            for (feature_name, feature) in &rd.features {
                let animated = feature.animatable();
                let parsed = match shape.and_then(|_| primitive_feature(feature_name)) {
                    Some(Some(converted)) => Some(converted),
                    Some(None) => {
                        log::warn!(
                            "{node_name}: {} stroke feature {feature_name:?} dropped — \
                             an ellipse or rectangle draws as a shape, which has no stroke",
                            rd.kind
                        );
                        unbound.extend(
                            animated
                                .map(|id| feature.studio_keys(id))
                                .unwrap_or_default(),
                        );
                        continue;
                    }
                    None => FeatureKind::parse(feature_name, &rd.morph_targets),
                };
                let Some(id) = animated else {
                    continue;
                };
                let Some((kind, axis)) = parsed else {
                    log::debug!("{node_name}: unmapped feature {feature_name:?} — skipped");
                    unbound.extend(feature.studio_keys(id));
                    continue;
                };
                let named_axis = axis;
                let Some((animatable, axis)) = animatable_of(id, &kind, axis) else {
                    log::debug!(
                        "{node_name}: {feature_name:?} names no animatable ({id:?}) — skipped"
                    );
                    unbound.extend(feature.studio_keys(id));
                    continue;
                };
                // A component `<id>.<axis>` of a compound animatable states
                // that one component's rest; the binding its siblings made
                // holds the others.
                let rest = match (axis, named_axis) {
                    (None, Some(component)) => {
                        let mut components = match animatables.get(&animatable) {
                            Some(Binding {
                                rest: Rest::Components(components),
                                ..
                            }) => *components,
                            _ => [0.0; 3],
                        };
                        components[component] = feature.number().unwrap_or(0.0);
                        Rest::Components(components)
                    }
                    (None, None) => feature.rest(kind.axes()),
                    (Some(_), _) => feature.rest(None),
                };
                animatables.insert(
                    animatable,
                    Binding {
                        node_name: node_name.clone(),
                        feature: kind,
                        axis,
                        rest,
                    },
                );
            }

            let primitive = shape.map(|shape| {
                let number = |name: &str| rd.features.get(name).and_then(RawFeature::number);
                Primitive {
                    shape,
                    size: [
                        number("width").unwrap_or(1.0),
                        number("height").unwrap_or(1.0),
                    ],
                    fill: rd
                        .features
                        .get("fillColor")
                        .and_then(RawFeature::rgb)
                        .unwrap_or([1.0; 3]),
                    opacity: number("fillOpacity").unwrap_or(1.0),
                }
            });
            elements.push(Element {
                id: rd.id,
                node_name,
                kind: if primitive.is_some() {
                    "shape".to_string()
                } else {
                    rd.kind
                },
                material: rd.material,
                morph_targets: rd.morph_targets,
                primitive,
            });
        }
        let root_bounds = root_bounds.or_else(|| scene_bounds(gltf));

        // The `VIZIJ_bundle` half — graphs, programs, neutral pose — is the
        // portable host glue, read (and composed/staged) by the shared crate.
        let bundle = Bundle::from_gltf_json(gltf).unwrap_or_default();

        unbound.sort();
        unbound.dedup();
        Ok(Self {
            elements,
            animatables,
            unbound,
            root_bounds,
            bundle,
        })
    }
}

/// The animatable a feature's animatable `id` names, and the component of
/// the feature it drives alone. A UUID is the animatable itself, driving the
/// whole feature or, for a per-axis feature, its `axis`. Studio's split of a
/// compound animatable `<uuid>` names its components `<uuid>.<axis>`: those
/// bind the compound animatable to the whole feature, the key a device writes.
fn animatable_of(
    id: &str,
    kind: &FeatureKind,
    axis: Option<usize>,
) -> Option<(Uuid, Option<usize>)> {
    if let Ok(uuid) = Uuid::parse_str(id) {
        return Some((uuid, axis));
    }
    let (compound, component) = id.split_once('.')?;
    let named = kind.axes()?[axis?];
    (component == named)
        .then(|| Uuid::parse_str(compound).ok())
        .flatten()
        .map(|uuid| (uuid, None))
}

/// The bounding box in the XY plane of every mesh of the GLB's first scene:
/// `(center_x, center_y, size_x, size_y)`, each size at least 1e-3. Measured
/// as the web's import does (three's `Box3.setFromObject` over what its glTF
/// loader read): each primitive's POSITION bounds, grown on every axis by the
/// largest displacement any of its morph targets declares, transformed by
/// its node's world matrix. `None` for a scene without a mesh.
fn scene_bounds(gltf: &Json) -> Option<(f32, f32, f32, f32)> {
    let nodes = gltf.get("nodes")?.as_array()?;
    let meshes = gltf.get("meshes").and_then(Json::as_array);
    let accessors = gltf.get("accessors").and_then(Json::as_array);
    let accessor_bounds = |index: &Json| -> Option<(Vec3, Vec3)> {
        let accessor = accessors?.get(usize::try_from(index.as_u64()?).ok()?)?;
        let vec3 = |key: &str| -> Option<Vec3> {
            let v = accessor.get(key)?.as_array()?;
            let c = |i: usize| v.get(i).and_then(Json::as_f64).map(|x| x as f32);
            Some(Vec3::new(c(0)?, c(1)?, c(2)?))
        };
        Some((vec3("min")?, vec3("max")?))
    };
    let local_matrix = |node: &Json| -> Mat4 {
        let floats = |key: &str| -> Option<Vec<f32>> {
            node.get(key)?
                .as_array()?
                .iter()
                .map(|x| x.as_f64().map(|x| x as f32))
                .collect()
        };
        if let Some(m) = floats("matrix").filter(|m| m.len() == 16) {
            return Mat4::from_cols_slice(&m);
        }
        let t = floats("translation").filter(|v| v.len() == 3);
        let r = floats("rotation").filter(|v| v.len() == 4);
        let s = floats("scale").filter(|v| v.len() == 3);
        Mat4::from_scale_rotation_translation(
            s.map_or(Vec3::ONE, |s| Vec3::from_slice(&s)),
            r.map_or(Quat::IDENTITY, |r| Quat::from_slice(&r)),
            t.map_or(Vec3::ZERO, |t| Vec3::from_slice(&t)),
        )
    };

    let scene = gltf
        .get("scenes")?
        .as_array()?
        .first()?
        .get("nodes")?
        .as_array()?;
    let mut stack: Vec<(usize, Mat4)> = scene
        .iter()
        .filter_map(|n| Some((usize::try_from(n.as_u64()?).ok()?, Mat4::IDENTITY)))
        .collect();
    let (mut min, mut max) = (Vec3::splat(f32::INFINITY), Vec3::splat(f32::NEG_INFINITY));
    while let Some((index, parent)) = stack.pop() {
        let Some(node) = nodes.get(index) else {
            continue;
        };
        let world = parent * local_matrix(node);
        let primitives = node
            .get("mesh")
            .and_then(Json::as_u64)
            .and_then(|m| meshes?.get(usize::try_from(m).ok()?))
            .and_then(|mesh| mesh.get("primitives")?.as_array());
        for primitive in primitives.into_iter().flatten() {
            let Some((lo, hi)) = primitive
                .get("attributes")
                .and_then(|a| a.get("POSITION"))
                .and_then(accessor_bounds)
            else {
                continue;
            };
            let displacement = primitive
                .get("targets")
                .and_then(Json::as_array)
                .into_iter()
                .flatten()
                .filter_map(|target| accessor_bounds(target.get("POSITION")?))
                .fold(Vec3::ZERO, |d, (tlo, thi)| d.max(tlo.abs().max(thi.abs())));
            let (lo, hi) = (lo - displacement, hi + displacement);
            for corner in 0..8 {
                let pick = |bit: usize, l: f32, h: f32| if corner & bit == 0 { l } else { h };
                let p = world.transform_point3(Vec3::new(
                    pick(1, lo.x, hi.x),
                    pick(2, lo.y, hi.y),
                    pick(4, lo.z, hi.z),
                ));
                min = min.min(p);
                max = max.max(p);
            }
        }
        for child in node
            .get("children")
            .and_then(Json::as_array)
            .into_iter()
            .flatten()
        {
            if let Some(child) = child.as_u64().and_then(|c| usize::try_from(c).ok()) {
                stack.push((child, world));
            }
        }
    }
    if !(min.x.is_finite() && min.y.is_finite() && max.x.is_finite() && max.y.is_finite()) {
        return None;
    }
    let center = (min + max) / 2.0;
    let size = (max - min).abs().max(Vec3::splat(1e-3));
    Some((center.x, center.y, size.x, size.y))
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

    /// A static feature carries its value, not an animatable: the element
    /// reads, and only its animated features bind.
    #[test]
    fn a_static_feature_reads_and_binds_nothing() {
        let animated = Uuid::new_v4();
        let gltf = serde_json::json!({
            "nodes": [{
                "name": "Plate",
                "extensions": { "RobotData": {
                    "id": Uuid::new_v4().to_string(),
                    "name": "Plate",
                    "type": "shape",
                    "features": {
                        "color": { "animated": false, "value": { "r": 1, "g": 0.5, "b": 0 } },
                        "opacity": { "animated": false, "value": 0.5 },
                        "translation": {
                            "animated": true,
                            "value": { "id": animated.to_string(), "type": "vector3" }
                        },
                    }
                } }
            }]
        });
        let meta = FaceMeta::from_gltf_json(&gltf).expect("parses");
        assert_eq!(meta.elements.len(), 1);
        assert_eq!(meta.animatables.len(), 1);
        assert_eq!(
            meta.animatables[&animated].feature,
            FeatureKind::Translation
        );
    }

    /// A Studio export names its features per axis. An axis driven by an
    /// animatable of its own binds that one component; an axis whose
    /// animatable is a component `<id>.<axis>` of a compound animatable
    /// binds the compound one to the whole feature. An unnamed node joins
    /// under the name Bevy's loader gives it.
    #[test]
    fn studio_per_axis_features_bind_their_components() {
        let (x, roll, compound) = (Uuid::new_v4(), Uuid::new_v4(), Uuid::new_v4());
        let axis = |id: String| {
            serde_json::json!({
                "animated": true,
                "value": { "id": id, "type": "number", "default": 0 }
            })
        };
        let gltf = serde_json::json!({
            "nodes": [{}, {
                "extensions": { "RobotData": {
                    "id": Uuid::new_v4().to_string(),
                    "name": "Head",
                    "type": "body",
                    "features": {
                        "translation.x": axis(x.to_string()),
                        "translation.y": { "animated": false, "value": 0.5 },
                        "rotation.r": axis(roll.to_string()),
                        "color.r": axis(format!("{compound}.r")),
                        "color.g": axis(format!("{compound}.g")),
                        "color.b": axis(format!("{compound}.b")),
                        // A component that is not the feature's own axis.
                        "translation.z": axis(format!("{compound}.x")),
                        "jointValue": axis(Uuid::new_v4().to_string()),
                    }
                } }
            }]
        });
        let meta = FaceMeta::from_gltf_json(&gltf).expect("parses");
        assert_eq!(meta.elements[0].node_name, "GltfNode1");
        let bound = |id: &Uuid| {
            let b = &meta.animatables[id];
            (b.feature.clone(), b.axis, b.feature_name())
        };
        assert_eq!(
            bound(&x),
            (FeatureKind::Translation, Some(0), "translation.x".into())
        );
        assert_eq!(
            bound(&roll),
            (FeatureKind::Rotation, Some(0), "rotation.r".into())
        );
        assert_eq!(bound(&compound), (FeatureKind::Color, None, "color".into()));
        assert_eq!(meta.animatables.len(), 3);
        // What Studio writes but nothing draws: the joint's value, and the
        // component that is not its feature's own axis.
        let mut unbound = vec![format!("{compound}.x")];
        let joint = gltf["nodes"][1]["extensions"]["RobotData"]["features"]["jointValue"]["value"]
            ["id"]
            .as_str()
            .unwrap()
            .to_string();
        unbound.push(joint);
        unbound.sort();
        assert_eq!(meta.unbound, unbound);
    }

    /// Each animatable rests where its RobotData default says, in the shape
    /// Studio writes it: a number, or the components of a compound feature —
    /// gathered from the per-axis features of a split compound, read by
    /// axis name from a whole one's default (`x`/`y`/`z` standing for roll,
    /// pitch and yaw), zero where nothing states one.
    #[test]
    fn rests_follow_the_robotdata_defaults() {
        let ids: Vec<Uuid> = (0..7).map(|_| Uuid::new_v4()).collect();
        let animated = |id: String, ty: &str, default: Option<serde_json::Value>| {
            let mut value = serde_json::json!({ "id": id, "type": ty });
            if let Some(default) = default {
                value["default"] = default;
            }
            serde_json::json!({ "animated": true, "value": value })
        };
        let n = |x: f64| Some(serde_json::json!(x));
        let gltf = serde_json::json!({
            "nodes": [{
                "name": "Plate",
                "extensions": { "RobotData": {
                    "id": Uuid::new_v4().to_string(),
                    "type": "shape",
                    "morphTargets": ["smile"],
                    "features": {
                        "translation.x": animated(ids[0].to_string(), "number", n(0.5)),
                        "color.r": animated(format!("{}.r", ids[1]), "number", n(0.1)),
                        "color.g": animated(format!("{}.g", ids[1]), "number", n(0.2)),
                        "color.b": animated(format!("{}.b", ids[1]), "number", None),
                        "rotation": animated(ids[2].to_string(), "euler",
                            Some(serde_json::json!({ "x": 0.1, "y": 0.2, "z": 0.3 }))),
                        "scale": animated(ids[3].to_string(), "number", n(2.0)),
                        "emissive": animated(ids[4].to_string(), "rgb", None),
                        "specular": animated(ids[6].to_string(), "rgb",
                            Some(serde_json::json!({ "r": 0.4, "g": 0.5, "b": 0.6 }))),
                        "smile": animated(ids[5].to_string(), "number", n(0.25)),
                    }
                } }
            }]
        });
        let meta = FaceMeta::from_gltf_json(&gltf).expect("parses");
        let rest = |i: usize| meta.animatables[&ids[i]].rest;
        assert_eq!(rest(0), Rest::Scalar(0.5));
        assert_eq!(rest(1), Rest::Components([0.1, 0.2, 0.0]));
        assert_eq!(rest(2), Rest::Components([0.1, 0.2, 0.3]));
        assert_eq!(rest(3), Rest::Scalar(2.0));
        assert_eq!(rest(4), Rest::Components([0.0; 3]));
        assert_eq!(rest(5), Rest::Scalar(0.25));
        assert_eq!(rest(6), Rest::Components([0.4, 0.5, 0.6]));
    }

    /// A whole feature name wins over a split one, so a morph target named
    /// with a dot stays a morph; a split name takes only its feature's axes.
    #[test]
    fn a_feature_name_parses_whole_first() {
        let morphs = ["mouth.open".to_string()];
        assert_eq!(
            FeatureKind::parse("mouth.open", &morphs),
            Some((FeatureKind::Morph("mouth.open".into()), None))
        );
        assert_eq!(
            FeatureKind::parse("rotation.y", &[]),
            Some((FeatureKind::Rotation, Some(2)))
        );
        assert_eq!(
            FeatureKind::parse("scale.z", &[]),
            Some((FeatureKind::Scale, Some(2)))
        );
        assert_eq!(FeatureKind::parse("rotation.x", &[]), None);
        assert_eq!(FeatureKind::parse("opacity.r", &[]), None);
        assert_eq!(FeatureKind::parse("color.h", &[]), None);
    }

    /// An ellipse or rectangle reads as a shape drawn with its unit mesh:
    /// its size is the scale on X and Y, its fill the material, its stroke
    /// dropped.
    #[test]
    fn an_ellipse_converts_to_a_shape() {
        let (width, fill, stroke) = (Uuid::new_v4(), Uuid::new_v4(), Uuid::new_v4());
        let animated = |id: Uuid, default: serde_json::Value| serde_json::json!({ "animated": true, "value": { "id": id.to_string(), "default": default } });
        let gltf = serde_json::json!({
            "nodes": [{
                "name": "Eye",
                "extensions": { "RobotData": {
                    "id": Uuid::new_v4().to_string(),
                    "type": "ellipse",
                    "features": {
                        "width": animated(width, serde_json::json!(0.4)),
                        "height": { "animated": false, "value": 0.2 },
                        "fillColor": animated(fill, serde_json::json!({"r": 1, "g": 0, "b": 0})),
                        "strokeColor": animated(stroke, serde_json::json!({"r": 0, "g": 0, "b": 0})),
                        "strokeWidth": { "animated": false, "value": 2 },
                    }
                } }
            }]
        });
        let meta = FaceMeta::from_gltf_json(&gltf).expect("parses");
        let eye = &meta.elements[0];
        assert_eq!(eye.kind, "shape");
        assert_eq!(
            eye.primitive,
            Some(Primitive {
                shape: PrimitiveShape::Circle,
                size: [0.4, 0.2],
                fill: [1.0, 0.0, 0.0],
                opacity: 1.0,
            })
        );
        assert_eq!(meta.animatables[&width].feature, FeatureKind::Scale);
        assert_eq!(meta.animatables[&width].axis, Some(0));
        assert_eq!(meta.animatables[&fill].feature, FeatureKind::Color);
        assert!(!meta.animatables.contains_key(&stroke));
    }

    /// A GLB declaring no rootBounds is framed on its scene's bounding box,
    /// as the web's import derives it: each primitive's position bounds,
    /// grown by its largest morph displacement, through its node's world
    /// transform.
    #[test]
    fn bounds_derive_from_the_scene_without_root_bounds() {
        let gltf = serde_json::json!({
            "scenes": [{ "nodes": [0] }],
            "nodes": [
                { "translation": [10.0, 0.0, 0.0], "scale": [2.0, 2.0, 2.0], "children": [1] },
                { "mesh": 0, "translation": [0.0, 1.0, 0.0] },
            ],
            "meshes": [{ "primitives": [{
                "attributes": { "POSITION": 0 },
                "targets": [{ "POSITION": 1 }],
            }] }],
            "accessors": [
                { "min": [-1.0, -0.5, 0.0], "max": [1.0, 0.5, 0.0] },
                { "min": [0.0, -0.25, 0.0], "max": [0.0, 0.0, 0.0] },
            ],
        });
        let meta = FaceMeta::from_gltf_json(&gltf).expect("parses");
        // Local x ∈ [-1, 1], y ∈ [0.25, 1.75] (grown by 0.25 about the
        // translated center); then scaled by 2 and moved 10 along X.
        assert_eq!(meta.root_bounds, Some((10.0, 2.0, 4.0, 3.0)));

        let empty = serde_json::json!({ "scenes": [{ "nodes": [0] }], "nodes": [{}] });
        assert_eq!(FaceMeta::from_gltf_json(&empty).unwrap().root_bounds, None);
    }
}
