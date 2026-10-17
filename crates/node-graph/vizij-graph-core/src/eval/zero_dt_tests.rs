//! A step of zero time changes nothing that depends on time.
//!
//! Every node kind that reads the clock or keeps state across evaluations is
//! driven here by the graph's own time, so it is in motion when a zero step
//! comes: the zero step leaves every port and every node's state as they
//! were, and a run with zero steps interleaved follows the same course as one
//! without.

use super::*;
use crate::types::GraphSpec;
use serde_json::{json, Value as Json};
use std::collections::BTreeMap;
use uuid::Uuid;
use vizij_api_core::value as vocab;
use vizij_api_core::Value;

const DT: f32 = 1.0 / 60.0;

/// The function the task-run, spawn and external-function cases name.
const FUNCTION: &str = "6c1f0d1e-4b8a-4f43-9d55-2f7a3b9e0c11";
/// The parameter the spawn case passes the clock to.
const PARAM: &str = "0f2b9c3d-6e7a-4d18-8b5c-91a4e2f7d6b0";

fn spec(json: Json) -> GraphSpec {
    serde_json::from_value::<GraphSpec>(json)
        .expect("a valid graph spec")
        .with_cache()
}

fn f32_json(value: f32) -> Json {
    json!({ "f32": value })
}

fn edge(from: &str, to: &str, input: &str) -> Json {
    json!({ "from": { "node_id": from }, "to": { "node_id": to, "input": input } })
}

/// A function host whose functions are pure: a task run is `Running`, any
/// other call answers its first argument.
struct PureFunctions;

impl NodeFunctions for PureFunctions {
    fn call(&mut self, _function: Uuid, args: &[(Uuid, Value)]) -> Result<Value, String> {
        Ok(args
            .first()
            .map(|(_, value)| value.clone())
            .unwrap_or(Value::Unit))
    }

    fn call_module_with_outputs(
        &mut self,
        _module: Option<Uuid>,
        _function: Uuid,
        _args: &[(Uuid, Value)],
    ) -> Result<(Value, Vec<(Uuid, Value)>), String> {
        Ok((crate::task::running(), Vec::new()))
    }
}

/// Advance the graph's clock by `dt` and evaluate it.
fn step(rt: &mut GraphRuntime, spec: &GraphSpec, dt: f32) {
    rt.dt = dt;
    rt.t += dt;
    evaluate_all_with_functions(rt, spec, &mut PureFunctions).expect("the graph evaluates");
}

/// What an evaluation leaves: the clock, every port and every node's state.
/// A spawn node's port is left out: it says whether that one evaluation
/// requested a run, an event rather than a state, and the runs requested are
/// checked on their own.
#[derive(Debug, PartialEq)]
struct Observed {
    t: f32,
    ports: BTreeMap<String, BTreeMap<String, Value>>,
    states: BTreeMap<String, String>,
}

fn observed(rt: &GraphRuntime, spec: &GraphSpec) -> Observed {
    let events: Vec<&str> = spec
        .nodes
        .iter()
        .filter(|node| node.kind == crate::types::NodeType::Spawn)
        .map(|node| node.id.as_str())
        .collect();
    Observed {
        t: rt.t,
        ports: rt
            .outputs
            .iter()
            .filter(|(node, _)| !events.contains(&node.as_str()))
            .map(|(node, ports)| {
                (
                    node.clone(),
                    ports
                        .iter()
                        .map(|(port, value)| (port.clone(), value.value.clone()))
                        .collect(),
                )
            })
            .collect(),
        states: rt
            .node_states
            .iter()
            .map(|(node, state)| (node.clone(), format!("{state:?}")))
            .collect(),
    }
}

/// A graph with the clock (`clock`), a wave of it (`wave`, a 0.7 Hz
/// oscillator) and the nodes of one case, each fed from those.
fn driven(nodes: Vec<Json>, edges: Vec<Json>) -> GraphSpec {
    let mut all = vec![
        json!({ "id": "clock", "type": "time" }),
        json!({ "id": "frequency", "type": "constant", "params": { "value": f32_json(0.7) } }),
        json!({ "id": "wave", "type": "oscillator" }),
    ];
    all.extend(nodes);
    let mut all_edges = vec![edge("frequency", "wave", "frequency")];
    all_edges.extend(edges);
    spec(json!({ "nodes": all, "edges": all_edges }))
}

/// One case per node kind that reads the clock or keeps state across
/// evaluations, each node driven by the clock so that it moves.
fn cases() -> Vec<(&'static str, GraphSpec)> {
    let mut cases = vec![
        ("time", driven(vec![], vec![])),
        ("oscillator", driven(vec![], vec![])),
        (
            "spring",
            driven(
                vec![json!({ "id": "node", "type": "spring",
                    "params": { "stiffness": 40.0, "damping": 4.0, "mass": 1.0 } })],
                vec![edge("wave", "node", "in")],
            ),
        ),
        (
            "spring over a vector",
            driven(
                vec![
                    json!({ "id": "pair", "type": "join" }),
                    json!({ "id": "node", "type": "spring", "params": { "stiffness": 40.0, "damping": 4.0 } }),
                ],
                vec![
                    edge("wave", "pair", "operand_0"),
                    edge("clock", "pair", "operand_1"),
                    edge("pair", "node", "in"),
                ],
            ),
        ),
        (
            "damp",
            driven(
                vec![json!({ "id": "node", "type": "damp", "params": { "half_life": 0.3 } })],
                vec![edge("wave", "node", "in")],
            ),
        ),
        (
            "slew",
            driven(
                vec![
                    json!({ "id": "gain", "type": "constant", "params": { "value": f32_json(5.0) } }),
                    json!({ "id": "far", "type": "multiply" }),
                    json!({ "id": "node", "type": "slew", "params": { "max_rate": 1.0 } }),
                ],
                vec![
                    edge("wave", "far", "operand_0"),
                    edge("gain", "far", "operand_1"),
                    edge("far", "node", "in"),
                ],
            ),
        ),
    ];
    for noise in ["simplenoise", "perlinnoise", "simplexnoise"] {
        cases.push((
            noise,
            driven(
                vec![json!({ "id": "node", "type": noise, "params": { "noise_seed": 7.0 } })],
                vec![edge("clock", "node", "x"), edge("wave", "node", "y")],
            ),
        ));
    }
    let function = json!(FUNCTION);
    cases.push((
        "taskrun",
        driven(
            vec![json!({ "id": "node", "type": "taskrun", "params": { "function": function } })],
            vec![],
        ),
    ));
    cases.push((
        "spawn",
        driven(
            vec![json!({ "id": "node", "type": "spawn",
                "params": { "function": function, "record_keys": [PARAM] } })],
            vec![edge("clock", "node", "args_0")],
        ),
    ));
    cases.push((
        "externalfunction",
        driven(
            vec![json!({ "id": "node", "type": "externalfunction",
                "params": { "function": function, "param_ids": [PARAM] } })],
            vec![edge("wave", "node", "args_0")],
        ),
    ));
    #[cfg(feature = "urdf_ik")]
    cases.extend(kinematics_cases());
    cases
}

/// The URDF nodes over the sample arm of the `urdf-ik-position` fixture: IK
/// toward a target that sweeps with the clock, seeded from its last solution,
/// and FK of joints that do.
#[cfg(feature = "urdf_ik")]
fn kinematics_cases() -> Vec<(&'static str, GraphSpec)> {
    let fixture: Json = vizij_test_fixtures::node_graphs::spec("urdf-ik-position")
        .expect("the urdf-ik-position fixture");
    let ik = fixture["spec"]["nodes"]
        .as_array()
        .and_then(|nodes| nodes.iter().find(|node| node["id"] == "ik"))
        .expect("the fixture's IK node");
    let arm = json!({
        "urdf_xml": ik["params"]["urdf_xml"],
        "root_link": "base_link",
        "tip_link": "tool",
    });
    let mut ik_params = arm.clone();
    ik_params["max_iters"] = json!(512);
    ik_params["tol_pos"] = json!(0.02);
    // The arm's tip at rest, swept across it (in y and z) by the wave.
    let rest = serde_json::to_value(vocab::vec3([0.95, 0.0, 0.1])).expect("a vec3 serializes");
    let across = serde_json::to_value(vocab::vec3([0.0, 0.05, 0.05])).expect("a vec3 serializes");
    vec![
        (
            "urdfikposition",
            driven(
                vec![
                    json!({ "id": "rest", "type": "constant", "params": { "value": rest } }),
                    json!({ "id": "across", "type": "constant", "params": { "value": across } }),
                    json!({ "id": "sweep", "type": "multiply" }),
                    json!({ "id": "target", "type": "add" }),
                    json!({ "id": "node", "type": "urdfikposition", "params": ik_params }),
                ],
                vec![
                    edge("across", "sweep", "operand_0"),
                    edge("wave", "sweep", "operand_1"),
                    edge("rest", "target", "operand_0"),
                    edge("sweep", "target", "operand_1"),
                    edge("target", "node", "target_pos"),
                ],
            ),
        ),
        (
            "urdffk",
            driven(
                vec![
                    json!({ "id": "joints", "type": "join" }),
                    json!({ "id": "node", "type": "urdffk", "params": arm }),
                ],
                (0..6)
                    .map(|i| edge("wave", "joints", &format!("operand_{i}")))
                    .chain([edge("joints", "node", "joints")])
                    .collect(),
            ),
        ),
    ]
}

/// A zero step leaves every port and every node's state as the last real
/// step left them, for each kind that reads the clock or keeps state, and
/// requests no run — and so do several zero steps in a row.
#[test]
fn a_zero_step_changes_no_node_that_keeps_time_or_state() {
    for (kind, spec) in cases() {
        let mut rt = GraphRuntime::default();
        for _ in 0..30 {
            step(&mut rt, &spec, DT);
        }
        if kind == "spawn" {
            assert!(
                !rt.spawns.is_empty(),
                "the clock spawns a run each real step"
            );
        }
        let before = observed(&rt, &spec);
        for _ in 0..3 {
            step(&mut rt, &spec, 0.0);
            assert_eq!(
                observed(&rt, &spec),
                before,
                "{kind}: a zero step changed the graph"
            );
            assert!(rt.spawns.is_empty(), "{kind}: a zero step requested a run");
        }
    }
}

/// Zero steps interleaved in a run leave its course unchanged: the run
/// reaches the same ports and states as one stepped without them.
#[test]
fn zero_steps_leave_the_course_of_a_run_unchanged() {
    for (kind, spec) in cases() {
        let mut plain = GraphRuntime::default();
        let mut interleaved = GraphRuntime::default();
        for _ in 0..30 {
            step(&mut plain, &spec, DT);
            step(&mut interleaved, &spec, DT);
            step(&mut interleaved, &spec, 0.0);
        }
        assert_eq!(
            observed(&interleaved, &spec),
            observed(&plain, &spec),
            "{kind}: zero steps changed the course of the run"
        );
    }
}

/// A smoother fed a new target `target` (a constant) in motion toward an
/// old one: the graph and the node of `kind` with `params`.
fn smoother(kind: &str, params: Json) -> GraphSpec {
    spec(json!({
        "nodes": [
            { "id": "target", "type": "constant", "params": { "value": f32_json(0.0) } },
            { "id": "node", "type": kind, "params": params },
        ],
        "edges": [edge("target", "node", "in")],
    }))
}

fn set_target(spec: &mut GraphSpec, value: f32) {
    spec.nodes[0].params.value = Some(Value::F32(value));
}

fn output(rt: &GraphRuntime) -> f32 {
    match rt.outputs.get("node").and_then(|ports| ports.get("out")) {
        Some(port) => vocab::as_float(&port.value).expect("a float output"),
        None => panic!("the node has no output"),
    }
}

fn smoothers() -> [(&'static str, Json); 3] {
    [
        ("spring", json!({ "stiffness": 40.0, "damping": 4.0 })),
        ("damp", json!({ "half_life": 0.3 })),
        ("slew", json!({ "max_rate": 2.0 })),
    ]
}

/// A target that changes at a zero step is recorded, not reached: the
/// smoother's value (and a spring's velocity) hold, and the next real step
/// moves on from there exactly as if the target had changed at that step.
#[test]
fn a_smoother_records_a_target_changed_at_a_zero_step() {
    for (kind, params) in smoothers() {
        let mut spec = smoother(kind, params);
        let mut paused = GraphRuntime::default();
        let mut playing = GraphRuntime::default();
        step(&mut paused, &spec, 0.0);
        step(&mut playing, &spec, 0.0);
        set_target(&mut spec, 1.0);
        for _ in 0..10 {
            step(&mut paused, &spec, DT);
            step(&mut playing, &spec, DT);
        }
        let held = output(&paused);
        assert!(held > 0.0 && held < 1.0, "{kind} in motion ({held})");

        set_target(&mut spec, -1.0);
        let state = format!("{:?}", paused.node_states.get("node"));
        step(&mut paused, &spec, 0.0);
        assert_eq!(
            output(&paused),
            held,
            "{kind}: the value moved at a zero step"
        );
        if kind == "spring" {
            let Some(graph_runtime::NodeRuntimeState::Spring(spring)) =
                paused.node_states.get("node")
            else {
                panic!("the spring keeps its state");
            };
            assert_eq!(spring.target, vec![-1.0], "the new target is recorded");
            assert_ne!(spring.velocity, vec![0.0], "the velocity holds");
        } else {
            assert_eq!(
                format!("{:?}", paused.node_states.get("node")),
                state,
                "{kind}: the state moved at a zero step"
            );
        }

        step(&mut paused, &spec, DT);
        step(&mut playing, &spec, DT);
        assert_eq!(
            output(&paused),
            output(&playing),
            "{kind}: the zero step changed where the next step lands"
        );
    }
}

/// Neither a negative nor a non-finite step advances a smoother.
#[test]
fn a_negative_or_non_finite_dt_holds_a_smoother() {
    for (kind, params) in smoothers() {
        let mut spec = smoother(kind, params);
        let mut rt = GraphRuntime::default();
        step(&mut rt, &spec, 0.0);
        set_target(&mut spec, 1.0);
        for _ in 0..10 {
            step(&mut rt, &spec, DT);
        }
        let held = output(&rt);
        for dt in [-DT, f32::NAN, f32::INFINITY] {
            rt.dt = dt;
            evaluate_all(&mut rt, &spec).expect("the graph evaluates");
            assert_eq!(output(&rt), held, "{kind} moved at dt {dt}");
        }
    }
}

/// A damp with no half-life and a slew with no rate limit do not smooth:
/// they pass their target through, a zero step included.
#[test]
fn an_unsmoothed_damp_or_slew_passes_its_target_through_at_a_zero_step() {
    for (kind, params) in [
        ("damp", json!({ "half_life": 0.0 })),
        ("slew", json!({ "max_rate": 0.0 })),
    ] {
        let mut spec = smoother(kind, params);
        let mut rt = GraphRuntime::default();
        step(&mut rt, &spec, DT);
        set_target(&mut spec, 0.75);
        step(&mut rt, &spec, 0.0);
        assert_eq!(output(&rt), 0.75, "{kind} passes its target through");
    }
}

/// An IK node seeded from its last solution solves again only when what it
/// solves for changes: re-solving a redundant arm from its own solution
/// lands on another one, which would move the joints with nothing changed.
#[cfg(feature = "urdf_ik")]
#[test]
fn an_ik_node_solves_again_only_when_its_target_changes() {
    let (_, sweep) = kinematics_cases()
        .into_iter()
        .find(|(kind, _)| *kind == "urdfikposition")
        .expect("the IK case");
    let ik = sweep
        .nodes
        .iter()
        .find(|node| node.id == "node")
        .expect("the IK node")
        .clone();
    let target =
        |y: f32| serde_json::to_value(vocab::vec3([0.95, y, 0.1])).expect("a vec3 serializes");
    let mut spec = spec(json!({
        "nodes": [
            { "id": "target", "type": "constant", "params": { "value": target(0.02) } },
            serde_json::to_value(&ik).expect("the IK node serializes"),
        ],
        "edges": [edge("target", "node", "target_pos")],
    }));
    let joints = |rt: &GraphRuntime| {
        rt.outputs
            .get("node")
            .and_then(|ports| ports.get("out"))
            .map(|port| port.value.clone())
            .expect("the IK node's joints")
    };

    let mut rt = GraphRuntime::default();
    step(&mut rt, &spec, DT);
    let solved = joints(&rt);
    for dt in [DT, 0.0] {
        step(&mut rt, &spec, dt);
        assert_eq!(joints(&rt), solved, "the joints moved at dt {dt}");
    }

    spec.nodes[0].params.value = Some(serde_json::from_value(target(0.04)).expect("a vec3 value"));
    step(&mut rt, &spec, 0.0);
    assert_ne!(joints(&rt), solved, "a new target is solved for");
}
