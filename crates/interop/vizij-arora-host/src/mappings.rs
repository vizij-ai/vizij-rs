//! The registry of standard **mappings** — the composable graph assets that
//! implement one profile in terms of another, so a face responds to an
//! external standard. One entry today (ROS4HRI): it consumes the `ros4hri`
//! profile and produces the `vizij-face` profile, both declared in
//! [`crate::profile`].
//!
//! A mapping is the operation half of a standard; the profile is the
//! interface it translates. The registry exists so hosts, the bundler, and
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

/// A standard mapping: its identity, the committed asset behind it, and the
/// generator the asset is regenerated from (`vizij-bundle export-mapping
/// <id>`).
pub struct StandardMapping {
    /// Registry id, e.g. `ros4hri` — also names the mapping everywhere a user
    /// opts in (CLI flags, bundle graph ids, npm lookups).
    pub id: &'static str,
    pub title: &'static str,
    pub description: &'static str,
    /// The canonical graph asset, verbatim JSON.
    pub asset_json: &'static str,
    /// Regenerate the unprefixed graph from first principles.
    pub generate: fn() -> Json,
}

/// Every standard mapping Vizij ships.
pub const STANDARD_MAPPINGS: [StandardMapping; 1] = [StandardMapping {
    id: "ros4hri",
    title: "ROS4HRI",
    description: "Drives the standard face controls from the standard/ros4hri/* keys: \
                  expression names and valence/arousal, gaze targets with vergence, FACS \
                  action units, idle blink, and the incumbent's ~200 ms smoothing.",
    asset_json: ros4hri::MAPPING_JSON,
    generate: ros4hri::generate,
}];

/// Look a mapping up by id.
pub fn standard_mapping(id: &str) -> Option<&'static StandardMapping> {
    STANDARD_MAPPINGS.iter().find(|m| m.id == id)
}

/// The bundle graph id under which `mapping_id` embeds in a GLB — stable, so
/// re-embedding replaces the entry in place.
pub fn embedded_graph_id(mapping_id: &str) -> String {
    format!("standard::{mapping_id}")
}

/// A mapping's graph as a composable source, with the face's rig prefix
/// applied to the written control paths. `None` for an unknown id.
pub fn standard_mapping_source(id: &str, rig_prefix: &str) -> Option<(String, Json)> {
    let mapping = standard_mapping(id)?;
    let mut spec: Json = serde_json::from_str(mapping.asset_json).ok()?;
    ros4hri::apply_rig_prefix(&mut spec, rig_prefix);
    Some((id.to_string(), spec))
}

/// The registry as JSON — what CLIs print and the web runtime serves for
/// opt-in pickers.
pub fn standard_mappings_json() -> Json {
    Json::Array(
        STANDARD_MAPPINGS
            .iter()
            .map(|m| {
                json!({
                    "id": m.id,
                    "title": m.title,
                    "description": m.description,
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
    fn mapping_source_matches_the_ros4hri_source() {
        let via_registry = standard_mapping_source("ros4hri", "rig/f/").expect("known id");
        let direct = ros4hri::ros4hri_source("rig/f/");
        assert_eq!(via_registry.1, direct.1);
    }

    /// The registry's generator is the one the drift test holds the asset to.
    #[test]
    fn the_registry_generator_reproduces_the_asset() {
        let entry = standard_mapping("ros4hri").unwrap();
        let committed: Json = serde_json::from_str(entry.asset_json).unwrap();
        assert_eq!(committed, (entry.generate)());
    }
}
