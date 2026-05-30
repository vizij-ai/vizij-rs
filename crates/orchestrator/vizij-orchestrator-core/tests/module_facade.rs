use serde_json::{json, Value};
use vizij_orchestrator::VizijModuleFacade;

fn dispatch(facade: &mut VizijModuleFacade, call: &str, args: Value) -> Value {
    let response = facade.dispatch_json(
        &json!({
            "call": call,
            "requestId": format!("req:{call}"),
            "args": args,
        })
        .to_string(),
    );
    let parsed: Value = serde_json::from_str(&response).expect("facade response json");
    assert_eq!(parsed["ok"], true, "{parsed}");
    parsed["result"].clone()
}

fn fixture_graph() -> Value {
    json!({
        "nodes": [
            {
                "id": "source",
                "type": "constant",
                "params": {
                    "value": { "type": "float", "data": 2.5 }
                }
            },
            {
                "id": "out",
                "type": "output",
                "params": {
                    "path": "facade/graph.value"
                }
            }
        ],
        "edges": [
            {
                "from": { "node_id": "source", "output": "out" },
                "to": { "node_id": "out", "input": "in" }
            }
        ]
    })
}

fn graph_constant_output(path: &str, value: f32) -> Value {
    json!({
        "nodes": [
            {
                "id": "source",
                "type": "constant",
                "params": {
                    "value": { "type": "float", "data": value }
                }
            },
            {
                "id": "out",
                "type": "output",
                "params": {
                    "path": path
                }
            }
        ],
        "edges": [
            {
                "from": { "node_id": "source", "output": "out" },
                "to": { "node_id": "out", "input": "in" }
            }
        ]
    })
}

fn graph_time_output(path: &str) -> Value {
    json!({
        "nodes": [
            {
                "id": "time",
                "type": "time"
            },
            {
                "id": "out",
                "type": "output",
                "params": {
                    "path": path
                }
            }
        ],
        "edges": [
            {
                "from": { "node_id": "time", "output": "out" },
                "to": { "node_id": "out", "input": "in" }
            }
        ]
    })
}

fn fixture_animation() -> Value {
    fixture_animation_for_path("face/smile.amount")
}

fn fixture_animation_for_path(output_path: &str) -> Value {
    json!({
        "id": "facade-animation-smoke",
        "name": "Facade Animation Smoke",
        "formatVersion": 2,
        "defaultViewportExtent": 1000,
        "groups": [],
        "tracks": [
            {
                "id": "smile-track",
                "name": "Smile",
                "animatableId": output_path,
                "points": [
                    { "id": "smile-0", "stamp": 0, "value": 0, "transitions": { "out": "linear" } },
                    { "id": "smile-1", "stamp": 1000, "value": 1, "transitions": { "in": "linear" } }
                ]
            }
        ]
    })
}

#[test]
fn step_delta_suppresses_only_unchanged_paths() {
    let mut facade = VizijModuleFacade::new();
    dispatch(
        &mut facade,
        "runtime.create",
        json!({ "schedule": "SinglePass" }),
    );
    dispatch(
        &mut facade,
        "graph.register",
        json!({
            "id": "graph:static",
            "spec": graph_constant_output("face/static.value", 0.25)
        }),
    );
    dispatch(
        &mut facade,
        "graph.register",
        json!({
            "id": "graph:time",
            "spec": graph_time_output("face/time.value")
        }),
    );

    let initial = dispatch(&mut facade, "orchestrator.stepDelta", json!({ "dt": 0.25 }));
    assert_eq!(initial["version"], 1);
    assert_eq!(
        initial["merged_writes"]
            .as_array()
            .expect("initial writes")
            .len(),
        2
    );

    let delta = dispatch(
        &mut facade,
        "orchestrator.stepDelta",
        json!({ "dt": 0.25, "sinceVersion": 1 }),
    );
    let writes = delta["merged_writes"].as_array().expect("delta writes");
    assert_eq!(writes.len(), 1);
    assert_eq!(writes[0]["path"], "face/time.value");
}

#[test]
fn facade_dispatches_stateful_runtime_calls() {
    let mut facade = VizijModuleFacade::new();

    let runtime = dispatch(
        &mut facade,
        "runtime.create",
        json!({ "schedule": "SinglePass" }),
    );
    assert_eq!(runtime["runtimeHandle"], "runtime:0");

    let graph = dispatch(
        &mut facade,
        "graph.register",
        json!({ "id": "graph:facade", "spec": fixture_graph() }),
    );
    assert_eq!(graph["graphId"], "graph:facade");

    let animation = dispatch(
        &mut facade,
        "animation.register",
        json!({
            "id": "anim:facade",
            "setup": {
                "animation": fixture_animation(),
                "player": { "name": "facade-player", "loop_mode": "once" },
                "instance": { "weight": 1.0 }
            }
        }),
    );
    assert_eq!(animation["animationId"], "anim:facade");

    let controllers = dispatch(&mut facade, "controllers.list", json!({}));
    assert_eq!(controllers["graphs"], json!(["graph:facade"]));
    assert_eq!(controllers["anims"], json!(["anim:facade"]));

    let frame = dispatch(&mut facade, "orchestrator.step", json!({ "dt": 0.5 }));
    let writes = frame["merged_writes"].as_array().expect("writes array");
    assert!(
        writes
            .iter()
            .any(|write| write["path"] == "facade/graph.value"),
        "graph write missing: {writes:?}"
    );
    assert!(
        writes
            .iter()
            .any(|write| write["path"] == "face/smile.amount"),
        "animation write missing: {writes:?}"
    );
}

#[test]
fn animation_setup_accepts_studio_instance_aliases() {
    let mut facade = VizijModuleFacade::new();
    dispatch(
        &mut facade,
        "runtime.create",
        json!({ "schedule": "SinglePass" }),
    );

    dispatch(
        &mut facade,
        "animation.register",
        json!({
            "id": "anim:studio-aliases",
            "setup": {
                "animation": fixture_animation(),
                "player": { "name": "studio-player", "loopMode": "once" },
                "instance": {
                    "weight": 1.0,
                    "timeScale": 2.0,
                    "offset": 250.0,
                    "active": true
                }
            }
        }),
    );

    let frame = dispatch(&mut facade, "orchestrator.step", json!({ "dt": 0.375 }));
    let writes = frame["merged_writes"].as_array().expect("writes array");
    let smile = writes
        .iter()
        .find(|write| write["path"] == "face/smile.amount")
        .expect("animation write");
    let value = smile["value"]["data"].as_f64().expect("float value");
    assert!(
        (value - 0.25).abs() < 0.0001,
        "expected Studio offset/timeScale aliases to produce 0.25, got {value}"
    );
}

#[test]
fn animation_setup_accepts_legacy_timescale_and_start_offset_aliases() {
    let mut facade = VizijModuleFacade::new();
    dispatch(
        &mut facade,
        "runtime.create",
        json!({ "schedule": "SinglePass" }),
    );

    dispatch(
        &mut facade,
        "animation.register",
        json!({
            "id": "anim:legacy-aliases",
            "setup": {
                "animation": fixture_animation(),
                "instance": {
                    "timescale": 2.0,
                    "startOffset": 0.25
                }
            }
        }),
    );

    let frame = dispatch(&mut facade, "orchestrator.step", json!({ "dt": 0.375 }));
    let writes = frame["merged_writes"].as_array().expect("writes array");
    let smile = writes
        .iter()
        .find(|write| write["path"] == "face/smile.amount")
        .expect("animation write");
    let value = smile["value"]["data"].as_f64().expect("float value");
    assert!(
        (value - 0.25).abs() < 0.0001,
        "expected legacy timescale/startOffset aliases to produce 0.25, got {value}"
    );
}

#[test]
fn animation_setup_active_false_disables_studio_instance() {
    let mut facade = VizijModuleFacade::new();
    dispatch(
        &mut facade,
        "runtime.create",
        json!({ "schedule": "SinglePass" }),
    );

    dispatch(
        &mut facade,
        "animation.register",
        json!({
            "id": "anim:inactive",
            "setup": {
                "animation": fixture_animation(),
                "instance": {
                    "active": false
                }
            }
        }),
    );

    let frame = dispatch(&mut facade, "orchestrator.step", json!({ "dt": 0.5 }));
    let writes = frame["merged_writes"].as_array().expect("writes array");
    assert!(
        !writes
            .iter()
            .any(|write| write["path"] == "face/smile.amount"),
        "inactive Studio instance should not publish animation writes: {writes:?}"
    );
}

#[test]
fn scoped_animation_commands_target_registered_controller_id() {
    let mut facade = VizijModuleFacade::new();
    dispatch(
        &mut facade,
        "runtime.create",
        json!({ "schedule": "SinglePass" }),
    );

    dispatch(
        &mut facade,
        "animation.register",
        json!({
            "id": "default/animation/blink",
            "setup": {
                "animation": fixture_animation_for_path("face/blink.amount"),
                "player": { "speed": 0.0 }
            }
        }),
    );
    dispatch(
        &mut facade,
        "animation.register",
        json!({
            "id": "default/animation/smile",
            "setup": {
                "animation": fixture_animation_for_path("face/smile.amount"),
                "player": { "speed": 0.0 }
            }
        }),
    );

    dispatch(
        &mut facade,
        "input.set",
        json!({
            "path": "anim/controller/default/animation/blink/player/0/cmd/seek",
            "value": { "type": "float", "data": 0.75 }
        }),
    );

    let frame = dispatch(&mut facade, "orchestrator.step", json!({ "dt": 0.0 }));
    let writes = frame["merged_writes"].as_array().expect("writes array");
    let blink = writes
        .iter()
        .find(|write| write["path"] == "face/blink.amount")
        .expect("blink animation write");
    let value = blink["value"]["data"].as_f64().expect("float value");
    assert!(
        (value - 0.75).abs() < 0.0001,
        "expected scoped seek to move blink only, got {value}"
    );
    let smile = writes
        .iter()
        .find(|write| write["path"] == "face/smile.amount")
        .expect("smile animation should still report its unchanged initial value");
    let smile_value = smile["value"]["data"].as_f64().expect("float value");
    assert!(
        smile_value.abs() < 0.0001,
        "scoped blink command should not move smile, got {smile_value}"
    );
}

#[test]
fn legacy_animation_commands_remain_broadcast_compatible() {
    let mut facade = VizijModuleFacade::new();
    dispatch(
        &mut facade,
        "runtime.create",
        json!({ "schedule": "SinglePass" }),
    );

    for (id, path) in [
        ("default/animation/blink", "face/blink.amount"),
        ("default/animation/smile", "face/smile.amount"),
    ] {
        dispatch(
            &mut facade,
            "animation.register",
            json!({
                "id": id,
                "setup": {
                    "animation": fixture_animation_for_path(path),
                    "player": { "speed": 0.0 }
                }
            }),
        );
    }

    dispatch(
        &mut facade,
        "input.set",
        json!({
            "path": "anim/player/0/cmd/seek",
            "value": { "type": "float", "data": 0.5 }
        }),
    );

    let frame = dispatch(&mut facade, "orchestrator.step", json!({ "dt": 0.0 }));
    let writes = frame["merged_writes"].as_array().expect("writes array");
    for path in ["face/blink.amount", "face/smile.amount"] {
        let write = writes
            .iter()
            .find(|write| write["path"] == path)
            .unwrap_or_else(|| panic!("missing broadcast write for {path}: {writes:?}"));
        let value = write["value"]["data"].as_f64().expect("float value");
        assert!(
            (value - 0.5).abs() < 0.0001,
            "expected legacy command to broadcast to {path}, got {value}"
        );
    }
}

#[test]
fn facade_reports_errors_as_json() {
    let mut facade = VizijModuleFacade::new();
    let response = facade.dispatch_json(
        &json!({
            "call": "orchestrator.step",
            "requestId": "req:step",
            "args": { "dt": 0.016 }
        })
        .to_string(),
    );
    let parsed: Value = serde_json::from_str(&response).expect("facade response json");
    assert_eq!(parsed["ok"], false);
    assert_eq!(parsed["requestId"], "req:step");
    assert!(parsed["error"]
        .as_str()
        .expect("error string")
        .contains("runtime is not created"));
}

#[test]
fn facade_normalizes_graph_specs_without_runtime() {
    let mut facade = VizijModuleFacade::new();
    let normalized = dispatch(
        &mut facade,
        "graph.normalize",
        json!({
            "spec": {
                "nodes": [
                    {
                        "id": "source",
                        "kind": "Node",
                        "params": {
                            "value": { "float": 1.0 }
                        }
                    }
                ]
            }
        }),
    );

    assert_eq!(normalized["nodes"][0]["type"], "node");
    assert_eq!(normalized["nodes"][0]["params"]["value"]["type"], "float");
    assert_eq!(
        normalized["edges"].as_array().map(|edges| edges.len()),
        Some(0)
    );
}

#[test]
fn facade_rejects_mismatched_runtime_handles() {
    let mut facade = VizijModuleFacade::new();
    dispatch(
        &mut facade,
        "runtime.create",
        json!({ "schedule": "SinglePass" }),
    );

    let response = facade.dispatch_json(
        &json!({
            "call": "orchestrator.step",
            "runtimeHandle": "runtime:wrong",
            "requestId": "req:step",
            "args": { "dt": 0.016 }
        })
        .to_string(),
    );
    let parsed: Value = serde_json::from_str(&response).expect("facade response json");
    assert_eq!(parsed["ok"], false);
    assert!(parsed["error"]
        .as_str()
        .expect("error string")
        .contains("runtime handle mismatch"));
}
