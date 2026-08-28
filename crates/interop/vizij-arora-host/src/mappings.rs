//! The registry of standard **mappings** — the composable graph assets that
//! carry one profile's keys onto another's, so a face responds to an external
//! standard. One entry today (ROS4HRI): it reads the `ros4hri` profile and
//! writes the `vizij-face` profile, both declared in [`crate::profile`].
//!
//! A mapping is the second half of a standard; the first is the profile that
//! names and types the keys. The registry exists so hosts, the bundler, and
//! authoring UIs can list what a face may opt into, uniformly.

use serde_json::{json, Value as Json};

use crate::ros4hri;

/// The bundle graph kind under which a standard mapping embeds in a GLB.
///
/// The value stays `standard-profile` deliberately. Faces in the wild already
/// carry that string, and the authoring app already writes it, so renaming the
/// wire format would strand every exported GLB. The Rust name says what the
/// entry *is*; changing what is written needs a read-both migration first.
pub const STANDARD_MAPPING_KIND: &str = "standard-profile";

/// A standard profile: its identity and the asset behind it.
pub struct StandardMapping {
    /// Registry id, e.g. `ros4hri` — also names the profile everywhere a user
    /// opts in (CLI flags, bundle graph ids, npm lookups).
    pub id: &'static str,
    pub title: &'static str,
    pub description: &'static str,
    /// The canonical profile asset, unprefixed, verbatim JSON.
    pub asset_json: &'static str,
}

/// Every standard profile Vizij ships.
pub const STANDARD_MAPPINGS: [StandardMapping; 1] = [StandardMapping {
    id: "ros4hri",
    title: "ROS4HRI",
    description: "Drives the standard face controls from the standard/ros4hri/* keys: \
                  expression names and valence/arousal, gaze targets with vergence, FACS \
                  action units, visemes, idle blink, and the incumbent's ~200 ms smoothing.",
    asset_json: ros4hri::MAPPING_JSON,
}];

/// Look a profile up by id.
pub fn standard_mapping(id: &str) -> Option<&'static StandardMapping> {
    STANDARD_MAPPINGS.iter().find(|p| p.id == id)
}

/// The bundle graph id under which `profile_id` embeds in a GLB — stable, so
/// re-adding replaces rather than duplicates.
pub fn embedded_graph_id(profile_id: &str) -> String {
    format!("standard::{profile_id}")
}

/// A profile's graph as a composable source, with the face's rig prefix
/// applied to the written control paths. `None` for an unknown id.
pub fn standard_mapping_source(id: &str, rig_prefix: &str) -> Option<(String, Json)> {
    let profile = standard_mapping(id)?;
    let mut spec: Json = serde_json::from_str(profile.asset_json).ok()?;
    ros4hri::apply_rig_prefix(&mut spec, rig_prefix);
    Some((id.to_string(), spec))
}

/// The registry as JSON — what CLIs print and the web runtime serves for
/// opt-in pickers.
pub fn standard_mappings_json() -> Json {
    Json::Array(
        STANDARD_MAPPINGS
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
        let listed = standard_mappings_json();
        assert_eq!(listed[0]["id"], "ros4hri");
        assert!(standard_mapping("ros4hri").is_some());
        assert!(standard_mapping("nope").is_none());
        assert_eq!(embedded_graph_id("ros4hri"), "standard::ros4hri");
    }

    #[test]
    fn profile_source_matches_the_ros4hri_source() {
        let via_registry = standard_mapping_source("ros4hri", "rig/f/").expect("known id");
        let direct = ros4hri::ros4hri_source("rig/f/");
        assert_eq!(via_registry.1, direct.1);
    }
}
