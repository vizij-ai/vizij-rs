// ExternalFunction node unit tests: host dispatch, arg zipping, and the no-host error path.

use super::*;
use crate::types::{
    EdgeInputEndpoint, EdgeOutputEndpoint, EdgeSpec, GraphSpec, NodeParams, NodeSpec, NodeType,
};
use hashbrown::HashMap;
use uuid::Uuid;
use vizij_api_core::value as vocab;
use vizij_api_core::Value;

/// Records each `call` it receives and always returns Float(42.0).
#[derive(Default)]
struct RecordingFunctions {
    calls: Vec<(String, Vec<(Uuid, Value)>)>,
}

impl NodeFunctions<Value> for RecordingFunctions {
    fn call(&mut self, function: &str, args: &[(Uuid, Value)]) -> Result<Value, String> {
        self.calls.push((function.to_string(), args.to_vec()));
        Ok(vocab::float(42.0))
    }
}

fn constant_node(id: &str, value: Value) -> NodeSpec<Value> {
    NodeSpec {
        id: id.to_string(),
        kind: NodeType::Constant,
        params: NodeParams {
            value: Some(value),
            ..Default::default()
        },
        output_shapes: HashMap::new(),
        input_defaults: HashMap::new(),
    }
}

fn link(from: &str, to: &str, input: &str) -> EdgeSpec {
    EdgeSpec {
        from: EdgeOutputEndpoint {
            node_id: from.to_string(),
            output: "out".to_string(),
        },
        to: EdgeInputEndpoint {
            node_id: to.to_string(),
            input: input.to_string(),
        },
        selector: None,
    }
}

fn external_function_graph(function_id: Uuid, param_id: Uuid) -> GraphSpec<Value> {
    GraphSpec {
        nodes: vec![
            constant_node("arg0", Value::F32(7.0)),
            NodeSpec {
                id: "call".to_string(),
                kind: NodeType::ExternalFunction,
                params: NodeParams {
                    function: Some(function_id.to_string()),
                    param_ids: Some(vec![param_id]),
                    ..Default::default()
                },
                output_shapes: HashMap::new(),
                input_defaults: HashMap::new(),
            },
        ],
        // The variadic "args" group is addressed as args_0, args_1, ...
        edges: vec![link("arg0", "call", "args_0")],
        ..Default::default()
    }
    .with_cache()
}

#[test]
fn external_function_dispatches_through_host_and_sets_output() {
    let function_id = Uuid::from_u128(0x2222);
    let param_id = Uuid::from_u128(0x3333);
    let graph = external_function_graph(function_id, param_id);

    let mut rt = GraphRuntime::default();
    let mut functions = RecordingFunctions::default();
    evaluate_all_with_functions(&mut rt, &graph, &mut functions)
        .expect("external function should evaluate");

    // Output "out" carries the host's returned value.
    let out = rt
        .outputs
        .get("call")
        .and_then(|ports| ports.get("out"))
        .expect("call out present");
    match &out.value {
        Value::F32(f) => assert!((*f - 42.0).abs() < 1e-6, "expected 42.0, got {f}"),
        other => panic!("expected float out, got {other:?}"),
    }

    // The host received exactly one call with the configured id and zipped arg.
    assert_eq!(functions.calls.len(), 1, "expected a single invocation");
    let (ref got_function, ref args) = functions.calls[0];
    assert_eq!(*got_function, function_id.to_string(), "function id");
    assert_eq!(args.len(), 1, "expected one positional arg");
    assert_eq!(args[0].0, param_id, "arg key should match param_ids");
    match &args[0].1 {
        Value::F32(f) => assert!((*f - 7.0).abs() < 1e-6, "expected arg 7.0, got {f}"),
        other => panic!("expected float arg, got {other:?}"),
    }
}

#[test]
fn external_function_without_host_errors() {
    let graph = external_function_graph(Uuid::from_u128(0x2222), Uuid::from_u128(0x3333));

    let mut rt = GraphRuntime::default();
    let err = evaluate_all(&mut rt, &graph)
        .expect_err("evaluating an ExternalFunction without a host must error");
    assert!(
        err.contains("without a function host"),
        "unexpected error message: {err}"
    );
}

#[test]
fn external_function_missing_id_errors() {
    let graph = GraphSpec {
        nodes: vec![NodeSpec {
            id: "call".to_string(),
            kind: NodeType::ExternalFunction,
            params: NodeParams::default(),
            output_shapes: HashMap::new(),
            input_defaults: HashMap::new(),
        }],
        edges: vec![],
        ..Default::default()
    }
    .with_cache();

    let mut rt = GraphRuntime::default();
    let mut functions = RecordingFunctions::default();
    let err = evaluate_all_with_functions(&mut rt, &graph, &mut functions)
        .expect_err("missing function id must error");
    assert!(
        err.contains("requires a function id"),
        "unexpected error message: {err}"
    );
}
