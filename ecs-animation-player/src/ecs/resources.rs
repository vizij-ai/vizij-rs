use crate::{AnimationData, Value};
use bevy::prelude::*;
use super::path::BevyPath;
use std::collections::HashMap;

/// A global resource that stores the final computed animation values at the end of each frame.
#[derive(Resource, Default)]
pub struct AnimationOutput {
    pub values: HashMap<String, HashMap<String, Value>>,
}

/// A frame-local cache used to accumulate weighted values for blending before they are applied.
#[derive(Default)]
pub struct FrameBlendData {
    pub blended_values: HashMap<(Entity, BevyPath), Vec<(f32, Value)>>,
}

/// A resource to bridge the gap between Wasm string IDs and Bevy Entity IDs.
#[derive(Resource, Default)]
pub struct IdMapping {
    pub players: HashMap<String, Entity>,
    pub instances: HashMap<String, Entity>,
    pub animations: HashMap<String, Handle<AnimationData>>,
}
