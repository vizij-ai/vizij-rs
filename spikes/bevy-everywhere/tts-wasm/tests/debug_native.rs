#![cfg(not(target_arch = "wasm32"))]
use std::time::Duration;
use arora_behavior::{interpreter_module, RunPolicy};
use arora_types::call::Call;
use arora_types::data::Key;
use arora_types::value::{StructureField, Value};
use tts_wasm_spike::*;
use vizij_arora_behavior::viseme;

#[test]
fn debug_play_viseme() {
    let provider = host_module(Config { api_base: DEFAULT_API_BASE.to_string() });
    let spec = serde_json::json!({
        "nodes": [
            { "id": "in",  "type": "input",  "params": { "path": "sensor/x" } },
            { "id": "out", "type": "output", "params": { "path": "actuator/y" } }
        ],
        "edges": [ { "from": { "node_id": "in" }, "to": { "node_id": "out", "input": "in" } } ]
    }).to_string();
    let mut arora = compose(&spec, "", provider).unwrap().build().unwrap();
    let parameters = viseme::play_viseme_parameters();
    let arg = |name: &str, value: Value| StructureField {
        id: *parameters.iter().find(|(_, n)| n == &name).unwrap().0,
        value: Box::new(value),
    };
    let call = Call { module_id: Some(viseme::MODULE_ID), id: viseme::PLAY_VISEME_ID,
        args: vec![arg("shape", Value::String("aa".into())), arg("weight", Value::F32(1.0))] };
    let spawned = arora.call(interpreter_module::encode_spawn(&call, RunPolicy::Concurrent));
    println!("spawn result: {spawned:?}");
    let handle = interpreter_module::decode_spawn_result(&spawned.unwrap().ret).unwrap();
    for i in 0..3 {
        let r = arora.step(Duration::from_millis(16));
        println!("step {i}: {r:?}; behavior_error = {:?}", arora.behavior_error().borrow().clone());
    }
    for path in [handle.status.path.clone(), "standard/vizij/viseme/aa".to_string(), "standard/vizij/viseme".to_string(), "actuator/y".to_string()] {
        let v = arora.store().read(&[Key::from(path.as_str())]).into_iter().next().flatten();
        println!("{path} = {v:?}");
    }
}
