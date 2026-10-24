//! Where a call to another module goes on the tree, read from the
//! description of the function it calls ([`placement`]): left out when the
//! function returns nothing, and ticked at every tick otherwise, whatever
//! its arguments, as a module's function may keep state they do not show.
use arora_behavior::{FunctionIndex, ModuleFunction};
use arora_types::record::module::frozen::{Function, Parameter};
use arora_types::record::ty::{FrozenTy, PrimitiveKind};
use uuid::Uuid;
use vizij_api_core::Value;

use crate::eval::NodeFunctions;
use crate::functions::{placement, Placement};
use crate::tree_runtime::{evaluate_with_functions, Evaluation, TreeRuntime};
use crate::types::{GraphSpec, NodeSpec};

/// Where the importer places `node`, as `functions` describe what it calls.
fn placed(node: &NodeSpec, functions: &FunctionIndex) -> Placement {
    let kind = node.kind.kind().expect("a node of the vocabulary");
    placement(&kind, &node.params.params(), functions)
}

const FUNCTION: Uuid = Uuid::from_u128(0x2222);
const PARAMETER: Uuid = Uuid::from_u128(0x3333);

/// Counts its calls and returns 42.
#[derive(Default)]
struct Counting {
    calls: usize,
}

impl NodeFunctions for Counting {
    fn call(&mut self, _: Uuid, _: &[(Uuid, Value)]) -> Result<Value, String> {
        self.calls += 1;
        Ok(Value::F32(42.0))
    }
}

/// `FUNCTION` described with `parameters` and returning `returns`.
fn described(parameters: &[Uuid], returns: PrimitiveKind) -> FunctionIndex {
    let function = ModuleFunction {
        module_id: Uuid::from_u128(0x1111),
        function_id: FUNCTION,
        function_name: "f".to_string(),
        function: Function {
            parameter_ordering: parameters.to_vec(),
            parameters: parameters
                .iter()
                .map(|id| {
                    (
                        *id,
                        Parameter {
                            name: "x".to_string(),
                            ty: FrozenTy::from(PrimitiveKind::F32),
                            mutable: false,
                        },
                    )
                })
                .collect(),
            return_ty: FrozenTy::from(returns),
        },
    };
    std::iter::once((FUNCTION, function)).collect()
}

/// An input on `demo/in` passed to the call, whose result is written to
/// `demo/out`. With `parameters` empty, the call takes no argument.
fn graph(parameters: &[Uuid]) -> GraphSpec {
    let mut json = serde_json::json!({
        "nodes": [
            { "id": "in", "type": "input", "params": { "path": "demo/in", "value": 1.0 } },
            { "id": "call", "type": "externalfunction",
              "params": { "function": FUNCTION.to_string(),
                          "param_ids": parameters.iter().map(Uuid::to_string).collect::<Vec<_>>() } },
            { "id": "out", "type": "output", "params": { "path": "demo/out" } }
        ],
        "edges": [
            { "from": { "node_id": "call" }, "to": { "node_id": "out", "input": "in" } }
        ]
    });
    if !parameters.is_empty() {
        json["edges"]
            .as_array_mut()
            .expect("edges")
            .push(serde_json::json!(
                { "from": { "node_id": "in" }, "to": { "node_id": "call", "input": "args_0" } }
            ));
    }
    vizij_api_core::json::normalize_graph_spec_value(&mut json).expect("the spec normalizes");
    serde_json::from_value(json).expect("a graph")
}

/// How many times the call ran over `ticks` evaluations, the input taking
/// each of `inputs` in turn, for each kind of evaluation.
fn calls(spec: &GraphSpec, functions: &FunctionIndex, inputs: &[f32]) -> Vec<usize> {
    [Evaluation::Demanded, Evaluation::Every]
        .into_iter()
        .map(|evaluation| {
            let mut rt = TreeRuntime::new(evaluation);
            rt.load_with_functions(spec, functions)
                .expect("the graph loads");
            let mut host = Counting::default();
            for input in inputs {
                rt.set_input(
                    vizij_api_core::TypedPath::parse("demo/in").expect("a path"),
                    Value::F32(*input),
                    None,
                );
                evaluate_with_functions(&mut rt, &mut host).expect("the graph evaluates");
            }
            host.calls
        })
        .collect()
}

/// The animation module's `step(dt)`: the same `dt` every frame, and the
/// players advanced by each call.
#[test]
fn a_call_whose_arguments_repeat_runs_at_every_tick() {
    let spec = graph(&[PARAMETER]);
    let functions = described(&[PARAMETER], PrimitiveKind::F32);
    assert_eq!(placed(&spec.nodes[1], &functions), Placement::Attached);
    assert_eq!(calls(&spec, &functions, &[1.0, 1.0, 1.0, 2.0]), [4, 4]);

    let mut rt = TreeRuntime::new(Evaluation::Demanded);
    rt.load_with_functions(&spec, &functions)
        .expect("the graph loads");
    evaluate_with_functions(&mut rt, &mut Counting::default()).expect("the graph evaluates");
    assert_eq!(
        rt.writes
            .iter()
            .map(|op| op.value.clone())
            .collect::<Vec<_>>(),
        [Value::F32(42.0)],
        "what it returns reaches the output"
    );
}

#[test]
fn a_call_whose_function_returns_nothing_is_left_out() {
    let spec = graph(&[PARAMETER]);
    let functions = described(&[PARAMETER], PrimitiveKind::Unit);
    assert_eq!(placed(&spec.nodes[1], &functions), Placement::Ignored);
    assert_eq!(calls(&spec, &functions, &[1.0, 2.0, 3.0]), [0, 0]);

    let mut rt = TreeRuntime::default();
    rt.load_with_functions(&spec, &functions)
        .expect("the graph loads");
    evaluate_with_functions(&mut rt, &mut Counting::default()).expect("the graph evaluates");
    let call = rt
        .imported()
        .and_then(|graph| graph.node("call"))
        .expect("the node");
    assert!(call.ignored && call.ports.is_empty());
    assert!(!rt.outputs.contains_key("call"), "it has no ports");
}

#[test]
fn a_call_whose_function_takes_no_parameter_runs_at_every_tick() {
    let spec = graph(&[]);
    let functions = described(&[], PrimitiveKind::F32);
    assert_eq!(placed(&spec.nodes[1], &functions), Placement::Attached);
    assert_eq!(calls(&spec, &functions, &[1.0, 1.0, 1.0]), [3, 3]);
}

#[test]
fn a_call_no_description_names_runs_at_every_tick() {
    let spec = graph(&[PARAMETER]);
    let functions = FunctionIndex::new();
    assert_eq!(placed(&spec.nodes[1], &functions), Placement::Attached);
    assert_eq!(calls(&spec, &functions, &[1.0, 1.0, 1.0]), [3, 3]);
}

#[test]
fn a_task_run_and_a_spawn_run_at_every_tick_whatever_is_described() {
    let mut node = graph(&[PARAMETER]).nodes[1].clone();
    let functions = described(&[PARAMETER], PrimitiveKind::F32);
    for kind in [
        crate::types::NodeType::TaskRun,
        crate::types::NodeType::Spawn,
    ] {
        node.kind = kind;
        assert_eq!(placed(&node, &functions), Placement::Attached);
    }
}
