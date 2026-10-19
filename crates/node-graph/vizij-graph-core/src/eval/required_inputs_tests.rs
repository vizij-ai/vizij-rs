//! The registry's required inputs agree with the evaluator: a node fails
//! without each input [`required_inputs`] names, and evaluates without any
//! other.

use super::*;
use crate::schema::{registry, required_inputs, NodeSignature, PortSpec, PortType};
use crate::types::{InputDefault, NodeParams, NodeSpec, NodeType};
use hashbrown::HashMap;
use uuid::Uuid;
use vizij_api_core::value as vocab;
use vizij_api_core::{TypedPath, Value};

/// The pose of the fixture arm's tool with its joints at [`REST`]: a target
/// its IK nodes reach from there.
struct Rest {
    position: Value,
    rotation: Value,
}

#[cfg(feature = "urdf_ik")]
fn rest() -> Rest {
    let spec = GraphSpec {
        nodes: vec![NodeSpec {
            id: "fk".to_string(),
            kind: NodeType::UrdfFk,
            params: params(&NodeType::UrdfFk),
            output_shapes: HashMap::new(),
            input_defaults: [(
                "joints".to_string(),
                InputDefault {
                    value: vocab::vector(REST.to_vec()),
                    shape: None,
                },
            )]
            .into_iter()
            .collect(),
        }],
        ..Default::default()
    }
    .with_cache();
    let mut rt = GraphRuntime::default();
    evaluate_all(&mut rt, &spec).expect("the arm's FK at rest");
    let port = |name: &str| rt.outputs["fk"][name].value.clone();
    Rest {
        position: port("position"),
        rotation: port("rotation"),
    }
}

#[cfg(not(feature = "urdf_ik"))]
fn rest() -> Rest {
    Rest {
        position: vocab::vec3([0.0; 3]),
        rotation: vocab::quat([0.0, 0.0, 0.0, 1.0]),
    }
}

/// A pose of the fixture's six joints away from the arm's singular straight
/// pose, where a pose solve cannot invert its Jacobian.
const REST: [f32; 6] = [0.1, 0.2, 0.3, 0.1, 0.2, 0.3];

/// A value `port` accepts on every node that has it.
fn sample(port: &PortSpec, rest: &Rest) -> Value {
    match (port.id, &port.ty) {
        ("target_pos", _) => rest.position.clone(),
        ("target_rot", _) => rest.rotation.clone(),
        // One value per joint of the fixture's arm.
        ("seed" | "joints", _) => vocab::vector(REST.to_vec()),
        // A run's live arguments are a bundle of call args.
        ("args", _) => Value::Structure(vizij_api_core::value::Structure {
            id: Uuid::from_u128(0xA265),
            fields: Vec::new(),
        }),
        (_, PortType::Float | PortType::Any) => vocab::float(0.5),
        (_, PortType::Bool) => vocab::bool_(true),
        (_, PortType::Vec3) => vocab::vec3([0.0, 0.0, 0.0]),
        (_, PortType::Quat) => vocab::quat([0.0, 0.0, 0.0, 1.0]),
        (_, PortType::Transform) => vocab::transform(vocab::Transform {
            translation: [0.0; 3],
            rotation: [0.0, 0.0, 0.0, 1.0],
            scale: [1.0; 3],
        }),
        (_, PortType::Vector) => vocab::vector(vec![0.0, 1.0]),
    }
}

/// What a node of `kind` needs besides its inputs to evaluate: the path an
/// `Input` reads, the function a call node makes, the arm a kinematics node
/// solves.
fn params(kind: &NodeType) -> NodeParams {
    let function = Some(Uuid::from_u128(0xF00D));
    match kind {
        NodeType::Input => NodeParams {
            path: Some(TypedPath::parse("sensor/x").expect("a path")),
            ..Default::default()
        },
        NodeType::ExternalFunction | NodeType::TaskRun | NodeType::Spawn => NodeParams {
            function,
            ..Default::default()
        },
        #[cfg(feature = "urdf_ik")]
        NodeType::UrdfIkPosition | NodeType::UrdfIkPose | NodeType::UrdfFk => {
            let fixture: serde_json::Value =
                vizij_test_fixtures::node_graphs::spec("urdf-ik-position")
                    .expect("the urdf-ik-position fixture");
            let urdf = fixture["spec"]["nodes"]
                .as_array()
                .and_then(|nodes| nodes.iter().find(|node| node["id"] == "ik"))
                .and_then(|ik| ik["params"]["urdf_xml"].as_str())
                .expect("the fixture's arm");
            NodeParams {
                urdf_xml: Some(urdf.to_string()),
                root_link: Some("base_link".to_string()),
                tip_link: Some("tool".to_string()),
                // What an IK node starts from with no `seed` input.
                seed: Some(REST.to_vec()),
                ..Default::default()
            }
        }
        _ => NodeParams::default(),
    }
}

/// Answers every call with a float.
struct Answering;

impl NodeFunctions for Answering {
    fn call(&mut self, _function: Uuid, _args: &[(Uuid, Value)]) -> Result<Value, String> {
        Ok(vocab::float(1.0))
    }
}

/// A graph of one node of `signature`'s type, each of `inputs` given an
/// inline default.
fn graph(signature: &NodeSignature, inputs: &[&PortSpec], rest: &Rest) -> GraphSpec {
    GraphSpec {
        nodes: vec![NodeSpec {
            id: "node".to_string(),
            kind: signature.type_id.clone(),
            params: params(&signature.type_id),
            output_shapes: HashMap::new(),
            input_defaults: inputs
                .iter()
                .map(|port| {
                    (
                        port.id.to_string(),
                        InputDefault {
                            value: sample(port, rest),
                            shape: None,
                        },
                    )
                })
                .collect(),
        }],
        edges: Vec::new(),
        version: 0,
        fingerprint: 0,
    }
    .with_cache()
}

/// Evaluate `spec` once, its `Input` nodes' paths staged.
fn evaluates(spec: &GraphSpec) -> Result<(), String> {
    let mut rt = GraphRuntime::default();
    rt.set_input(
        TypedPath::parse("sensor/x").expect("a path"),
        vocab::float(0.5),
        None,
    );
    evaluate_all_with_functions(&mut rt, spec, &mut Answering)
}

/// With every input given, every node kind evaluates: what the tests below
/// take away is the only thing missing.
#[test]
fn every_node_evaluates_with_all_its_inputs() {
    let rest = rest();
    for signature in registry().nodes {
        let all: Vec<&PortSpec> = signature.inputs.iter().collect();
        evaluates(&graph(&signature, &all, &rest))
            .unwrap_or_else(|e| panic!("{:?} with every input: {e}", signature.type_id));
    }
}

/// A node fails without an input the registry marks required, and evaluates
/// without any other one.
#[test]
fn a_node_fails_without_exactly_its_required_inputs() {
    let rest = rest();
    for signature in registry().nodes {
        let required = required_inputs(&signature.type_id);
        for port in &signature.inputs {
            let others: Vec<&PortSpec> = signature
                .inputs
                .iter()
                .filter(|other| other.id != port.id)
                .collect();
            let outcome = evaluates(&graph(&signature, &others, &rest));
            if required.contains(&port.id) {
                outcome.expect_err(&format!(
                    "{:?} is marked as requiring '{}', yet evaluates without it",
                    signature.type_id, port.id
                ));
            } else {
                outcome.unwrap_or_else(|e| {
                    panic!(
                        "{:?} fails without '{}', which is not marked required: {e}",
                        signature.type_id, port.id
                    )
                });
            }
        }
    }
}

/// Every required input names one of its node's declared input ports.
#[test]
fn required_inputs_are_declared_ports() {
    for signature in registry().nodes {
        for port in required_inputs(&signature.type_id) {
            assert!(
                signature.inputs.iter().any(|input| input.id == *port),
                "{:?} requires '{port}', which it does not declare",
                signature.type_id
            );
        }
    }
}

/// The registry's JSON marks each required input, and only those.
#[test]
fn the_registry_json_marks_required_inputs() {
    let json = serde_json::to_value(registry()).expect("the registry serializes");
    for node in json["nodes"].as_array().expect("nodes") {
        let kind: NodeType = serde_json::from_value(node["type_id"].clone()).expect("a node type");
        let required = required_inputs(&kind);
        for input in node["inputs"].as_array().expect("inputs") {
            let id = input["id"].as_str().expect("a port id");
            assert_eq!(
                input.get("required").and_then(serde_json::Value::as_bool),
                required.contains(&id).then_some(true),
                "{kind:?}'s '{id}' in {input}"
            );
        }
    }
}

// =============================================================================
// GraphSpec::validate
// =============================================================================

fn node(id: &str, kind: NodeType) -> NodeSpec {
    NodeSpec {
        id: id.to_string(),
        kind,
        params: NodeParams {
            value: Some(vocab::float(0.5)),
            ..Default::default()
        },
        output_shapes: HashMap::new(),
        input_defaults: HashMap::new(),
    }
}

fn edge(from: &str, to: &str, input: &str) -> crate::types::EdgeSpec {
    crate::types::EdgeSpec {
        from: crate::types::EdgeOutputEndpoint {
            node_id: from.to_string(),
            output: "out".to_string(),
        },
        to: crate::types::EdgeInputEndpoint {
            node_id: to.to_string(),
            input: input.to_string(),
        },
        selector: None,
    }
}

/// A Remap fed by `source` on every input but `in`, and an Output of it.
fn remap_without_in() -> GraphSpec {
    GraphSpec {
        nodes: vec![
            node("source", NodeType::Constant),
            node("remap", NodeType::Remap),
            node("output", NodeType::Output),
        ],
        edges: ["in_min", "in_max", "out_min", "out_max"]
            .into_iter()
            .map(|input| edge("source", "remap", input))
            .chain([edge("remap", "output", "in")])
            .collect(),
        ..Default::default()
    }
}

#[test]
fn validate_refuses_an_unwired_required_input_naming_the_node_and_port() {
    let spec = remap_without_in();
    let error = spec.validate().expect_err("Remap has no 'in'");
    assert!(error.contains("'remap'"), "{error}");
    assert!(error.contains("'in'"), "{error}");
    evaluate_all(&mut GraphRuntime::default(), &spec.clone().with_cache())
        .expect_err("the graph it refuses fails to evaluate");
}

#[test]
fn validate_accepts_a_required_input_wired_by_an_edge_or_a_default() {
    let mut wired = remap_without_in();
    wired.edges.push(edge("source", "remap", "in"));
    wired.validate().expect("an edge wires 'in'");

    let mut defaulted = remap_without_in();
    defaulted.nodes[1].input_defaults.insert(
        "in".to_string(),
        InputDefault {
            value: vocab::float(0.25),
            shape: None,
        },
    );
    defaulted.validate().expect("an inline default wires 'in'");
}

/// An input the registry calls non-optional but that reads as zero unwired
/// is not required: a graph leaving it unwired evaluates, and is accepted.
#[test]
fn validate_accepts_an_unwired_input_that_reads_as_zero() {
    let spec = GraphSpec {
        nodes: vec![node("output", NodeType::Output), node("add", NodeType::Add)],
        ..Default::default()
    };
    spec.validate().expect("an unwired Output and Add evaluate");
    evaluate_all(&mut GraphRuntime::default(), &spec.with_cache()).expect("and they do");
}

/// A graph whose input path has no value staged yet is accepted: the value
/// can arrive before it evaluates.
#[test]
fn validate_accepts_an_input_with_nothing_staged() {
    let mut input = node("input", NodeType::Input);
    input.params.path = Some(TypedPath::parse("sensor/x").expect("a path"));
    let spec = GraphSpec {
        nodes: vec![input, node("output", NodeType::Output)],
        edges: vec![edge("input", "output", "in")],
        ..Default::default()
    };
    spec.validate()
        .expect("nothing staged is not a wiring problem");
}

#[test]
fn validate_refuses_what_the_plan_refuses() {
    let base = || GraphSpec {
        nodes: vec![node("a", NodeType::Constant), node("b", NodeType::Output)],
        edges: vec![edge("a", "b", "in")],
        ..Default::default()
    };
    let cases: Vec<(&str, GraphSpec)> = vec![
        ("missing source node 'ghost'", {
            let mut spec = base();
            spec.edges.push(edge("ghost", "b", "x"));
            spec
        }),
        ("missing target node 'ghost'", {
            let mut spec = base();
            spec.edges.push(edge("a", "ghost", "in"));
            spec
        }),
        ("duplicate edge for input 'b:in'", {
            let mut spec = base();
            spec.edges.push(edge("a", "b", "in"));
            spec
        }),
        ("two nodes share an id", {
            let mut spec = base();
            spec.nodes.push(node("a", NodeType::Constant));
            spec
        }),
        ("cycle detected", {
            let mut spec = base();
            spec.nodes.push(node("c", NodeType::Add));
            spec.edges.push(edge("c", "c", "operand_0"));
            spec
        }),
    ];
    for (expected, spec) in cases {
        let error = spec.validate().expect_err(expected);
        assert!(error.contains(expected), "{expected}: {error}");
        evaluate_all(&mut GraphRuntime::default(), &spec.with_cache())
            .expect_err("the graph it refuses fails to evaluate");
    }
}
