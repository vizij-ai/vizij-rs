//! The registry of standard profiles — the composable graph assets that make
//! a face respond to an external standard's keys. One entry today (ROS4HRI);
//! the registry exists so hosts, the bundler, and authoring UIs can list what
//! a user may opt into, uniformly.

use serde_json::{json, Value as Json};

use crate::ros4hri;

/// The bundle graph kind under which a standard profile embeds in a GLB.
pub const STANDARD_PROFILE_KIND: &str = "standard-profile";

/// A standard profile: its identity and the asset behind it.
pub struct StandardProfile {
    /// Registry id, e.g. `ros4hri` — also names the profile everywhere a user
    /// opts in (CLI flags, bundle graph ids, npm lookups).
    pub id: &'static str,
    pub title: &'static str,
    pub description: &'static str,
    /// The canonical profile asset, unprefixed, verbatim JSON.
    pub asset_json: &'static str,
}

/// Every standard profile Vizij ships.
pub const STANDARD_PROFILES: [StandardProfile; 1] = [StandardProfile {
    id: "ros4hri",
    title: "ROS4HRI",
    description: "Drives the standard face controls from the standard/ros4hri/* keys: \
                  expression names and valence/arousal, gaze targets with vergence, FACS \
                  action units, visemes, idle blink, and the incumbent's ~200 ms smoothing.",
    asset_json: ros4hri::PROFILE_JSON,
}];

/// Look a profile up by id.
pub fn standard_profile(id: &str) -> Option<&'static StandardProfile> {
    STANDARD_PROFILES.iter().find(|p| p.id == id)
}

/// The bundle graph id under which `profile_id` embeds in a GLB — stable, so
/// re-adding replaces rather than duplicates.
pub fn embedded_graph_id(profile_id: &str) -> String {
    format!("standard::{profile_id}")
}

/// A profile's graph as a composable source, with the face's rig prefix
/// applied to the written control paths. `None` for an unknown id.
pub fn standard_profile_source(id: &str, rig_prefix: &str) -> Option<(String, Json)> {
    let profile = standard_profile(id)?;
    let mut spec: Json = serde_json::from_str(profile.asset_json).ok()?;
    ros4hri::apply_rig_prefix(&mut spec, rig_prefix);
    Some((id.to_string(), spec))
}

/// The registry as JSON — what CLIs print and the web runtime serves for
/// opt-in pickers.
pub fn standard_profiles_json() -> Json {
    Json::Array(
        STANDARD_PROFILES
            .iter()
            .map(|p| {
                json!({
                    "id": p.id,
                    "title": p.title,
                    "description": p.description,
                })
            })
            .collect(),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn registry_lists_ros4hri() {
        let listed = standard_profiles_json();
        assert_eq!(listed[0]["id"], "ros4hri");
        assert!(standard_profile("ros4hri").is_some());
        assert!(standard_profile("nope").is_none());
        assert_eq!(embedded_graph_id("ros4hri"), "standard::ros4hri");
    }

    #[test]
    fn profile_source_matches_the_ros4hri_source() {
        let via_registry = standard_profile_source("ros4hri", "rig/f/").expect("known id");
        let direct = ros4hri::ros4hri_source("rig/f/");
        assert_eq!(via_registry.1, direct.1);
    }
}
