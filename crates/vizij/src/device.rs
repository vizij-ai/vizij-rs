//! The arora device behind the view: `RigHal` + `BlackboardStore` + the
//! face's composed graph as the behavior, stepped on a worker thread.

use std::thread;
use std::time::{Duration, Instant};

use anyhow::{anyhow, Result};
use serde_json::{json, Value as Json};
use vizij_arora_behavior::{parse_spec, ProcessingGraph};
use vizij_arora_hal::RigHal;
use vizij_arora_store::BlackboardStore;

/// A running face device. Both handles share storage with the device's own
/// (they are sibling clones), so the view reads the rig and the store live.
pub struct Device {
    pub rig: RigHal,
    /// Sibling store handle; unused until input staging/UI lands (VIZ-47 UI stage).
    #[allow(dead_code)]
    pub store: BlackboardStore,
    _thread: thread::JoinHandle<()>,
}

/// Compose bundle graphs into the one spec the device runs — the native
/// equivalent of the web host's `composeGraphSpecs`. Node ids are namespaced
/// per source (`{source}::{id}`) so sources cannot collide; **store paths stay
/// shared** — that is the cross-source contract.
///
/// Each graph is normalized first (legacy input-connection forms become
/// edges), so id rewriting sees the canonical `nodes`/`edges` shape.
pub fn compose(graphs: &[(String, Json)]) -> Result<Json> {
    let mut nodes = Vec::new();
    let mut edges = Vec::new();
    for (index, (kind, spec)) in graphs.iter().enumerate() {
        let mut normalized = spec.clone();
        vizij_api_core::json::normalize_graph_spec_value(&mut normalized)
            .map_err(|e| anyhow!("graph {index} ({kind}) does not normalize: {e:?}"))?;
        let prefix = format!("{kind}{index}::");
        for node in normalized
            .get("nodes")
            .and_then(Json::as_array)
            .into_iter()
            .flatten()
        {
            let mut node = node.clone();
            if let Some(id) = node.get("id").and_then(Json::as_str) {
                node["id"] = json!(format!("{prefix}{id}"));
            }
            nodes.push(node);
        }
        for edge in normalized
            .get("edges")
            .and_then(Json::as_array)
            .into_iter()
            .flatten()
        {
            let mut edge = edge.clone();
            for end in ["from", "to"] {
                if let Some(id) = edge
                    .get(end)
                    .and_then(|e| e.get("node_id"))
                    .and_then(Json::as_str)
                {
                    edge[end]["node_id"] = json!(format!("{prefix}{id}"));
                }
            }
            edges.push(edge);
        }
    }
    Ok(json!({ "nodes": nodes, "edges": edges }))
}

/// Builds the arora (RigHal + BlackboardStore + ProcessingGraph over the
/// composed spec) and steps it at ~100 Hz with measured dt.
///
/// The `Arora` is constructed **inside** the worker thread — it is not `Send`
/// (single-owner by design); only the spec JSON and the sibling rig/store
/// handles cross the thread boundary. The spec is validated here first so
/// composition errors surface to the caller, not in a log.
pub fn start(composed_spec: &Json) -> Result<Device> {
    let spec_json = composed_spec.to_string();
    parse_spec(&spec_json).map_err(|e| anyhow!("composed spec does not parse: {e}"))?;

    let rig = RigHal::new();
    let store = BlackboardStore::new();

    let thread = {
        let rig = rig.clone();
        let store = store.clone();
        thread::Builder::new().name("arora".into()).spawn(move || {
            let spec = match parse_spec(&spec_json) {
                Ok(spec) => spec,
                Err(e) => return log::error!("spec re-parse failed: {e}"),
            };
            let graph = match ProcessingGraph::from_spec(spec) {
                Ok(graph) => graph,
                Err(e) => return log::error!("graph encode failed: {e}"),
            };
            let mut arora = match arora::Arora::builder()
                .with_hal(Box::new(rig))
                .with_data_store(Box::new(store))
                .with_behavior_interpreter(Box::new(graph))
                .build()
            {
                Ok(arora) => arora,
                Err(e) => return log::error!("building the arora device: {e:?}"),
            };

            let period = Duration::from_millis(10);
            let mut last = Instant::now();
            loop {
                let now = Instant::now();
                let dt = now.duration_since(last);
                last = now;
                if let Err(e) = arora.step(dt) {
                    log::error!("arora device stopped: {e:?}");
                    break;
                }
                thread::sleep(period);
            }
        })?
    };

    Ok(Device {
        rig,
        store,
        _thread: thread,
    })
}
