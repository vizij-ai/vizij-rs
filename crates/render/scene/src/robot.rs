//! Reading the robot's own metadata out of the GLB.
//!
//! Bevy's glTF loader parses the extensions it knows about and drops the rest,
//! and `RobotData` is an extension rather than `extras` — so none of it
//! reaches the spawned scene. The screen is the sharpest case: it declares a
//! size and a place and carries no mesh at all, so without reading the JSON
//! there is nothing in the world to find.
//!
//! The face crate meets the same problem for its own metadata and answers it
//! the same way, which is why `glb_json_chunk` is shared from there rather
//! than written again here.

use anyhow::{anyhow, Result};
use bevy::prelude::*;
use serde_json::Value as Json;
use vizij_render::meta::glb_json_chunk;

/// A screen the robot declares: a size in metres and a place to put it, with
/// no geometry of its own. Whoever renders it builds the quad.
#[derive(Debug, Clone)]
pub struct Screen {
    /// The glTF node that declares the screen. The quad belongs *under* this
    /// node rather than at its world position: a screen is mounted on a robot
    /// that moves, so it has to inherit whatever the joints above it do.
    pub node: usize,
    /// Where the screen sits once its ancestors have had their say. Reported,
    /// not used to place the quad.
    pub transform: Transform,
    pub width: f32,
    pub height: f32,
    /// Pixels per metre.
    pub resolution: f32,
}

impl Screen {
    /// The render target's size, as Studio sizes it: metres times pixels per
    /// metre, in `packages/scene/src/renderables/screen.tsx`.
    pub fn texture_size(&self) -> (u32, u32) {
        (
            (self.width * self.resolution).round().max(1.0) as u32,
            (self.height * self.resolution).round().max(1.0) as u32,
        )
    }
}

/// How a joint moves, which decides what kind of control it takes.
///
/// A platform is free to use any of these on any link, so nothing downstream
/// may assume a joint turns — that assumption is what made a sliding joint
/// silently rotate.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum JointKind {
    /// Turns about its axis, within limits.
    Revolute,
    /// Turns about its axis without end.
    Continuous,
    /// Slides along its axis, within limits.
    Prismatic,
    /// Does not move; carries its child along.
    Fixed,
}

impl JointKind {
    fn parse(kind: &str) -> Option<Self> {
        Some(match kind {
            "revolute" => Self::Revolute,
            "continuous" => Self::Continuous,
            "prismatic" => Self::Prismatic,
            "fixed" => Self::Fixed,
            _ => return None,
        })
    }

    /// Whether a person can drive it at all.
    pub fn is_drivable(&self) -> bool {
        !matches!(self, Self::Fixed)
    }

    /// Whether its value is an angle. The alternative is a distance, and the
    /// two are not interchangeable: 0.09 is a twentieth of a turn or nine
    /// centimetres depending on this.
    pub fn is_angular(&self) -> bool {
        matches!(self, Self::Revolute | Self::Continuous)
    }

    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Revolute => "revolute",
            Self::Continuous => "continuous",
            Self::Prismatic => "prismatic",
            Self::Fixed => "fixed",
        }
    }
}

/// A joint the robot declares, with the range its value is allowed to take.
#[derive(Debug, Clone)]
pub struct Joint {
    pub name: String,
    pub kind: JointKind,
    pub axis: Vec3,
    /// The glTF node index of the body this joint moves. Bevy names an unnamed
    /// glTF node `GltfNode{index}`, which is the only join back to the spawned
    /// scene — a robot's nodes carry no names of their own.
    pub child_node: usize,
    pub min: f32,
    pub max: f32,
    pub default: f32,
    /// The colour the author gave this joint's control.
    pub color: Option<Srgba>,
}

impl Joint {
    /// Whether the joint's value is bounded at all. A `continuous` joint turns
    /// forever and a `fixed` one does not turn, so neither takes a limited
    /// control.
    pub fn is_limited(&self) -> bool {
        self.min.is_finite() && self.max.is_finite() && self.max > self.min
    }
}

/// Finds every joint the robot declares, resolved to the nodes they move.
pub fn find_joints(bytes: &[u8]) -> Result<Vec<Joint>> {
    let gltf = glb_json_chunk(bytes)?;
    let nodes = gltf
        .get("nodes")
        .and_then(Json::as_array)
        .ok_or_else(|| anyhow!("glTF has no nodes"))?;

    let mut parent = vec![usize::MAX; nodes.len()];
    let mut by_id = std::collections::HashMap::new();
    for (index, node) in nodes.iter().enumerate() {
        if let Some(id) = node
            .pointer("/extensions/RobotData/id")
            .and_then(Json::as_str)
        {
            by_id.insert(id.to_string(), index);
        }
        let Some(children) = node.get("children").and_then(Json::as_array) else {
            continue;
        };
        for child in children.iter().filter_map(Json::as_u64) {
            parent[child as usize] = index;
        }
    }

    let mut joints = Vec::new();
    for node in nodes {
        let Some(data) = node.pointer("/extensions/RobotData") else {
            continue;
        };
        let Some(kind) = data
            .get("type")
            .and_then(Json::as_str)
            .and_then(JointKind::parse)
        else {
            continue;
        };
        let Some(&child_node) = data
            .get("child")
            .and_then(Json::as_str)
            .and_then(|id| by_id.get(id))
        else {
            continue;
        };

        let axis = data
            .get("axis")
            .map(|a| {
                let at = |k: &str| a.get(k).and_then(Json::as_f64).unwrap_or_default() as f32;
                Vec3::new(at("x"), at("y"), at("z"))
            })
            .unwrap_or(Vec3::Y);

        let value = data.pointer("/features/jointValue/value");
        let number = |path: &str| {
            value
                .and_then(|v| v.pointer(path))
                .and_then(Json::as_f64)
                .map(|v| v as f32)
        };
        joints.push(Joint {
            name: data
                .get("name")
                .and_then(Json::as_str)
                .unwrap_or("(unnamed)")
                .to_string(),
            kind,
            axis: axis.normalize_or(Vec3::Y),
            child_node,
            min: number("/constraints/min").unwrap_or(f32::NEG_INFINITY),
            max: number("/constraints/max").unwrap_or(f32::INFINITY),
            default: number("/default").unwrap_or_default(),
            color: value
                .and_then(|v| v.pointer("/pub/color"))
                .and_then(Json::as_str)
                .and_then(|hex| Srgba::hex(hex).ok()),
        });
    }
    Ok(joints)
}

/// Composes a node's transform with every ancestor's, root first.
fn world_transform(nodes: &[Json], parent: &[usize], index: usize) -> Transform {
    let mut chain = vec![index];
    let mut current = index;
    while parent[current] != usize::MAX {
        current = parent[current];
        chain.push(current);
    }
    let mut transform = Transform::IDENTITY;
    for ancestor in chain.into_iter().rev() {
        transform = transform * node_transform(&nodes[ancestor]);
    }
    transform
}

/// Finds the screen a robot declares, and where it ends up once its ancestors
/// have had their say.
pub fn find_screen(bytes: &[u8]) -> Result<Screen> {
    let gltf = glb_json_chunk(bytes)?;
    let nodes = gltf
        .get("nodes")
        .and_then(Json::as_array)
        .ok_or_else(|| anyhow!("glTF has no nodes"))?;

    let mut parent = vec![usize::MAX; nodes.len()];
    for (index, node) in nodes.iter().enumerate() {
        let Some(children) = node.get("children").and_then(Json::as_array) else {
            continue;
        };
        for child in children.iter().filter_map(Json::as_u64) {
            parent[child as usize] = index;
        }
    }

    let (index, data) = nodes
        .iter()
        .enumerate()
        .find_map(|(index, node)| {
            let data = node.pointer("/extensions/RobotData")?;
            (data.get("type")?.as_str()? == "screen").then_some((index, data))
        })
        .ok_or_else(|| anyhow!("no node declares a screen"))?;

    let number = |key: &str| data.get(key).and_then(Json::as_f64).map(|v| v as f32);
    Ok(Screen {
        node: index,
        transform: world_transform(nodes, &parent, index),
        width: number("width").ok_or_else(|| anyhow!("screen has no width"))?,
        height: number("height").ok_or_else(|| anyhow!("screen has no height"))?,
        resolution: number("resolution").unwrap_or(1080.0),
    })
}

/// A glTF node's own transform, given either way the format allows.
fn node_transform(node: &Json) -> Transform {
    if let Some(matrix) = node.get("matrix").and_then(Json::as_array) {
        if matrix.len() == 16 {
            let mut columns = [0.0f32; 16];
            for (slot, value) in columns.iter_mut().zip(matrix) {
                *slot = value.as_f64().unwrap_or_default() as f32;
            }
            return Transform::from_matrix(Mat4::from_cols_array(&columns));
        }
    }
    Transform {
        translation: read_vec(node.get("translation")).unwrap_or(Vec3::ZERO),
        rotation: node
            .get("rotation")
            .and_then(Json::as_array)
            .filter(|values| values.len() == 4)
            .map(|values| {
                let at = |i: usize| values[i].as_f64().unwrap_or_default() as f32;
                Quat::from_xyzw(at(0), at(1), at(2), at(3))
            })
            .unwrap_or(Quat::IDENTITY),
        scale: read_vec(node.get("scale")).unwrap_or(Vec3::ONE),
    }
}

fn read_vec(value: Option<&Json>) -> Option<Vec3> {
    let values = value?.as_array()?;
    if values.len() != 3 {
        return None;
    }
    let at = |i: usize| values[i].as_f64().unwrap_or_default() as f32;
    Some(Vec3::new(at(0), at(1), at(2)))
}
