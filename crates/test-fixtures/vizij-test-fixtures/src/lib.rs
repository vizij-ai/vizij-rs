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
    fn as_path(&self) -> &str {
        match self {
            OrchestrationEntry::Path(path) => path,
            OrchestrationEntry::Detailed { path } => path,
        }
    }
}

fn fixtures_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../../fixtures")
}

fn resolve_path(rel: &str) -> PathBuf {
    fixtures_root().join(rel)
}

fn read_to_string(rel: &str) -> Result<String> {
    let path = resolve_path(rel);
    fs::read_to_string(&path)
        .with_context(|| format!("failed to read fixture at {}", path.display()))
}

fn load_json<T: DeserializeOwned>(rel: &str) -> Result<T> {
    let text = read_to_string(rel)?;
    serde_json::from_str(&text).with_context(|| format!("failed to parse JSON fixture {rel}"))
}

fn lookup<'a, T>(map: &'a HashMap<String, T>, kind: &str, name: &str) -> Result<&'a T> {
    map.get(name)
        .ok_or_else(|| anyhow!("unknown {kind} fixture '{name}'"))
}

pub mod animations {
    use super::*;

    pub fn keys() -> Vec<String> {
        MANIFEST.animations.keys().cloned().collect()
    }

    pub fn json(name: &str) -> Result<String> {
        let rel = lookup(&MANIFEST.animations, "animation", name)?;
        read_to_string(rel)
    }

    pub fn load<T: DeserializeOwned>(name: &str) -> Result<T> {
        let rel = lookup(&MANIFEST.animations, "animation", name)?;
        super::load_json(rel)
    }

    pub fn path(name: &str) -> Result<PathBuf> {
        let rel = lookup(&MANIFEST.animations, "animation", name)?;
        Ok(resolve_path(rel))
    }
}

pub mod node_graphs {
    use super::*;

    pub fn keys() -> Vec<String> {
        MANIFEST.node_graphs.keys().cloned().collect()
    }

    pub fn spec_json(name: &str) -> Result<String> {
        let entry = lookup(&MANIFEST.node_graphs, "node graph", name)?;
        read_to_string(&entry.spec)
    }

    pub fn spec<T: DeserializeOwned>(name: &str) -> Result<T> {
        let entry = lookup(&MANIFEST.node_graphs, "node graph", name)?;
        super::load_json(&entry.spec)
    }

    pub fn stage_json(name: &str) -> Result<Option<String>> {
        let entry = lookup(&MANIFEST.node_graphs, "node graph", name)?;
        match &entry.stage {
            Some(stage) => read_to_string(stage).map(Some),
            None => Ok(None),
        }
    }

    pub fn stage<T: DeserializeOwned>(name: &str) -> Result<Option<T>> {
        let entry = lookup(&MANIFEST.node_graphs, "node graph", name)?;
        match &entry.stage {
            Some(stage) => super::load_json(stage).map(Some),
            None => Ok(None),
        }
    }

    pub fn spec_path(name: &str) -> Result<PathBuf> {
        let entry = lookup(&MANIFEST.node_graphs, "node graph", name)?;
        Ok(resolve_path(&entry.spec))
    }

    pub fn stage_path(name: &str) -> Result<Option<PathBuf>> {
        let entry = lookup(&MANIFEST.node_graphs, "node graph", name)?;
        Ok(entry.stage.as_deref().map(resolve_path))
    }
}

pub mod orchestrations {
    use super::*;

    pub fn keys() -> Vec<String> {
        MANIFEST.orchestrations.keys().cloned().collect()
    }

    pub fn json(name: &str) -> Result<String> {
        let entry = lookup(&MANIFEST.orchestrations, "orchestration", name)?;
        read_to_string(entry.as_path())
    }

    pub fn load<T: DeserializeOwned>(name: &str) -> Result<T> {
        let entry = lookup(&MANIFEST.orchestrations, "orchestration", name)?;
        super::load_json(entry.as_path())
    }

    pub fn path(name: &str) -> Result<PathBuf> {
        let entry = lookup(&MANIFEST.orchestrations, "orchestration", name)?;
        Ok(resolve_path(entry.as_path()))
    }
}
