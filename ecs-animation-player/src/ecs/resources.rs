use super::path::BevyPath;
use crate::{AnimationData, Value};
use bevy::prelude::*;
use std::collections::HashMap;

/// A global resource that stores the final computed animation values at the end of each frame.
#[derive(Resource, Default)]
pub struct AnimationOutput {
    pub values: HashMap<String, HashMap<String, Value>>,
}

/// A frame-local cache used to accumulate weighted values for blending before they are applied.
#[derive(Resource, Default)]
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

/// External tick-based time resource:
/// - delta_seconds: The externally provided delta for the current tick (consumed each frame)
/// - elapsed_seconds: The accumulated elapsed time from all applied deltas
#[derive(Resource, Debug, Clone, Copy)]
pub struct EngineTime {
    pub delta_seconds: f64,
    pub elapsed_seconds: f64,
}

impl Default for EngineTime {
    fn default() -> Self {
        EngineTime {
            delta_seconds: 0.0,
            elapsed_seconds: 0.0,
        }
    }
}
