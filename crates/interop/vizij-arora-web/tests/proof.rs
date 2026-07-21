//! Values flow JS → store → arora tick → store → JS, through arora-web.
//!
//! Boots the thin Vizij-on-Arora device (an `arora::Arora` composed over a
//! `BlackboardStore` + `RigHal` + a passthrough Vizij graph, wrapped with
//! `arora_web::AroraWeb`), writes an input key from "JS", steps the arora
//! tick, and reads the graph's output key back out — proving the whole seam
//! end to end in wasm. Then swaps the graph in place through the caller's
//! LOAD path (VIZ-57) and proves the device and its store survived.

#![cfg(target_arch = "wasm32")]

use serde_json::{json, Value as Json};
use vizij_arora_web::VizijArora;
use wasm_bindgen::JsValue;
use wasm_bindgen_futures::JsFuture;
use wasm_bindgen_test::*;

wasm_bindgen_test_configure!(run_in_browser);

/// Build a JS `string[]` from string slices.
fn paths(ps: &[&str]) -> JsValue {
    serde_wasm_bindgen::to_value(&ps.iter().map(|p| p.to_string()).collect::<Vec<_>>()).unwrap()
}

/// Read the JS object returned by `read_values`/`drain_changes` back into JSON.
fn as_json(v: JsValue) -> Json {
    serde_wasm_bindgen::from_value(v).unwrap()
}

#[wasm_bindgen_test]
async fn values_flow_js_store_tick_store_js() {
    let rt = VizijArora::start(None, None).await.expect("runtime starts");

    // The graph's output key is absent before any tick runs.
    let before = as_json(rt.read_values(paths(&["actuator/y"])).unwrap());
    assert_eq!(before["actuator/y"], Json::Null);

    // "JS" writes the input key into the store (Arora Value JSON).
    rt.set_value("sensor/x", &json!({ "f32": 0.75 }).to_string())
        .unwrap();

    // One tick: the arora runtime ticks the installed Vizij graph, which reads
    // sensor/x from the store and writes actuator/y back.
    rt.step(16.0).expect("the device steps");

    // "JS" reads the transformed output back out of the same store.
    let after = as_json(rt.read_values(paths(&["sensor/x", "actuator/y"])).unwrap());
    assert_eq!(after["sensor/x"], json!({ "f32": 0.75 }));
    assert_eq!(
        after["actuator/y"],
        json!({ "f32": 0.75 }),
        "the value flowed through the arora tick into the graph's output key"
    );

    // A second value proves it keeps flowing tick to tick.
    rt.write_values(&json!({ "sensor/x": { "f32": 0.25 } }).to_string())
        .unwrap();
    rt.step(16.0).expect("the device steps");
    let after2 = as_json(rt.read_values(paths(&["actuator/y"])).unwrap());
    assert_eq!(after2["actuator/y"], json!({ "f32": 0.25 }));

    // The change feed saw the graph's write this step.
    let changes = as_json(rt.drain_changes().unwrap());
    assert_eq!(changes["actuator/y"], json!({ "f32": 0.25 }));
}

/// The first drain returns the store's whole current state — the subscription
/// opens on it — so a consumer needs no separate init snapshot.
#[wasm_bindgen_test]
async fn the_first_drain_is_the_whole_state() {
    let rt = VizijArora::start(None, None).await.expect("runtime starts");
    rt.set_value("sensor/x", &json!({ "f32": 0.5 }).to_string())
        .unwrap();
    let opening = as_json(rt.drain_changes().unwrap());
    assert_eq!(opening["sensor/x"], json!({ "f32": 0.5 }));
}

/// The in-place load path (VIZ-57): a `loadGraph` dispatched through the
/// device's caller lands at the next step, swaps the running graph, and the
/// device and its store survive the swap.
#[wasm_bindgen_test]
async fn load_graph_swaps_the_behavior_in_place() {
    let rt = VizijArora::start(None, None).await.expect("runtime starts");

    rt.set_value("sensor/x", &json!({ "f32": 0.75 }).to_string())
        .unwrap();
    rt.step(16.0).expect("the device steps");

    // Swap to a different passthrough. The call applies at the next step,
    // whose tick already runs the new graph — the input's declared default
    // covers it until sensor/b is first written.
    let loaded = rt.load_graph(
        &json!({
            "nodes": [
                {
                    "id": "in",
                    "type": "input",
                    "params": { "path": "sensor/b", "value": { "float": 0.0 } }
                },
                { "id": "out", "type": "output", "params": { "path": "actuator/b" } }
            ],
            "edges": [
                { "from": { "node_id": "in" }, "to": { "node_id": "out", "input": "in" } }
            ]
        })
        .to_string(),
    );
    rt.step(16.0).expect("the device steps");
    JsFuture::from(loaded).await.expect("the load call lands");

    // The new graph runs on the same device and store.
    rt.set_value("sensor/b", &json!({ "f32": 0.25 }).to_string())
        .unwrap();
    rt.step(16.0).expect("the device steps");
    let after = as_json(
        rt.read_values(paths(&["actuator/b", "sensor/x", "actuator/y"]))
            .unwrap(),
    );
    assert_eq!(after["actuator/b"], json!({ "f32": 0.25 }));
    // The store carried across the swap untouched.
    assert_eq!(after["sensor/x"], json!({ "f32": 0.75 }));
    assert_eq!(after["actuator/y"], json!({ "f32": 0.75 }));
}
