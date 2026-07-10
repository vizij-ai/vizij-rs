//! Values flow JS → store → arora tick → store → JS, through arora-web.
//!
//! Boots the thin Vizij-on-Arora device (which builds an `arora_web::BrowserRuntime`
//! over a `BlackboardStore` + `RigHal` + a passthrough Vizij graph), writes an
//! input key from "JS", steps the arora tick, and reads the graph's output key
//! back out — proving the whole seam end to end in wasm through the published
//! browser-runtime primitive.

#![cfg(target_arch = "wasm32")]

use serde_json::{json, Value as Json};
use vizij_arora_web::VizijArora;
use wasm_bindgen::JsValue;
use wasm_bindgen_test::*;

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
    let mut rt = VizijArora::start(None).await.expect("runtime starts");

    // The graph's output key is absent before any tick runs.
    let before = as_json(rt.read_values(paths(&["actuator/y"])).unwrap());
    assert_eq!(before["actuator/y"], Json::Null);

    // "JS" writes the input key into the store (Arora Value JSON).
    rt.set_value("sensor/x", &json!({ "f32": 0.75 }).to_string())
        .unwrap();

    // One tick: the arora runtime ticks the installed Vizij graph, which reads
    // sensor/x from the store and writes actuator/y back.
    assert!(rt.step(16.0).unwrap(), "runtime stays live");

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
    assert!(rt.step(16.0).unwrap());
    let after2 = as_json(rt.read_values(paths(&["actuator/y"])).unwrap());
    assert_eq!(after2["actuator/y"], json!({ "f32": 0.25 }));

    // The change feed saw the graph's write this step.
    let changes = as_json(rt.drain_changes().unwrap());
    assert_eq!(changes["actuator/y"], json!({ "f32": 0.25 }));
}
