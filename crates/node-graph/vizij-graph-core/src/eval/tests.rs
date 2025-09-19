//! Behavioural coverage for the evaluation pipeline.

use super::*;
use crate::types::{GraphSpec, InputConnection, NodeParams, NodeSpec, NodeType};
use hashbrown::HashMap;
use vizij_api_core::{Shape, ShapeId, TypedPath, Value};

fn constant_node(id: &str, value: Value) -> NodeSpec {
    NodeSpec {
        id: id.to_string(),
        kind: NodeType::Constant,
        params: NodeParams {
            value: Some(value),
            ..Default::default()
        },
        inputs: HashMap::new(),
        output_shapes: HashMap::new(),
    }
}

// --- Shape validation ----------------------------------------------------

#[test]
fn it_should_respect_declared_shape() {
    let mut node = constant_node("a", Value::Float(1.0));
    node.output_shapes
        .insert("out".to_string(), Shape::new(ShapeId::Scalar));

    let spec = GraphSpec { nodes: vec![node] };
    let mut rt = GraphRuntime::default();
    evaluate_all(&mut rt, &spec).expect("shape should match");
    let outputs = rt.outputs.get("a").expect("outputs present");
    let port = outputs.get("out").expect("out port present");
    assert!(matches!(port.shape.id, ShapeId::Scalar));
}

#[test]
fn it_should_error_when_shape_mismatches() {
    let mut node = constant_node("a", Value::Float(1.0));
    node.output_shapes
        .insert("out".to_string(), Shape::new(ShapeId::Vec3));

    let spec = GraphSpec { nodes: vec![node] };
    let mut rt = GraphRuntime::default();
    let err = evaluate_all(&mut rt, &spec).expect_err("should fail due to mismatch");
    assert!(err.contains("does not match declared shape"));
}

// --- Runtime outputs -----------------------------------------------------

#[test]
fn it_should_emit_write_for_output_nodes() {
    let mut output_inputs = HashMap::new();
    output_inputs.insert(
        "in".to_string(),
        InputConnection {
            node_id: "src".to_string(),
            output_key: "out".to_string(),
        },
    );

    let graph = GraphSpec {
        nodes: vec![
            constant_node("src", Value::Float(2.0)),
            NodeSpec {
                id: "out".to_string(),
                kind: NodeType::Output,
                params: NodeParams {
                    path: Some(TypedPath::parse("robot1/Arm/Joint.angle").expect("valid path")),
                    ..Default::default()
                },
                inputs: output_inputs,
                output_shapes: HashMap::new(),
            },
        ],
    };

    let mut rt = GraphRuntime::default();
    evaluate_all(&mut rt, &graph).expect("graph should evaluate");
    assert_eq!(rt.writes.iter().count(), 1);
    let op = rt.writes.iter().next().expect("write present");
    assert_eq!(op.path.to_string(), "robot1/Arm/Joint.angle");
    match op.value {
        Value::Float(f) => assert_eq!(f, 2.0),
        _ => panic!("expected float write"),
    }
}

// --- Variadic & oscillator behaviour ------------------------------------

#[test]
fn join_respects_operand_order() {
    let mut inputs = HashMap::new();
    inputs.insert(
        "operands_1".to_string(),
        InputConnection {
            node_id: "a".to_string(),
            output_key: "out".to_string(),
        },
    );
    inputs.insert(
        "operands_2".to_string(),
        InputConnection {
            node_id: "b".to_string(),
            output_key: "out".to_string(),
        },
    );
    inputs.insert(
        "operands_3".to_string(),
        InputConnection {
            node_id: "c".to_string(),
            output_key: "out".to_string(),
        },
    );

    let graph = GraphSpec {
        nodes: vec![
            constant_node("a", Value::Vector(vec![1.0, 2.0])),
            constant_node("b", Value::Vector(vec![3.0])),
            constant_node("c", Value::Vector(vec![4.0, 5.0])),
            NodeSpec {
                id: "join".to_string(),
                kind: NodeType::Join,
                params: NodeParams::default(),
                inputs,
                output_shapes: HashMap::new(),
            },
        ],
    };

    let mut rt = GraphRuntime::default();
    evaluate_all(&mut rt, &graph).expect("join should evaluate");
    let outputs = rt.outputs.get("join").expect("join outputs present");
    let port = outputs.get("out").expect("out port present");
    match &port.value {
        Value::Vector(vec) => assert_eq!(vec, &vec![1.0, 2.0, 3.0, 4.0, 5.0]),
        other => panic!("expected vector output, got {:?}", other),
    }
}

#[test]
fn oscillator_broadcasts_vector_inputs() {
    let mut inputs = HashMap::new();
    inputs.insert(
        "frequency".to_string(),
        InputConnection {
            node_id: "freq".to_string(),
            output_key: "out".to_string(),
        },
    );
    inputs.insert(
        "phase".to_string(),
        InputConnection {
            node_id: "phase".to_string(),
            output_key: "out".to_string(),
        },
    );

    let graph = GraphSpec {
        nodes: vec![
            constant_node("freq", Value::Vector(vec![1.0, 2.0, 3.0])),
            constant_node("phase", Value::Float(0.0)),
            NodeSpec {
                id: "osc".to_string(),
                kind: NodeType::Oscillator,
                params: NodeParams::default(),
                inputs,
                output_shapes: HashMap::new(),
            },
        ],
    };

    let mut rt = GraphRuntime {
        t: 0.5,
        ..Default::default()
    };
    evaluate_all(&mut rt, &graph).expect("oscillator should evaluate");

    let outputs = rt.outputs.get("osc").expect("osc outputs present");
    let port = outputs.get("out").expect("osc out port present");
    let expected: Vec<f32> = vec![1.0, 2.0, 3.0]
        .into_iter()
        .map(|f| (std::f32::consts::TAU * f * rt.t).sin())
        .collect();

    match &port.value {
        Value::Vector(vec) => {
            assert_eq!(vec.len(), expected.len());
            for (actual, expected) in vec.iter().zip(expected.iter()) {
                assert!(
                    (actual - expected).abs() < 1e-6,
                    "expected {expected}, got {actual}"
                );
            }
        }
        other => panic!("expected vector output, got {:?}", other),
    }
}

// --- Shape inference -----------------------------------------------------

#[test]
fn it_should_infer_vector_length_hints() {
    let node = constant_node("vec", Value::Vector(vec![1.0, 2.0, 3.0]));
    let spec = GraphSpec { nodes: vec![node] };
    let mut rt = GraphRuntime::default();
    evaluate_all(&mut rt, &spec).expect("graph should evaluate");
    let outputs = rt.outputs.get("vec").expect("outputs present");
    let port = outputs.get("out").expect("out port");
    match &port.shape.id {
        ShapeId::Vector { len } => assert_eq!(*len, Some(3)),
        other => panic!("expected vector shape, got {:?}", other),
    }
}

// --- Declared shape error handling --------------------------------------

#[test]
fn it_should_error_when_declared_output_missing() {
    let mut node = constant_node("a", Value::Float(1.0));
    node.output_shapes
        .insert("secondary".to_string(), Shape::new(ShapeId::Scalar));

    let spec = GraphSpec { nodes: vec![node] };
    let mut rt = GraphRuntime::default();
    let err = evaluate_all(&mut rt, &spec).expect_err("missing declared output should error");
    assert!(err.contains("missing declared output"));
}

#[test]
fn it_should_validate_vector_length_against_declared_shape() {
    let mut node = constant_node("a", Value::Vector(vec![1.0, 2.0, 3.0]));
    node.output_shapes.insert(
        "out".to_string(),
        Shape::new(ShapeId::Vector { len: Some(4) }),
    );

    let spec = GraphSpec { nodes: vec![node] };
    let mut rt = GraphRuntime::default();
    let err = evaluate_all(&mut rt, &spec).expect_err("vector length mismatch should error");
    assert!(err.contains("does not match declared shape"));
}

#[test]
fn it_should_reject_invalid_paths_during_deserialization() {
    let json = r#"{
        "id": "node",
        "type": "output",
        "params": { "path": "robot/invalid/" },
        "inputs": {},
        "output_shapes": {}
    }"#;

    let err = serde_json::from_str::<NodeSpec>(json)
        .expect_err("invalid typed path should fail to parse");
    assert!(err.to_string().contains("path"));
}

// --- Stateful nodes ------------------------------------------------------

#[test]
fn spring_node_transitions_toward_new_target() {
    let mut spring_inputs = HashMap::new();
    spring_inputs.insert(
        "in".to_string(),
        InputConnection {
            node_id: "target".to_string(),
            output_key: "out".to_string(),
        },
    );

    let spring = NodeSpec {
        id: "spring".to_string(),
        kind: NodeType::Spring,
        params: NodeParams {
            stiffness: Some(30.0),
            damping: Some(6.0),
            mass: Some(1.0),
            ..Default::default()
        },
        inputs: spring_inputs,
        output_shapes: HashMap::new(),
    };

    let mut spec = GraphSpec {
        nodes: vec![constant_node("target", Value::Float(0.0)), spring],
    };

    let mut rt = GraphRuntime::default();
    evaluate_all(&mut rt, &spec).expect("initial evaluate");

    spec.nodes[0].params.value = Some(Value::Float(10.0));

    rt.dt = 1.0 / 60.0;
    rt.t += rt.dt;
    evaluate_all(&mut rt, &spec).expect("first step");
    let first = match rt
        .outputs
        .get("spring")
        .and_then(|map| map.get("out"))
        .map(|pv| pv.value.clone())
        .expect("spring output")
    {
        Value::Float(f) => f,
        other => panic!("expected float, got {:?}", other),
    };
    assert!(
        (first - 10.0).abs() > 0.01,
        "spring should not immediately reach target"
    );

    for _ in 0..240 {
        rt.dt = 1.0 / 60.0;
        rt.t += rt.dt;
        evaluate_all(&mut rt, &spec).expect("subsequent step");
    }

    let final_val = match rt
        .outputs
        .get("spring")
        .and_then(|map| map.get("out"))
        .map(|pv| pv.value.clone())
        .expect("spring output")
    {
        Value::Float(f) => f,
        other => panic!("expected float, got {:?}", other),
    };
    assert!(
        (final_val - 10.0).abs() < 0.1,
        "spring should converge to target"
    );
}

#[test]
fn damp_node_smooths_toward_target() {
    let mut damp_inputs = HashMap::new();
    damp_inputs.insert(
        "in".to_string(),
        InputConnection {
            node_id: "target".to_string(),
            output_key: "out".to_string(),
        },
    );

    let damp = NodeSpec {
        id: "damp".to_string(),
        kind: NodeType::Damp,
        params: NodeParams {
            half_life: Some(0.2),
            ..Default::default()
        },
        inputs: damp_inputs,
        output_shapes: HashMap::new(),
    };

    let mut spec = GraphSpec {
        nodes: vec![constant_node("target", Value::Float(0.0)), damp],
    };

    let mut rt = GraphRuntime::default();
    evaluate_all(&mut rt, &spec).expect("initial evaluate");

    spec.nodes[0].params.value = Some(Value::Float(1.0));
    rt.dt = 0.1;
    rt.t += rt.dt;
    evaluate_all(&mut rt, &spec).expect("first step");

    let first = match rt
        .outputs
        .get("damp")
        .and_then(|map| map.get("out"))
        .map(|pv| pv.value.clone())
        .expect("damp output")
    {
        Value::Float(f) => f,
        other => panic!("expected float, got {:?}", other),
    };
    assert!(first > 0.0 && first < 1.0, "damp should move but not snap");

    for _ in 0..20 {
        rt.dt = 0.1;
        rt.t += rt.dt;
        evaluate_all(&mut rt, &spec).expect("subsequent step");
    }

    let final_val = match rt
        .outputs
        .get("damp")
        .and_then(|map| map.get("out"))
        .map(|pv| pv.value.clone())
        .expect("damp output")
    {
        Value::Float(f) => f,
        other => panic!("expected float, got {:?}", other),
    };
    assert!(
        (final_val - 1.0).abs() < 0.05,
        "damp should approach target"
    );
}

#[test]
fn slew_node_limits_rate_of_change() {
    let mut slew_inputs = HashMap::new();
    slew_inputs.insert(
        "in".to_string(),
        InputConnection {
            node_id: "target".to_string(),
            output_key: "out".to_string(),
        },
    );

    let slew = NodeSpec {
        id: "slew".to_string(),
        kind: NodeType::Slew,
        params: NodeParams {
            max_rate: Some(2.0),
            ..Default::default()
        },
        inputs: slew_inputs,
        output_shapes: HashMap::new(),
    };

    let mut spec = GraphSpec {
        nodes: vec![constant_node("target", Value::Float(0.0)), slew],
    };

    let mut rt = GraphRuntime::default();
    evaluate_all(&mut rt, &spec).expect("initial evaluate");

    spec.nodes[0].params.value = Some(Value::Float(5.0));
    rt.dt = 0.25;
    rt.t += rt.dt;
    evaluate_all(&mut rt, &spec).expect("slew step");

    let first = match rt
        .outputs
        .get("slew")
        .and_then(|map| map.get("out"))
        .map(|pv| pv.value.clone())
        .expect("slew output")
    {
        Value::Float(f) => f,
        other => panic!("expected float, got {:?}", other),
    };

    assert!(
        (first - 0.5).abs() < 1e-6,
        "slew should move at configured rate"
    );

    for _ in 0..10 {
        rt.dt = 0.25;
        rt.t += rt.dt;
        evaluate_all(&mut rt, &spec).expect("subsequent step");
    }

    let final_val = match rt
        .outputs
        .get("slew")
        .and_then(|map| map.get("out"))
        .map(|pv| pv.value.clone())
        .expect("slew output")
    {
        Value::Float(f) => f,
        other => panic!("expected float, got {:?}", other),
    };
    assert!(
        (final_val - 5.0).abs() < 0.25,
        "slew should eventually reach target"
    );
}

// --- Feature-gated URDF IK tests ----------------------------------------

#[cfg(feature = "urdf_ik")]
mod urdf_ik {
    use super::*;

    const PLANAR_URDF: &str = r#"
<robot name="planar_arm">
  <link name="base_link" />
  <link name="link1" />
  <link name="link2" />
  <link name="tool" />

  <joint name="joint1" type="revolute">
<parent link="base_link" />
<child link="link1" />
<origin xyz="0 0 0" rpy="0 0 0" />
<axis xyz="0 0 1" />
<limit lower="-3.1416" upper="3.1416" effort="1" velocity="1" />
  </joint>

  <joint name="joint2" type="revolute">
<parent link="link1" />
<child link="link2" />
<origin xyz="0.5 0 0" rpy="0 0 0" />
<axis xyz="0 0 1" />
<limit lower="-3.1416" upper="3.1416" effort="1" velocity="1" />
  </joint>

  <joint name="joint3" type="revolute">
<parent link="link2" />
<child link="tool" />
<origin xyz="0.5 0 0" rpy="0 0 0" />
<axis xyz="0 0 1" />
<limit lower="-3.1416" upper="3.1416" effort="1" velocity="1" />
  </joint>
</robot>
"#;

    const POSE_URDF: &str = r#"
<robot name="pose_arm">
  <link name="base_link" />
  <link name="link1" />
  <link name="link2" />
  <link name="link3" />
  <link name="link4" />
  <link name="link5" />
  <link name="link6" />
  <link name="tool" />

  <joint name="joint1" type="revolute">
<parent link="base_link" />
<child link="link1" />
<origin xyz="0 0 0.1" rpy="0 0 0" />
<axis xyz="0 0 1" />
<limit lower="-3.1416" upper="3.1416" effort="1" velocity="1" />
  </joint>

  <joint name="joint2" type="revolute">
<parent link="link1" />
<child link="link2" />
<origin xyz="0.2 0 0" rpy="0 0 0" />
<axis xyz="0 1 0" />
<limit lower="-3.1416" upper="3.1416" effort="1" velocity="1" />
  </joint>

  <joint name="joint3" type="revolute">
<parent link="link2" />
<child link="link3" />
<origin xyz="0.2 0 0" rpy="0 0 0" />
<axis xyz="1 0 0" />
<limit lower="-3.1416" upper="3.1416" effort="1" velocity="1" />
  </joint>

  <joint name="joint4" type="revolute">
<parent link="link3" />
<child link="link4" />
<origin xyz="0.2 0 0" rpy="0 0 0" />
<axis xyz="0 0 1" />
<limit lower="-3.1416" upper="3.1416" effort="1" velocity="1" />
  </joint>

  <joint name="joint5" type="revolute">
<parent link="link4" />
<child link="link5" />
<origin xyz="0.15 0 0" rpy="0 0 0" />
<axis xyz="0 1 0" />
<limit lower="-3.1416" upper="3.1416" effort="1" velocity="1" />
  </joint>

  <joint name="joint6" type="revolute">
<parent link="link5" />
<child link="link6" />
<origin xyz="0.1 0 0" rpy="0 0 0" />
<axis xyz="1 0 0" />
<limit lower="-3.1416" upper="3.1416" effort="1" velocity="1" />
  </joint>

  <joint name="tool_joint" type="fixed">
<parent link="link6" />
<child link="tool" />
<origin xyz="0.1 0 0" rpy="0 0 0" />
  </joint>
</robot>
"#;

    fn params_for(urdf: &str) -> NodeParams {
        NodeParams {
            urdf_xml: Some(urdf.to_string()),
            root_link: Some("base_link".to_string()),
            tip_link: Some("tool".to_string()),
            max_iters: Some(200),
            tol_pos: Some(1e-3),
            tol_rot: Some(1e-3),
            ..Default::default()
        }
    }

    fn run_graph(nodes: Vec<NodeSpec>) -> Result<GraphRuntime, String> {
        let spec = GraphSpec { nodes };
        let mut rt = GraphRuntime::default();
        evaluate_all(&mut rt, &spec)?;
        Ok(rt)
    }

    fn extract_angles(record: &HashMap<String, Value>, names: &[&str]) -> Vec<f32> {
        names
            .iter()
            .map(|name| match record.get(*name) {
                Some(Value::Float(angle)) => *angle,
                other => panic!("expected float angle for {name}, got {:?}", other),
            })
            .collect()
    }

    #[test]
    fn urdf_ik_position_reaches_target() {
        let target_pos_id = "target_pos";
        let ik_id = "ik";

        let seed = vec![0.1, -0.2, 0.3, 0.2, -0.1, 0.25];
        let (chain, _) = super::urdfik::build_chain_from_urdf(POSE_URDF, "base_link", "tool")
            .expect("valid chain");
        chain.set_joint_positions(&seed).expect("apply seed for fk");
        let target_pose = chain.end_transform();
        let target_pos_vec = target_pose.translation.vector;

        let mut nodes = Vec::new();
        nodes.push(super::constant_node(
            target_pos_id,
            Value::Vec3([target_pos_vec.x, target_pos_vec.y, target_pos_vec.z]),
        ));

        let mut inputs = HashMap::new();
        inputs.insert(
            "target_pos".to_string(),
            InputConnection {
                node_id: target_pos_id.to_string(),
                output_key: "out".to_string(),
            },
        );

        let mut params = params_for(POSE_URDF);
        params.seed = Some(seed.clone());

        nodes.push(NodeSpec {
            id: ik_id.to_string(),
            kind: NodeType::UrdfIkPosition,
            params,
            inputs,
            output_shapes: HashMap::new(),
        });

        let rt = run_graph(nodes).expect("IK position solve should succeed");
        let record = match rt
            .outputs
            .get(ik_id)
            .and_then(|map| map.get("out"))
            .map(|pv| pv.value.clone())
            .expect("ik output")
        {
            Value::Record(map) => map,
            other => panic!("expected record output, got {:?}", other),
        };

        assert_eq!(record.len(), 6, "expected six joint outputs");
        let joint_names = ["joint1", "joint2", "joint3", "joint4", "joint5", "joint6"];
        let angles = extract_angles(&record, &joint_names);

        for (expected, actual) in seed.iter().zip(angles.iter()) {
            assert!((expected - actual).abs() < 1e-3);
        }

        let (chain_verify, _) =
            super::urdfik::build_chain_from_urdf(POSE_URDF, "base_link", "tool")
                .expect("valid chain");
        chain_verify
            .set_joint_positions(&angles)
            .expect("seed application");
        let end = chain_verify.end_transform();
        let pos = end.translation.vector;
        assert!((pos.x - target_pos_vec.x).abs() < 5e-3);
        assert!((pos.y - target_pos_vec.y).abs() < 5e-3);
        assert!((pos.z - target_pos_vec.z).abs() < 5e-3);
    }

    #[test]
    fn urdf_ik_pose_matches_target_orientation() {
        let target_pos_id = "target_pos";
        let target_rot_id = "target_rot";
        let ik_id = "ik_pose";

        let seed = vec![0.3, -0.45, 0.35, 0.15, -0.2, 0.18];
        let (chain, _) = super::urdfik::build_chain_from_urdf(POSE_URDF, "base_link", "tool")
            .expect("valid chain");
        chain.set_joint_positions(&seed).expect("seed fk");
        let base_pose = chain.end_transform();
        let base_pos = base_pose.translation.vector;

        let desired_rot = base_pose.rotation;

        let mut nodes = Vec::new();
        nodes.push(super::constant_node(
            target_pos_id,
            Value::Vec3([base_pos.x, base_pos.y, base_pos.z]),
        ));
        nodes.push(super::constant_node(
            target_rot_id,
            Value::Quat([desired_rot.i, desired_rot.j, desired_rot.k, desired_rot.w]),
        ));

        let mut inputs = HashMap::new();
        inputs.insert(
            "target_pos".to_string(),
            InputConnection {
                node_id: target_pos_id.to_string(),
                output_key: "out".to_string(),
            },
        );
        inputs.insert(
            "target_rot".to_string(),
            InputConnection {
                node_id: target_rot_id.to_string(),
                output_key: "out".to_string(),
            },
        );

        let mut params = params_for(POSE_URDF);
        params.max_iters = Some(600);
        params.seed = Some(seed.clone());

        nodes.push(NodeSpec {
            id: ik_id.to_string(),
            kind: NodeType::UrdfIkPose,
            params,
            inputs,
            output_shapes: HashMap::new(),
        });

        let rt = run_graph(nodes).expect("IK pose solve should succeed");
        let record = match rt
            .outputs
            .get(ik_id)
            .and_then(|map| map.get("out"))
            .map(|pv| pv.value.clone())
            .expect("ik output")
        {
            Value::Record(map) => map,
            other => panic!("expected record output, got {:?}", other),
        };

        assert_eq!(record.len(), 6, "expected six joint outputs");
        let joint_names = ["joint1", "joint2", "joint3", "joint4", "joint5", "joint6"];
        let angles = extract_angles(&record, &joint_names);

        for (expected, actual) in seed.iter().zip(angles.iter()) {
            assert!((expected - actual).abs() < 1e-3);
        }

        let (chain_verify, _) =
            super::urdfik::build_chain_from_urdf(POSE_URDF, "base_link", "tool")
                .expect("valid chain");
        chain_verify
            .set_joint_positions(&angles)
            .expect("apply joints");
        let end = chain_verify.end_transform();
        let pos = end.translation.vector;
        assert!((pos.x - base_pos.x).abs() < 5e-3);
        assert!((pos.y - base_pos.y).abs() < 5e-3);
        assert!((pos.z - base_pos.z).abs() < 5e-3);

        let angle_err = desired_rot.angle_to(&end.rotation);
        assert!(angle_err.abs() < 5e-3);
    }

    #[test]
    fn malformed_urdf_returns_error() {
        let mut params = params_for(PLANAR_URDF);
        params.urdf_xml = Some("<robot".to_string());

        let mut nodes = Vec::new();
        nodes.push(super::constant_node(
            "target_pos",
            Value::Vec3([0.5, 0.0, 0.0]),
        ));

        let mut inputs = HashMap::new();
        inputs.insert(
            "target_pos".to_string(),
            InputConnection {
                node_id: "target_pos".to_string(),
                output_key: "out".to_string(),
            },
        );

        nodes.push(NodeSpec {
            id: "ik".to_string(),
            kind: NodeType::UrdfIkPosition,
            params,
            inputs,
            output_shapes: HashMap::new(),
        });

        let spec = GraphSpec { nodes };
        let mut rt = GraphRuntime::default();
        let err = evaluate_all(&mut rt, &spec).expect_err("malformed URDF should error");
        assert!(err.contains("parse URDF"));
    }

    #[test]
    fn seed_length_mismatch_errors() {
        let mut params = params_for(PLANAR_URDF);
        params.seed = Some(vec![0.0]);

        let mut nodes = Vec::new();
        nodes.push(super::constant_node(
            "target_pos",
            Value::Vec3([0.5, 0.0, 0.0]),
        ));

        let mut inputs = HashMap::new();
        inputs.insert(
            "target_pos".to_string(),
            InputConnection {
                node_id: "target_pos".to_string(),
                output_key: "out".to_string(),
            },
        );

        nodes.push(NodeSpec {
            id: "ik".to_string(),
            kind: NodeType::UrdfIkPosition,
            params,
            inputs,
            output_shapes: HashMap::new(),
        });

        let spec = GraphSpec { nodes };
        let mut rt = GraphRuntime::default();
        let err = evaluate_all(&mut rt, &spec).expect_err("seed mismatch should error");
        assert!(err.contains("seed length"));
    }

    #[test]
    fn weights_length_mismatch_errors() {
        let mut params = params_for(PLANAR_URDF);
        params.weights = Some(vec![1.0]);

        let mut nodes = Vec::new();
        nodes.push(super::constant_node(
            "target_pos",
            Value::Vec3([0.5, 0.0, 0.0]),
        ));

        let mut inputs = HashMap::new();
        inputs.insert(
            "target_pos".to_string(),
            InputConnection {
                node_id: "target_pos".to_string(),
                output_key: "out".to_string(),
            },
        );

        nodes.push(NodeSpec {
            id: "ik".to_string(),
            kind: NodeType::UrdfIkPosition,
            params,
            inputs,
            output_shapes: HashMap::new(),
        });

        let spec = GraphSpec { nodes };
        let mut rt = GraphRuntime::default();
        let err = evaluate_all(&mut rt, &spec).expect_err("weights mismatch should error");
        assert!(err.contains("weights length"));
    }

    #[test]
    fn urdf_fk_returns_correct_pose() {
        let joints_id = "joints";
        let fk_id = "fk";

        let joint_names = ["joint1", "joint2", "joint3", "joint4", "joint5", "joint6"];
        let joint_angles = vec![0.1, -0.2, 0.3, -0.15, 0.2, -0.1];

        let mut record = HashMap::new();
        for (name, angle) in joint_names.iter().zip(joint_angles.iter()) {
            record.insert((*name).to_string(), Value::Float(*angle));
        }

        let mut inputs = HashMap::new();
        inputs.insert(
            "joints".to_string(),
            InputConnection {
                node_id: joints_id.to_string(),
                output_key: "out".to_string(),
            },
        );

        let mut params = params_for(POSE_URDF);
        params.max_iters = None;
        params.tol_pos = None;
        params.tol_rot = None;

        let nodes = vec![
            super::constant_node(joints_id, Value::Record(record)),
            NodeSpec {
                id: fk_id.to_string(),
                kind: NodeType::UrdfFk,
                params,
                inputs,
                output_shapes: HashMap::new(),
            },
        ];

        let rt = run_graph(nodes).expect("FK evaluation should succeed");

        let (expected_chain, _) =
            super::urdfik::build_chain_from_urdf(POSE_URDF, "base_link", "tool")
                .expect("valid chain");
        expected_chain
            .set_joint_positions(&joint_angles)
            .expect("apply joints");
        let expected_pose = expected_chain.end_transform();
        let expected_pos = expected_pose.translation.vector;
        let expected_rot = expected_pose.rotation;

        let outputs = rt.outputs.get(fk_id).expect("fk outputs present");
        let position = match outputs
            .get("position")
            .map(|pv| pv.value.clone())
            .expect("position output")
        {
            Value::Vec3(arr) => arr,
            other => panic!("expected Vec3, got {:?}", other),
        };
        for (observed, expected) in
            position
                .iter()
                .zip([expected_pos.x, expected_pos.y, expected_pos.z])
        {
            assert!((observed - expected).abs() < 1e-4);
        }

        let rotation = match outputs
            .get("rotation")
            .map(|pv| pv.value.clone())
            .expect("rotation output")
        {
            Value::Quat(arr) => arr,
            other => panic!("expected Quat, got {:?}", other),
        };
        let actual_rot = k::UnitQuaternion::new_normalize(k::nalgebra::Quaternion::new(
            rotation[3],
            rotation[0],
            rotation[1],
            rotation[2],
        ));
        let angle_err = expected_rot.angle_to(&actual_rot);
        assert!(angle_err.abs() < 1e-4, "quaternion mismatch: {angle_err}");

        let (transform_pos, transform_rot) = match outputs
            .get("transform")
            .map(|pv| pv.value.clone())
            .expect("transform output")
        {
            Value::Transform { pos, rot, scale } => {
                assert_eq!(scale, [1.0, 1.0, 1.0]);
                (pos, rot)
            }
            other => panic!("expected Transform, got {:?}", other),
        };
        for (observed, expected) in
            transform_pos
                .iter()
                .zip([expected_pos.x, expected_pos.y, expected_pos.z])
        {
            assert!((observed - expected).abs() < 1e-4);
        }
        let transform_rot = k::UnitQuaternion::new_normalize(k::nalgebra::Quaternion::new(
            transform_rot[3],
            transform_rot[0],
            transform_rot[1],
            transform_rot[2],
        ));
        let transform_angle = expected_rot.angle_to(&transform_rot);
        assert!(
            transform_angle.abs() < 1e-4,
            "transform rotation mismatch: {transform_angle}"
        );
    }

    #[test]
    fn urdf_fk_handles_missing_joint_with_default() {
        let joints_id = "joints";
        let fk_id = "fk_defaults";

        let provided_angles = [0.25f32, -0.35f32];
        let default_angle = 0.5f32;

        let mut record = HashMap::new();
        record.insert("joint1".to_string(), Value::Float(provided_angles[0]));
        record.insert("joint2".to_string(), Value::Float(provided_angles[1]));

        let mut inputs = HashMap::new();
        inputs.insert(
            "joints".to_string(),
            InputConnection {
                node_id: joints_id.to_string(),
                output_key: "out".to_string(),
            },
        );

        let mut params = params_for(PLANAR_URDF);
        params.max_iters = None;
        params.tol_pos = None;
        params.tol_rot = None;
        params.joint_defaults = Some(vec![("joint3".to_string(), default_angle)]);

        let nodes = vec![
            super::constant_node(joints_id, Value::Record(record)),
            NodeSpec {
                id: fk_id.to_string(),
                kind: NodeType::UrdfFk,
                params,
                inputs,
                output_shapes: HashMap::new(),
            },
        ];

        let rt = run_graph(nodes).expect("FK evaluation with defaults should succeed");

        let full_angles = vec![provided_angles[0], provided_angles[1], default_angle];
        let (expected_chain, _) =
            super::urdfik::build_chain_from_urdf(PLANAR_URDF, "base_link", "tool")
                .expect("valid chain");
        expected_chain
            .set_joint_positions(&full_angles)
            .expect("apply joints");
        let expected_pose = expected_chain.end_transform();
        let expected_pos = expected_pose.translation.vector;

        let outputs = rt.outputs.get(fk_id).expect("fk outputs present");
        let position = match outputs
            .get("position")
            .map(|pv| pv.value.clone())
            .expect("position output")
        {
            Value::Vec3(arr) => arr,
            other => panic!("expected Vec3, got {:?}", other),
        };
        for (observed, expected) in
            position
                .iter()
                .zip([expected_pos.x, expected_pos.y, expected_pos.z])
        {
            assert!((observed - expected).abs() < 1e-4);
        }
    }

    #[test]
    fn urdf_fk_errors_on_bad_input_type() {
        let joints_id = "bad_joints";
        let fk_id = "fk_error";

        let mut record = HashMap::new();
        record.insert("joint1".to_string(), Value::Text("invalid".to_string()));
        record.insert("joint2".to_string(), Value::Float(0.0));
        record.insert("joint3".to_string(), Value::Float(0.0));

        let mut inputs = HashMap::new();
        inputs.insert(
            "joints".to_string(),
            InputConnection {
                node_id: joints_id.to_string(),
                output_key: "out".to_string(),
            },
        );

        let mut params = params_for(PLANAR_URDF);
        params.joint_defaults = None;

        let nodes = vec![
            super::constant_node(joints_id, Value::Record(record)),
            NodeSpec {
                id: fk_id.to_string(),
                kind: NodeType::UrdfFk,
                params,
                inputs,
                output_shapes: HashMap::new(),
            },
        ];

        let err = run_graph(nodes).expect_err("FK should fail on invalid joint input");
        assert!(err.contains("numeric scalar"), "unexpected error: {err}");
    }
}
