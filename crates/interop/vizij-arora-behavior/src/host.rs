//! [`AroraGraphHost`]: bind a Vizij node-graph's [`GraphHost`] callout seam to
//! an Arora [`CallBridge`] (VIZ-53 Step C).
//!
//! This is the *only* place arora call-types meet the graph's host seam. The
//! graph speaks in graph-core's own [`CallTarget`] and Vizij [`VValue`]s;
//! `AroraGraphHost` translates a [`CallTarget::ModuleFn`] into an arora
//! [`Call`] (function id + args), marshals args/results with [`vizij_arora`]'s
//! [`to_arora`](vizij_arora::to_arora)/[`from_arora`](vizij_arora::from_arora),
//! and dispatches through the bridge — exactly the shape a behavior-tree action
//! node uses (`caller.arora_call(module, Call { id, args })` -> `CallResult`).
//!
//! Args convention: a [`ModuleCall`](vizij_graph_core::types::NodeType::ModuleCall)
//! node's `args` value is a Vizij [`Record`](VValue::Record) mapping each
//! parameter's id to its value. A key that parses as a UUID is used directly;
//! otherwise it is derived with [`gen_uuid_from_str`](arora_types::gen_uuid_from_str)
//! (the same derivation `vizij-arora` uses for record field ids). The call's
//! return [`Value`](AValue) becomes the node's output.

use arora_types::call::{Call, CallBridge};
use arora_types::value::StructureField;
use uuid::Uuid;
use vizij_api_core::Value as VValue;
use vizij_graph_core::host::{CallTarget, GraphHost};

/// Adapts an Arora [`CallBridge`] into a Vizij [`GraphHost`], so a node graph
/// evaluated with [`evaluate_all_with_host`] can call arora module functions.
///
/// [`evaluate_all_with_host`]: vizij_graph_core::eval::evaluate_all_with_host
pub struct AroraGraphHost<'a> {
    bridge: &'a mut dyn CallBridge,
}

impl<'a> AroraGraphHost<'a> {
    /// Wrap a call bridge (the engine, or a mock) as a graph host.
    pub fn new(bridge: &'a mut dyn CallBridge) -> Self {
        Self { bridge }
    }
}

impl GraphHost for AroraGraphHost<'_> {
    fn call(&mut self, target: &CallTarget, args: VValue) -> Result<VValue, String> {
        match target {
            CallTarget::ModuleFn { module, function } => {
                let module = Uuid::parse_str(module)
                    .map_err(|e| format!("invalid module id '{module}': {e}"))?;
                let function = Uuid::parse_str(function)
                    .map_err(|e| format!("invalid function id '{function}': {e}"))?;
                let args = args_to_fields(&args)?;
                let result = self
                    .bridge
                    .arora_call(
                        &module,
                        Call {
                            module_id: None,
                            id: function,
                            args,
                        },
                    )
                    .map_err(|e| e.to_string())?;
                vizij_arora::from_arora(&result.ret).map_err(|e| e.to_string())
            }
            // `CallTarget` is `#[non_exhaustive]`: a future `Behavior` target
            // (design note §2) would be dispatched here to a named interpreter.
            other => Err(format!(
                "unsupported call target for AroraGraphHost: {other:?}"
            )),
        }
    }
}

/// Convert a `ModuleCall` args value (a Vizij `Record` of param-id -> value)
/// into the arora `Call`'s `Vec<StructureField>`.
fn args_to_fields(args: &VValue) -> Result<Vec<StructureField>, String> {
    match args {
        VValue::Record(entries) => entries
            .iter()
            .map(|(key, value)| {
                let id =
                    Uuid::parse_str(key).unwrap_or_else(|_| arora_types::gen_uuid_from_str(key));
                let value = vizij_arora::to_arora(value).map_err(|e| e.to_string())?;
                Ok(StructureField {
                    id,
                    value: Box::new(value),
                })
            })
            .collect(),
        other => Err(format!(
            "ModuleCall args must be a Record of param-id -> value, got {other:?}"
        )),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use arora_types::call::{CallError, CallResult, Callable, CallableId};
    use arora_types::value::Value as AValue;
    use std::rc::Rc;
    use vizij_graph_core::eval::{evaluate_all_with_host, GraphRuntime};
    use vizij_graph_core::types::{
        EdgeInputEndpoint, EdgeOutputEndpoint, EdgeSpec, GraphSpec, NodeParams, NodeSpec, NodeType,
    };

    // A trivial "echo" module: `arora_call` returns the value of the first arg
    // unchanged. Enough to prove the seam end to end at the graph level.
    #[derive(Default)]
    struct EchoBridge {
        last_module: Option<Uuid>,
        last_function: Option<Uuid>,
    }

    impl CallBridge for EchoBridge {
        fn arora_call(&mut self, module: &Uuid, call: Call) -> Result<CallResult, CallError> {
            self.last_module = Some(*module);
            self.last_function = Some(call.id);
            let ret = call
                .args
                .first()
                .map(|field| (*field.value).clone())
                .ok_or(CallError::Generic {
                    message: "echo module expects at least one arg".into(),
                })?;
            Ok(CallResult {
                ret,
                mutated: vec![],
            })
        }
        fn arora_register_callable(&mut self, _callable: Rc<dyn Callable>) -> CallableId {
            unimplemented!("echo bridge registers no callables")
        }
        fn arora_unregister_callable(&mut self, _callable_id: &CallableId) {}
        fn arora_call_indirect(&mut self, _callable_id: &CallableId) -> Result<AValue, CallError> {
            unimplemented!("echo bridge dispatches no indirect callables")
        }
    }

    const MODULE: &str = "76697a69-6a00-0000-0d00-000000000000";
    const FUNCTION: &str = "76697a69-6a00-0000-0f00-000000000001";
    const PARAM: &str = "76697a69-6a00-0000-0f01-000000000001";

    /// A graph: Constant(record{PARAM: 0.75}) -> ModuleCall -> Output("result/x").
    /// The echo module hands the arg value back, so 0.75 must reach both the
    /// node's `out` port and the `Output` sink's write.
    fn graph() -> GraphSpec {
        let args = VValue::Record(
            [(PARAM.to_string(), VValue::Float(0.75))]
                .into_iter()
                .collect(),
        );
        GraphSpec {
            nodes: vec![
                NodeSpec {
                    id: "args".into(),
                    kind: NodeType::Constant,
                    params: NodeParams {
                        value: Some(args),
                        ..Default::default()
                    },
                    output_shapes: Default::default(),
                    input_defaults: Default::default(),
                },
                NodeSpec {
                    id: "call".into(),
                    kind: NodeType::ModuleCall,
                    params: NodeParams {
                        module: Some(MODULE.into()),
                        function: Some(FUNCTION.into()),
                        ..Default::default()
                    },
                    output_shapes: Default::default(),
                    input_defaults: Default::default(),
                },
                NodeSpec {
                    id: "out".into(),
                    kind: NodeType::Output,
                    params: NodeParams {
                        path: Some(vizij_api_core::TypedPath::parse("result/x").unwrap()),
                        ..Default::default()
                    },
                    output_shapes: Default::default(),
                    input_defaults: Default::default(),
                },
            ],
            edges: vec![
                EdgeSpec {
                    from: EdgeOutputEndpoint {
                        node_id: "args".into(),
                        output: "out".into(),
                    },
                    to: EdgeInputEndpoint {
                        node_id: "call".into(),
                        input: "args".into(),
                    },
                    selector: None,
                },
                EdgeSpec {
                    from: EdgeOutputEndpoint {
                        node_id: "call".into(),
                        output: "out".into(),
                    },
                    to: EdgeInputEndpoint {
                        node_id: "out".into(),
                        input: "in".into(),
                    },
                    selector: None,
                },
            ],
            version: 0,
            fingerprint: 0,
        }
    }

    #[test]
    fn module_call_flows_through_the_call_bridge() {
        let spec = graph().with_cache();
        let mut rt = GraphRuntime::default();
        let mut bridge = EchoBridge::default();
        let mut host = AroraGraphHost::new(&mut bridge);

        evaluate_all_with_host(&mut rt, &spec, &mut host).expect("evaluation succeeds");

        // The ModuleCall node's output carries the echoed value.
        let out = rt
            .outputs
            .get("call")
            .and_then(|ports| ports.get("out"))
            .expect("call node produced an output");
        assert_eq!(out.value, VValue::Float(0.75));

        // ...and it flows through the Output sink into the write batch.
        let write = rt
            .writes
            .iter()
            .find(|op| op.path.to_string() == "result/x")
            .expect("output write emitted");
        assert_eq!(write.value, VValue::Float(0.75));

        // The host addressed the intended module/function.
        assert_eq!(bridge.last_module, Some(Uuid::parse_str(MODULE).unwrap()));
        assert_eq!(
            bridge.last_function,
            Some(Uuid::parse_str(FUNCTION).unwrap())
        );
    }

    #[test]
    fn missing_target_params_error_clearly() {
        // A ModuleCall with no `module` param errors when reached, and the
        // message names the offending node.
        let mut spec = graph();
        spec.nodes[1].params.module = None;
        let spec = spec.with_cache();

        let mut rt = GraphRuntime::default();
        let mut bridge = EchoBridge::default();
        let mut host = AroraGraphHost::new(&mut bridge);
        let err = evaluate_all_with_host(&mut rt, &spec, &mut host)
            .expect_err("missing module param should error");
        assert!(err.contains("missing required 'module'"), "got: {err}");
    }
}
