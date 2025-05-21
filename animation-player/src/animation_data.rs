use std::collections::HashMap;

use serde::{Deserialize, Serialize};

use crate::value::Value;

/**
 * Animation keypoint for bridge transfer
 */
#[allow(non_snake_case)]
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AnimationKeypoint {
    pub id: String,
    pub stamp: f64,
    pub value: f64, // TODO change to value to properly support different types
    pub trackId: Option<String>,
}

/**
 * Animation track for bridge transfer
 */
#[allow(non_snake_case)]
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AnimationTrack {
    pub id: String,
    pub name: String,
    pub points: Vec<AnimationKeypoint>,
    pub animatableId: String,
    pub settings: Option<HashMap<String, Value>>,
}

/**
 * Animation transition for bridge transfer
 */
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AnimationTransition {
    pub id: String,
    pub keypoints: (String, String),
    pub variant: String,
    pub parameters: HashMap<String, String>,
}

/**
 * Complete animation data for transfer over the bridge
 */
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AnimationData {
    pub id: String,
    pub name: String,
    pub tracks: Vec<AnimationTrack>,
    pub transitions: HashMap<String, AnimationTransition>, //  Does this need to be optional?
    pub duration: f64,
}

/**
 * Incremental update to an animation
 */
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AnimationUpdate {
    pub id: String,
    pub name: Option<String>,
    pub tracks: Option<Vec<AnimationTrack>>,
    pub transitions: Option<HashMap<String, AnimationTransition>>,
    pub duration: Option<f64>,
    pub removed_track_ids: Option<Vec<String>>,
    pub removed_transition_ids: Option<Vec<String>>,
}
