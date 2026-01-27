//! Shared JSON fixtures for Vizij engines, graphs, and orchestrations.
//!
//! The helpers in this crate map logical fixture names to JSON files declared in
//! `fixtures/manifest.json`, returning either raw strings, parsed values, or
//! filesystem paths for tooling that needs to read directly from disk.
//!
//! # Examples
//! ```no_run
//! use vizij_test_fixtures::{animations, node_graphs};
//!
//! let stored: serde_json::Value = animations::load("pose-quat-transform")?;
//! let spec: serde_json::Value = node_graphs::spec("simple-gain-offset")?;
//! # Ok::<(), anyhow::Error>(())
//! ```

use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{anyhow, Context, Result};
use once_cell::sync::Lazy;
use serde::de::DeserializeOwned;
use serde::Deserialize;

static MANIFEST: Lazy<Manifest> = Lazy::new(|| {
    let raw = include_str!("../../../../fixtures/manifest.json");
    serde_json::from_str(raw).expect("fixtures manifest should parse")
});

#[derive(Debug, Deserialize)]
struct Manifest {
    animations: HashMap<String, String>,
    #[serde(rename = "node-graphs")]
    node_graphs: HashMap<String, NodeGraphEntry>,
    orchestrations: HashMap<String, OrchestrationEntry>,
}

#[derive(Debug, Deserialize)]
struct NodeGraphEntry {
    spec: String,
    #[serde(default)]
    stage: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(untagged)]
enum OrchestrationEntry {
    Path(String),
    Detailed { path: String },
}

impl OrchestrationEntry {
    /// Returns path.
    fn as_path(&self) -> &str {
        match self {
            OrchestrationEntry::Path(path) => path,
            OrchestrationEntry::Detailed { path } => path,
        }
    }
}

/// Internal helper for `fixtures_root`.
fn fixtures_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../../fixtures")
}

/// Internal helper for `resolve_path`.
fn resolve_path(rel: &str) -> PathBuf {
    fixtures_root().join(rel)
}

/// Reads to string.
fn read_to_string(rel: &str) -> Result<String> {
    let path = resolve_path(rel);
    fs::read_to_string(&path)
        .with_context(|| format!("failed to read fixture at {}", path.display()))
}

/// Loads JSON.
fn load_json<T: DeserializeOwned>(rel: &str) -> Result<T> {
    let text = read_to_string(rel)?;
    serde_json::from_str(&text).with_context(|| format!("failed to parse JSON fixture {rel}"))
}

/// Internal helper for `lookup`.
fn lookup<'a, T>(map: &'a HashMap<String, T>, kind: &str, name: &str) -> Result<&'a T> {
    map.get(name)
        .ok_or_else(|| anyhow!("unknown {kind} fixture '{name}'"))
}

/// Helpers for animation fixtures listed in `fixtures/manifest.json`.
pub mod animations {
    use super::*;

    /// Returns the available animation fixture keys (unordered).
    ///
    /// Keys are taken directly from `fixtures/manifest.json` and do not
    /// guarantee any stable ordering.
    pub fn keys() -> Vec<String> {
        MANIFEST.animations.keys().cloned().collect()
    }

    /// Returns the raw JSON text for an animation fixture.
    ///
    /// # Errors
    /// Returns an error if the fixture name is unknown or the JSON file cannot be read.
    pub fn json(name: &str) -> Result<String> {
        let rel = lookup(&MANIFEST.animations, "animation", name)?;
        read_to_string(rel)
    }

    /// Loads an animation fixture into the requested JSON type.
    ///
    /// # Errors
    /// Returns an error if the fixture name is unknown, the file cannot be read,
    /// or the JSON cannot be deserialized.
    ///
    /// # Examples
    /// ```
    /// use vizij_test_fixtures::animations;
    ///
    /// let anim: serde_json::Value = animations::load("pose-quat-transform")?;
    /// assert!(anim.get("tracks").is_some());
    /// # Ok::<(), anyhow::Error>(())
    /// ```
    pub fn load<T: DeserializeOwned>(name: &str) -> Result<T> {
        let rel = lookup(&MANIFEST.animations, "animation", name)?;
        super::load_json(rel)
    }

    /// Resolves the filesystem path for an animation fixture.
    ///
    /// # Errors
    /// Returns an error if the fixture name is unknown.
    pub fn path(name: &str) -> Result<PathBuf> {
        let rel = lookup(&MANIFEST.animations, "animation", name)?;
        Ok(resolve_path(rel))
    }
}

/// Helpers for node graph fixtures listed in `fixtures/manifest.json`.
pub mod node_graphs {
    use super::*;

    /// Returns the available node-graph fixture keys (unordered).
    ///
    /// Keys are taken directly from `fixtures/manifest.json` and do not
    /// guarantee any stable ordering.
    pub fn keys() -> Vec<String> {
        MANIFEST.node_graphs.keys().cloned().collect()
    }

    /// Returns the raw JSON for a node-graph spec fixture.
    ///
    /// # Errors
    /// Returns an error if the fixture name is unknown or the JSON file cannot be read.
    pub fn spec_json(name: &str) -> Result<String> {
        let entry = lookup(&MANIFEST.node_graphs, "node graph", name)?;
        read_to_string(&entry.spec)
    }

    /// Loads a node-graph spec fixture into the requested JSON type.
    ///
    /// # Errors
    /// Returns an error if the fixture name is unknown, the file cannot be read,
    /// or the JSON cannot be deserialized.
    ///
    /// # Examples
    /// ```
    /// use vizij_test_fixtures::node_graphs;
    ///
    /// let spec: serde_json::Value = node_graphs::spec("simple-gain-offset")?;
    /// assert!(spec.get("spec").is_some());
    /// # Ok::<(), anyhow::Error>(())
    /// ```
    pub fn spec<T: DeserializeOwned>(name: &str) -> Result<T> {
        let entry = lookup(&MANIFEST.node_graphs, "node graph", name)?;
        super::load_json(&entry.spec)
    }

    /// Returns the raw JSON for a staged node-graph payload, if present.
    ///
    /// # Errors
    /// Returns an error if the fixture name is unknown or the stage JSON cannot be read.
    pub fn stage_json(name: &str) -> Result<Option<String>> {
        let entry = lookup(&MANIFEST.node_graphs, "node graph", name)?;
        match &entry.stage {
            Some(stage) => read_to_string(stage).map(Some),
            None => Ok(None),
        }
    }

    /// Loads a staged node-graph payload into the requested JSON type, if present.
    ///
    /// # Errors
    /// Returns an error if the fixture name is unknown, the stage file cannot be read,
    /// or the JSON cannot be deserialized.
    pub fn stage<T: DeserializeOwned>(name: &str) -> Result<Option<T>> {
        let entry = lookup(&MANIFEST.node_graphs, "node graph", name)?;
        match &entry.stage {
            Some(stage) => super::load_json(stage).map(Some),
            None => Ok(None),
        }
    }

    /// Resolves the filesystem path for a node-graph spec fixture.
    ///
    /// # Errors
    /// Returns an error if the fixture name is unknown.
    pub fn spec_path(name: &str) -> Result<PathBuf> {
        let entry = lookup(&MANIFEST.node_graphs, "node graph", name)?;
        Ok(resolve_path(&entry.spec))
    }

    /// Resolves the filesystem path for a staged node-graph payload, if present.
    ///
    /// # Errors
    /// Returns an error if the fixture name is unknown.
    pub fn stage_path(name: &str) -> Result<Option<PathBuf>> {
        let entry = lookup(&MANIFEST.node_graphs, "node graph", name)?;
        Ok(entry.stage.as_deref().map(resolve_path))
    }
}

/// Helpers for orchestration fixtures listed in `fixtures/manifest.json`.
pub mod orchestrations {
    use super::*;

    /// Returns the available orchestration fixture keys (unordered).
    ///
    /// Keys are taken directly from `fixtures/manifest.json` and do not
    /// guarantee any stable ordering.
    pub fn keys() -> Vec<String> {
        MANIFEST.orchestrations.keys().cloned().collect()
    }

    /// Returns the raw JSON text for an orchestration fixture.
    ///
    /// # Errors
    /// Returns an error if the fixture name is unknown or the JSON file cannot be read.
    pub fn json(name: &str) -> Result<String> {
        let entry = lookup(&MANIFEST.orchestrations, "orchestration", name)?;
        read_to_string(entry.as_path())
    }

    /// Loads an orchestration fixture into the requested JSON type.
    ///
    /// # Errors
    /// Returns an error if the fixture name is unknown, the file cannot be read,
    /// or the JSON cannot be deserialized.
    ///
    /// # Examples
    /// ```
    /// use vizij_test_fixtures::orchestrations;
    ///
    /// let orchestration: serde_json::Value = orchestrations::load("scalar-ramp-pipeline")?;
    /// assert!(orchestration.get("graphs").is_some());
    /// # Ok::<(), anyhow::Error>(())
    /// ```
    pub fn load<T: DeserializeOwned>(name: &str) -> Result<T> {
        let entry = lookup(&MANIFEST.orchestrations, "orchestration", name)?;
        super::load_json(entry.as_path())
    }

    /// Resolves the filesystem path for an orchestration fixture.
    ///
    /// # Errors
    /// Returns an error if the fixture name is unknown.
    pub fn path(name: &str) -> Result<PathBuf> {
        let entry = lookup(&MANIFEST.orchestrations, "orchestration", name)?;
        Ok(resolve_path(entry.as_path()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    /// Internal helper for `animation_pose_quat_transform_loads`.
    fn animation_pose_quat_transform_loads() {
        let value: serde_json::Value =
            animations::load("pose-quat-transform").expect("load pose-quat-transform fixture");
        assert!(value.get("tracks").is_some(), "animation tracks missing");
    }

    #[test]
    /// Internal helper for `node_graph_logic_gate_and_urdf_available`.
    fn node_graph_logic_gate_and_urdf_available() {
        let logic: serde_json::Value =
            node_graphs::spec("logic-gate").expect("load logic-gate graph spec");
        let nodes = logic
            .get("spec")
            .and_then(|spec| spec.get("nodes"))
            .and_then(|nodes| nodes.as_array());
        assert!(nodes.is_some(), "logic-gate nodes missing");

        let stage = node_graphs::stage_json("urdf-ik-position")
            .expect("fetch stage data for urdf-ik-position");
        assert!(
            stage.is_none(),
            "urdf-ik-position no longer includes shared stage data"
        );
    }

    #[test]
    /// Internal helper for `orchestration_blend_pose_pipeline_exists`.
    fn orchestration_blend_pose_pipeline_exists() {
        let json = orchestrations::json("blend-pose-pipeline")
            .expect("load blend-pose-pipeline descriptor");
        let value: serde_json::Value =
            serde_json::from_str(&json).expect("parse blend-pose-pipeline descriptor JSON");
        let legacy_animation = value.get("animation").and_then(|anim| anim.as_str());
        let primary_animation = value
            .get("animations")
            .and_then(|anims| anims.as_array())
            .and_then(|anims| anims.first())
            .and_then(|entry| entry.get("fixture"))
            .and_then(|fixture| fixture.as_str());
        let resolved = primary_animation.or(legacy_animation).unwrap_or_default();
        assert_eq!(resolved, "pose-quat-transform");
    }
}
