//! Control nodes in the graph: the behavior trees of the Arora spikes
//! (semio-ai/arora-sdk#395, #396, #397) written as Vizij graphs, and what
//! halting, memory and a host's own control node add.
//!
//! The functions are host functions behind `externalfunction` and `action`
//! nodes, as module functions are behind Arora's leaves, so the counts read
//! the same as there: `mul(a, b)`, `clamp(x, lo)`, `below(x, limit) ->
//! Status`, and actions that run for a set number of calls.
use std::cell::RefCell;
use std::rc::Rc;

use hashbrown::HashMap;
use uuid::Uuid;
use vizij_api_core::value as vocab;
use vizij_api_core::{TypedPath, Value};

use crate::control::{Children, Control, ControlRegistry, Status};
use crate::eval::graph_runtime::NodeRuntimeState;
use crate::eval::NodeFunctions;
use crate::eval::{evaluate_all_with_controls, evaluate_all_with_functions, GraphRuntime};
use crate::types::{
    EdgeInputEndpoint, EdgeOutputEndpoint, EdgeSpec, GraphSpec, NodeParams, NodeSpec, NodeType,
};

const MUL: Uuid = Uuid::from_u128(0x3A02);
const CLAMP: Uuid = Uuid::from_u128(0x3A03);
const BELOW: Uuid = Uuid::from_u128(0xC401);
const ARG: [Uuid; 2] = [Uuid::from_u128(0xA0), Uuid::from_u128(0xA1)];

/// A host of functions and actions that logs every call and every halt.
#[derive(Default)]
struct Host {
    log: Rc<RefCell<Vec<String>>>,
    /// Per action, how many calls it runs for before it succeeds (`u32::MAX`:
    /// for ever), or fails when negative.
    actions: HashMap<Uuid, (String, i64)>,
    calls: HashMap<Uuid, i64>,
}

impl Host {
    fn action(&mut self, id: u128, name: &str, calls: i64) -> Uuid {
        let id = Uuid::from_u128(id);
        self.actions.insert(id, (name.to_string(), calls));
        id
    }

    fn take(&self) -> Vec<String> {
        self.log.borrow_mut().drain(..).collect()
    }

    fn count(&self, name: &str) -> usize {
        self.log
            .borrow()
            .iter()
            .filter(|entry| *entry == name)
            .count()
    }
}

fn number(args: &[(Uuid, Value)], at: usize) -> f32 {
    vocab::as_float(&args[at].1).unwrap_or(f32::NAN)
}

impl NodeFunctions for Host {
    fn call(&mut self, function: Uuid, args: &[(Uuid, Value)]) -> Result<Value, String> {
        let name = match function {
            MUL => "mul",
            CLAMP => "clamp",
            BELOW => "below",
            _ => {
                let (name, runs) = self
                    .actions
                    .get(&function)
                    .cloned()
                    .ok_or_else(|| format!("no function {function}"))?;
                self.log.borrow_mut().push(name);
                let call = self.calls.entry(function).or_insert(0);
                *call += 1;
                return Ok(if runs < 0 {
                    Status::Failure.value()
                } else if *call >= runs {
                    *call = 0;
                    Status::Success.value()
                } else {
                    Status::Running.value()
                });
            }
        };
        self.log.borrow_mut().push(name.to_string());
        Ok(match function {
            MUL => vocab::float(number(args, 0) * number(args, 1)),
            CLAMP => vocab::float(number(args, 0).max(number(args, 1))),
            _ => if number(args, 0) < number(args, 1) {
                Status::Success
            } else {
                Status::Failure
            }
            .value(),
        })
    }

    fn halt(&mut self, _module: Option<Uuid>, function: Uuid) {
        if let Some((name, _)) = self.actions.get(&function) {
            self.log.borrow_mut().push(format!("halt {name}"));
            self.calls.insert(function, 0);
        }
    }
}

/// A graph under construction.
#[derive(Default)]
struct Graph {
    spec: GraphSpec,
}

impl Graph {
    fn node(&mut self, id: &str, kind: NodeType, params: NodeParams) -> &mut Self {
        self.spec.nodes.push(NodeSpec {
            id: id.to_string(),
            kind,
            params,
            output_shapes: Default::default(),
            input_defaults: Default::default(),
        });
        self
    }

    fn link(&mut self, from: &str, to: &str, input: &str) -> &mut Self {
        self.spec.edges.push(EdgeSpec {
            from: EdgeOutputEndpoint {
                node_id: from.to_string(),
                output: "out".to_string(),
            },
            to: EdgeInputEndpoint {
                node_id: to.to_string(),
                input: input.to_string(),
            },
            selector: None,
        });
        self
    }

    fn constant(&mut self, id: &str, value: f32) -> &mut Self {
        self.node(
            id,
            NodeType::Constant,
            NodeParams {
                value: Some(vocab::float(value)),
                ..Default::default()
            },
        )
    }

    /// An input reading the store key `path`, 0 when it is not staged.
    fn input(&mut self, id: &str, path: &str) -> &mut Self {
        self.node(
            id,
            NodeType::Input,
            NodeParams {
                path: Some(path.parse().unwrap()),
                value: Some(vocab::float(0.0)),
                ..Default::default()
            },
        )
    }

    /// `function(args…)`, a host function behind an `externalfunction`.
    fn call(&mut self, id: &str, function: Uuid, args: &[&str]) -> &mut Self {
        self.node(
            id,
            NodeType::ExternalFunction,
            NodeParams {
                function: Some(function),
                param_ids: Some(ARG[..args.len()].to_vec()),
                ..Default::default()
            },
        );
        for (at, arg) in args.iter().enumerate() {
            self.link(arg, id, &format!("args_{at}"));
        }
        self
    }

    fn action(&mut self, id: &str, function: Uuid) -> &mut Self {
        self.node(
            id,
            NodeType::Action,
            NodeParams {
                function: Some(function),
                ..Default::default()
            },
        )
    }

    fn control(&mut self, id: &str, kind: NodeType, children: &[&str]) -> &mut Self {
        self.node(id, kind, NodeParams::default());
        for (at, child) in children.iter().enumerate() {
            self.link(child, id, &format!("child_{at}"));
        }
        self
    }

    fn condition(&mut self, id: &str, from: &str) -> &mut Self {
        self.node(id, NodeType::Condition, NodeParams::default())
            .link(from, id, "in")
    }

    fn output(&mut self, id: &str, path: &str, from: &str) -> &mut Self {
        self.node(
            id,
            NodeType::Output,
            NodeParams {
                path: Some(path.parse().unwrap()),
                ..Default::default()
            },
        )
        .link(from, id, "in")
    }

    /// `#cm`: `clamp(x: mul(a: {sensor/distance}, b: 100), lo: 0)`, the
    /// distance in centimetres, never negative.
    fn centimetres(&mut self, prefix: &str) -> &mut Self {
        let id = |name: &str| format!("{prefix}{name}");
        self.input(&id("distance"), "sensor/distance")
            .constant(&id("hundred"), 100.0)
            .constant(&id("zero"), 0.0)
            .call(&id("raw"), MUL, &[&id("distance"), &id("hundred")])
            .call(&id("cm"), CLAMP, &[&id("raw"), &id("zero")])
    }

    /// `below(x: from, limit)`.
    fn below(&mut self, id: &str, from: &str, limit: f32) -> &mut Self {
        let limit_id = format!("{id}_limit");
        self.constant(&limit_id, limit)
            .call(id, BELOW, &[from, &limit_id])
    }
}

/// One evaluation, with the store keys `inputs` staged.
fn tick(rt: &mut GraphRuntime, spec: &GraphSpec, host: &mut Host, inputs: &[(&str, Value)]) {
    for (path, value) in inputs {
        rt.set_input(path.parse::<TypedPath>().unwrap(), value.clone(), None);
    }
    evaluate_all_with_functions(rt, spec, host).expect("the graph evaluates");
}

/// The status a node emitted at the last evaluation; `None` when it did not
/// run.
fn status(rt: &GraphRuntime, id: &str) -> Option<Status> {
    rt.outputs
        .get(id)
        .and_then(|ports| ports.get("out"))
        .and_then(|port| Status::of(&port.value))
}

fn written(rt: &GraphRuntime, path: &str) -> Option<Value> {
    rt.writes
        .0
        .iter()
        .rev()
        .find(|op| op.path.to_string() == path)
        .map(|op| op.value.clone())
}

fn distance(value: f32) -> [(&'static str, Value); 1] {
    [("sensor/distance", vocab::float(value))]
}

/// The cascade of the Arora spikes, `below(x: clamp(x: mul(a: distance,
/// b: 100), lo: 0), limit: 30)`, as the child of a sequence: its status
/// follows the sensor, and each tick calls the functions innermost first.
#[test]
fn a_cascade_of_functions_feeds_a_condition() {
    let mut graph = Graph::default();
    graph
        .centimetres("")
        .below("near", "cm", 30.0)
        .control("root", NodeType::Sequence, &["near"])
        .output("status", "task/status", "root");
    let spec = &graph.spec;
    spec.validate().expect("the graph is valid");
    let mut rt = GraphRuntime::default();
    let mut host = Host::default();
    for (metres, expected) in [
        (0.2, Status::Success),
        (0.5, Status::Failure),
        (-0.1, Status::Success),
    ] {
        tick(&mut rt, spec, &mut host, &distance(metres));
        assert_eq!(status(&rt, "root"), Some(expected), "at {metres} m");
        assert_eq!(host.take(), ["mul", "clamp", "below"]);
    }
    assert_eq!(
        written(&rt, "task/status").as_ref().and_then(Status::of),
        Some(Status::Success)
    );
    let index = |id: &str| spec.nodes.iter().position(|n| n.id == id).unwrap();
    assert!(rt.is_lazy(index("cm")) && rt.is_lazy(index("near")));
    assert!(!rt.is_lazy(index("root")) && !rt.is_lazy(index("status")));
}

/// Three children read one value: it runs once per tick, and every tick, as
/// nothing is cached.
#[test]
fn a_value_three_children_read_runs_once_per_tick() {
    let mut graph = Graph::default();
    graph
        .centimetres("")
        .below("near", "cm", 10.0)
        .below("mid", "cm", 30.0)
        .below("far", "cm", 50.0)
        .control("root", NodeType::Parallel, &["near", "mid", "far"]);
    let mut rt = GraphRuntime::default();
    let mut host = Host::default();
    for _ in 0..2 {
        tick(&mut rt, &graph.spec, &mut host, &distance(0.2));
        assert_eq!(
            (host.count("mul"), host.count("clamp"), host.count("below")),
            (1, 1, 3)
        );
        host.take();
    }
}

/// A fallback that succeeds at its first child does not tick the second: the
/// functions only the second needs do not run.
#[test]
fn a_branch_the_fallback_does_not_reach_does_not_run() {
    let mut graph = Graph::default();
    graph
        .input("left", "left")
        .input("right", "right")
        .constant("two", 2.0)
        .constant("three", 3.0)
        .call("l", MUL, &["left", "two"])
        .call("r", MUL, &["right", "three"])
        .below("left_low", "l", 10.0)
        .below("right_low", "r", 10.0)
        .control("root", NodeType::Fallback, &["left_low", "right_low"]);
    let mut rt = GraphRuntime::default();
    let mut host = Host::default();
    let sides =
        |left: f32, right: f32| [("left", vocab::float(left)), ("right", vocab::float(right))];

    tick(&mut rt, &graph.spec, &mut host, &sides(1.0, 1.0));
    assert_eq!(host.take(), ["mul", "below"], "only the left branch");
    assert!(
        !rt.outputs.contains_key("r"),
        "the right branch did not run"
    );

    tick(&mut rt, &graph.spec, &mut host, &sides(9.0, 1.0));
    assert_eq!(host.take(), ["mul", "below", "mul", "below"]);
    assert_eq!(status(&rt, "root"), Some(Status::Success));
}

/// An output outside any control node runs every tick, as a sink does. An
/// output that is a child writes its key only when ticked.
#[test]
fn an_output_inside_control_writes_only_when_ticked() {
    let mut graph = Graph::default();
    graph
        .centimetres("")
        .output("rogue", "out/cm", "cm_copy")
        .call("cm_copy", MUL, &["cm", "one"])
        .constant("one", 1.0)
        .below("near", "cm", 30.0)
        .output("alert", "out/alert", "cm")
        .control("root", NodeType::Sequence, &["near", "alert"]);
    let spec = &graph.spec;
    spec.validate().expect("the graph is valid");
    let mut rt = GraphRuntime::default();
    let mut host = Host::default();

    tick(&mut rt, spec, &mut host, &distance(0.2));
    assert_eq!(written(&rt, "out/cm"), Some(vocab::float(20.0)));
    assert_eq!(written(&rt, "out/alert"), Some(vocab::float(20.0)));

    tick(&mut rt, spec, &mut host, &distance(0.5));
    assert_eq!(written(&rt, "out/cm"), Some(vocab::float(50.0)));
    assert_eq!(
        written(&rt, "out/alert"),
        None,
        "the sequence stopped before it"
    );
}

/// Of two outputs of one key outside control, the later in the graph wins.
#[test]
fn the_later_of_two_outputs_of_one_key_wins() {
    let mut graph = Graph::default();
    graph
        .constant("x", 1.0)
        .constant("y", 2.0)
        .output("first", "out", "x")
        .output("second", "out", "y")
        .constant("yes", 1.0)
        .condition("ok", "yes")
        .control("root", NodeType::Sequence, &["ok"]);
    let mut rt = GraphRuntime::default();
    tick(&mut rt, &graph.spec, &mut Host::default(), &[]);
    assert_eq!(written(&rt, "out"), Some(vocab::float(2.0)));
}

/// What the tree writes reaches what it reads at the next tick: outputs land
/// after the evaluation, and inputs are staged before it.
#[test]
fn a_write_reaches_readers_at_the_next_tick() {
    let mut graph = Graph::default();
    graph
        .input("v", "v")
        .constant("one", 1.0)
        .constant("ten", 10.0)
        .call("next", MUL, &["v", "ten"])
        .output("write", "v", "one")
        .below("check", "next", 100.0)
        .control("root", NodeType::Sequence, &["write", "check"]);
    let mut rt = GraphRuntime::default();
    let mut host = Host::default();
    tick(&mut rt, &graph.spec, &mut host, &[("v", vocab::float(0.0))]);
    assert_eq!(written(&rt, "v"), Some(vocab::float(1.0)));
    assert_eq!(
        rt.outputs["next"]["out"].value,
        vocab::float(0.0),
        "check read the store of before the write"
    );
}

/// A `damp` in a branch is halted when the branch stops being ticked, and
/// starts again from its input when it is ticked anew.
#[test]
fn a_damp_in_a_branch_starts_again_from_its_input() {
    let mut graph = Graph::default();
    graph
        .input("enabled", "enabled")
        .input("target", "target")
        .condition("on", "enabled")
        .node(
            "smooth",
            NodeType::Damp,
            NodeParams {
                half_life: Some(0.1),
                ..Default::default()
            },
        )
        .link("target", "smooth", "in")
        .output("drive", "out", "smooth")
        .control("root", NodeType::Sequence, &["on", "drive"]);
    let mut rt = GraphRuntime {
        dt: 0.05,
        ..Default::default()
    };
    let mut host = Host::default();
    let stage = |enabled: bool, target: f32| {
        [
            ("enabled", vocab::bool_(enabled)),
            ("target", vocab::float(target)),
        ]
    };
    tick(&mut rt, &graph.spec, &mut host, &stage(true, 0.0));
    tick(&mut rt, &graph.spec, &mut host, &stage(true, 1.0));
    let eased = vocab::as_float(&written(&rt, "out").unwrap()).unwrap();
    assert!(eased > 0.0 && eased < 1.0, "it eases: {eased}");
    assert!(rt.node_states.contains_key("smooth"));

    tick(&mut rt, &graph.spec, &mut host, &stage(false, 1.0));
    assert!(!rt.node_states.contains_key("smooth"), "halted");

    tick(&mut rt, &graph.spec, &mut host, &stage(true, 1.0));
    assert_eq!(
        written(&rt, "out"),
        Some(vocab::float(1.0)),
        "from its input, not from where it was"
    );
}

/// A sequence ticks its condition at every tick; when the condition fails,
/// the action it was running is halted, and its call cancelled, at once.
#[test]
fn a_failing_condition_halts_the_running_action() {
    let mut host = Host::default();
    let say = host.action(0x5A1, "say", i64::MAX);
    let mut graph = Graph::default();
    graph
        .input("listening", "listening")
        .condition("ok", "listening")
        .action("speak", say)
        .control("root", NodeType::Sequence, &["ok", "speak"]);
    let mut rt = GraphRuntime::default();
    let listening = |on: bool| [("listening", vocab::bool_(on))];
    tick(&mut rt, &graph.spec, &mut host, &listening(true));
    tick(&mut rt, &graph.spec, &mut host, &listening(true));
    assert_eq!(status(&rt, "root"), Some(Status::Running));
    tick(&mut rt, &graph.spec, &mut host, &listening(false));
    assert_eq!(host.take(), ["say", "say", "halt say"]);
    assert_eq!(status(&rt, "root"), Some(Status::Failure));
}

/// A fallback leaving a running action for the next child cancels the first
/// before it calls the second.
#[test]
fn a_fallback_cancels_the_action_it_leaves_before_the_next_one() {
    let mut host = Host::default();
    let near_say = host.action(0x5A1, "greet", i64::MAX);
    let far_say = host.action(0x5A2, "call out", i64::MAX);
    let mut graph = Graph::default();
    graph
        .centimetres("")
        .below("near", "cm", 30.0)
        .action("greet", near_say)
        .control("close", NodeType::Sequence, &["near", "greet"])
        .action("call_out", far_say)
        .control("root", NodeType::Fallback, &["close", "call_out"]);
    let mut rt = GraphRuntime::default();
    tick(&mut rt, &graph.spec, &mut host, &distance(0.2));
    tick(&mut rt, &graph.spec, &mut host, &distance(0.9));
    let log = host.take();
    let actions: Vec<&str> = log
        .iter()
        .map(String::as_str)
        .filter(|entry| !["mul", "clamp", "below"].contains(entry))
        .collect();
    assert_eq!(actions, ["greet", "halt greet", "call out"]);
}

/// A sequence with memory resumes at the running child; a sequence without
/// ticks its earlier children again, which start again once done.
#[test]
fn a_sequence_with_memory_resumes_at_the_running_child() {
    for (kind, first_calls) in [(NodeType::SequenceWithMemory, 2), (NodeType::Sequence, 4)] {
        let mut host = Host::default();
        let first = host.action(0xF1, "first", 2);
        let second = host.action(0xF2, "second", i64::MAX);
        let mut graph = Graph::default();
        graph
            .action("a", first)
            .action("b", second)
            .control("root", kind.clone(), &["a", "b"]);
        let mut rt = GraphRuntime::default();
        for _ in 0..4 {
            tick(&mut rt, &graph.spec, &mut host, &[]);
        }
        assert_eq!(host.count("first"), first_calls, "{kind:?}");
    }
}

/// A parallel that reaches its threshold halts the children still running.
#[test]
fn a_parallel_halts_its_running_children_when_done() {
    let mut host = Host::default();
    let fast = host.action(0xFA, "fast", 2);
    let slow = host.action(0x51, "slow", i64::MAX);
    let mut graph = Graph::default();
    graph.action("fast", fast).action("slow", slow).node(
        "root",
        NodeType::Parallel,
        NodeParams {
            success_threshold: Some(1),
            ..Default::default()
        },
    );
    graph
        .link("fast", "root", "child_0")
        .link("slow", "root", "child_1");
    let mut rt = GraphRuntime::default();
    tick(&mut rt, &graph.spec, &mut host, &[]);
    assert_eq!(status(&rt, "root"), Some(Status::Running));
    tick(&mut rt, &graph.spec, &mut host, &[]);
    assert_eq!(status(&rt, "root"), Some(Status::Success));
    assert_eq!(host.take(), ["fast", "slow", "fast", "slow", "halt slow"]);
}

/// A node two branches read halts only once neither ticks it, at the end of
/// that tick: one branch stopping does not halt what the other still reads.
#[test]
fn a_shared_node_halts_once_no_branch_ticks_it() {
    let mut graph = Graph::default();
    graph
        .input("a", "a")
        .input("b", "b")
        .input("target", "target")
        .condition("a_on", "a")
        .condition("b_on", "b")
        .node("smooth", NodeType::Damp, NodeParams::default())
        .link("target", "smooth", "in")
        .output("left", "left", "smooth")
        .output("right", "right", "smooth")
        .control("a_side", NodeType::Sequence, &["a_on", "left"])
        .control("b_side", NodeType::Sequence, &["b_on", "right"])
        .control("root", NodeType::Parallel, &["a_side", "b_side"]);
    let mut rt = GraphRuntime {
        dt: 0.05,
        ..Default::default()
    };
    let mut host = Host::default();
    let on = |a: bool, b: bool| {
        [
            ("a", vocab::bool_(a)),
            ("b", vocab::bool_(b)),
            ("target", vocab::float(1.0)),
        ]
    };
    tick(&mut rt, &graph.spec, &mut host, &on(true, true));
    tick(&mut rt, &graph.spec, &mut host, &on(false, true));
    assert!(rt.node_states.contains_key("smooth"), "b still ticks it");
    tick(&mut rt, &graph.spec, &mut host, &on(false, false));
    assert!(!rt.node_states.contains_key("smooth"), "no branch ticks it");
}

/// `retry`: ticks its child again, up to `attempts` times, while it fails.
#[derive(Debug)]
struct Retry {
    attempts: u32,
    failed: u32,
}

impl Control for Retry {
    fn tick(&mut self, children: &mut Children<'_>) -> Result<Status, String> {
        match children.tick(0)? {
            Status::Failure if self.failed + 1 < self.attempts => {
                self.failed += 1;
                children.halt(0)?;
                Ok(Status::Running)
            }
            status => {
                if status != Status::Running {
                    self.failed = 0;
                }
                Ok(status)
            }
        }
    }
}

/// A host adds a control node of its own, through the same trait the
/// built-in ones implement, and a `control` node names it.
#[test]
fn a_host_registers_a_control_node() {
    let mut host = Host::default();
    let flaky = host.action(0xF7, "flaky", -1);
    let mut graph = Graph::default();
    graph.action("try", flaky).node(
        "retry",
        NodeType::Control,
        NodeParams {
            control: Some("retry".into()),
            value: Some(vocab::float(3.0)),
            ..Default::default()
        },
    );
    graph.link("try", "retry", "child_0");
    let mut controls = ControlRegistry::new();
    controls.register("retry", |spec| {
        let attempts = spec
            .params
            .value
            .as_ref()
            .and_then(vocab::as_float)
            .unwrap_or(1.0) as u32;
        Ok(Box::new(Retry {
            attempts,
            failed: 0,
        }))
    });
    let mut rt = GraphRuntime::default();
    let mut statuses = Vec::new();
    for _ in 0..3 {
        evaluate_all_with_controls(&mut rt, &graph.spec, &mut host, &controls).unwrap();
        statuses.push(status(&rt, "retry").unwrap());
    }
    assert_eq!(
        statuses,
        [Status::Running, Status::Running, Status::Failure]
    );
    assert_eq!(host.count("flaky"), 3);
    assert!(matches!(
        rt.node_states.get("retry"),
        Some(NodeRuntimeState::Control(_))
    ));
}

/// A node a control node ticks that a node outside control also reads is
/// refused: the outside reader would run it at every tick.
#[test]
fn a_ticked_node_read_from_outside_control_is_refused() {
    let mut host = Host::default();
    let say = host.action(0x5A1, "say", i64::MAX);
    let mut graph = Graph::default();
    graph
        .action("speak", say)
        .control("root", NodeType::Sequence, &["speak"])
        .output("watch", "spoken", "speak");
    let error = graph.spec.validate().expect_err("refused");
    assert!(
        error.contains("'speak' is a child of a control node")
            && error.contains("'watch' reads it outside any control node"),
        "{error}"
    );
}

/// A graph with no control node evaluates every node, every tick, as before:
/// no node is lazy.
#[test]
fn a_graph_without_control_nodes_runs_every_node() {
    let mut graph = Graph::default();
    graph.centimetres("").output("cm_out", "cm", "cm");
    let mut rt = GraphRuntime::default();
    let mut host = Host::default();
    tick(&mut rt, &graph.spec, &mut host, &distance(0.2));
    assert_eq!(host.take(), ["mul", "clamp"]);
    assert!((0..graph.spec.nodes.len()).all(|node| !rt.is_lazy(node)));
}

/// The counts of the Arora spikes over a hundred ticks: the cascade read by
/// three children of a parallel, the distance changing every tenth tick;
/// then the same three readers under a fallback, whose first child succeeds.
#[test]
fn over_a_hundred_ticks() {
    let run = |kind: NodeType| {
        let mut graph = Graph::default();
        graph
            .centimetres("")
            .below("near", "cm", 10.0)
            .below("mid", "cm", 30.0)
            .below("far", "cm", 50.0)
            .control("root", kind, &["far", "mid", "near"]);
        let mut rt = GraphRuntime::default();
        let mut host = Host::default();
        for tick_index in 0..100 {
            let metres = 0.1 * (tick_index / 10) as f32 / 4.0;
            tick(&mut rt, &graph.spec, &mut host, &distance(metres));
        }
        ["mul", "clamp", "below"].map(|name| host.count(name))
    };
    let parallel = run(NodeType::Parallel);
    let fallback = run(NodeType::Fallback);
    eprintln!(
        "calls over 100 ticks [mul, clamp, below]: parallel {parallel:?}, fallback {fallback:?}"
    );
    assert_eq!(parallel, [100, 100, 300]);
    assert_eq!(
        fallback,
        [100, 100, 100],
        "the first child succeeds every tick"
    );
}

/// An edit rebuilds the plan; the action running before it is still halted
/// when the edited graph stops ticking it.
#[test]
fn an_edit_keeps_what_each_node_last_did() {
    let mut host = Host::default();
    let say = host.action(0x5A1, "say", i64::MAX);
    let mut graph = Graph::default();
    graph
        .input("listening", "listening")
        .condition("ok", "listening")
        .action("speak", say)
        .control("root", NodeType::Sequence, &["ok", "speak"]);
    let mut rt = GraphRuntime::default();
    tick(
        &mut rt,
        &graph.spec,
        &mut host,
        &[("listening", vocab::bool_(true))],
    );
    // The edit adds a node ahead of the others: every index moves.
    graph.spec.nodes.insert(
        0,
        NodeSpec {
            id: "added".to_string(),
            kind: NodeType::Constant,
            params: NodeParams::default(),
            output_shapes: Default::default(),
            input_defaults: Default::default(),
        },
    );
    tick(
        &mut rt,
        &graph.spec,
        &mut host,
        &[("listening", vocab::bool_(false))],
    );
    assert_eq!(host.take(), ["say", "halt say"]);
}
