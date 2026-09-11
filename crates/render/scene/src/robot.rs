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
    /// Where the screen sits, in the glTF's own coordinates.
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

    // Compose root-first, so each transform applies inside its parent's frame.
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

    let number = |key: &str| data.get(key).and_then(Json::as_f64).map(|v| v as f32);
    Ok(Screen {
        transform,
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
