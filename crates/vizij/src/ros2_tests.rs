//! Live ROS 2 end-to-end: a typed action client drives a LookAt task run on
//! the real vizij device over DDS.
//!
//! The whole production chain is under test — discovery (`DescribeMethods`
//! over the described gaze host module), the bridge's action synthesis, SPAWN
//! into the node-graph interpreter (the run grafts as graph structure and
//! reports on its status key), cancel → halt → `CANCELED`. The only scripted
//! piece is the gaze skill itself, which tracks indefinitely (`Running` every
//! invocation) like the real LookAt.

use std::time::Duration;

use arora_behavior_tree_types::STATUS_ENUMERATION_ID;
use arora_types::call::CallResult;
use arora_types::gen_uuid_from_str;
use arora_types::record::module::frozen::{Function, Parameter};
use arora_types::record::ty::{FrozenScalar, FrozenTy, PrimitiveKind};
use arora_types::record::{FrozenReference, Version};
use arora_types::value::Value;
use rand::Rng;
use ros2_client::action_msgs::GoalStatusEnum;
use ros2_client::{Context, ContextOptions, Message, Name, NodeName, NodeOptions, ServiceMapping};
use serde::{Deserialize, Serialize};
use vizij_arora_behavior::task;
use vizij_arora_hal::RigHal;
use vizij_arora_store::BlackboardStore;

use crate::device::builder_for;

// The typed client's view of the action.
#[derive(Serialize, Deserialize, Clone)]
struct LookAtGoal {
    policy: String,
    x: f64,
}
impl Message for LookAtGoal {}
/// The run populates no result keys (v1 hosts one call, §2.2.2 of the
/// task-runs proposal), so the synthesised Result carries the goal status
/// alone — an empty payload on the typed side.
#[derive(Serialize, Deserialize, Clone)]
struct LookAtResult {}
impl Message for LookAtResult {}
#[derive(Serialize, Deserialize)]
struct LookAtFeedback {
    gaze_error: f32,
}
impl Message for LookAtFeedback {}

fn gaze_module() -> arora::HostModule {
    let look_at_signature = {
        let mut parameters = std::collections::HashMap::new();
        let mut parameter_ordering = Vec::new();
        for (name, kind) in [("policy", PrimitiveKind::String), ("x", PrimitiveKind::F64)] {
            let id = gen_uuid_from_str(name);
            parameter_ordering.push(id);
            parameters.insert(
                id,
                Parameter {
                    name: name.to_string(),
                    ty: FrozenTy::from(kind),
                    mutable: false,
                },
            );
        }
        Function {
            parameters,
            parameter_ordering,
            // The behavior `Status` return is the action marker: the bridge
            // exposes this method as a ROS 2 action, not a service.
            return_ty: FrozenTy::FrozenScalar(FrozenScalar {
                reference: FrozenReference {
                    id: STATUS_ENUMERATION_ID,
                    version: Version::parse("1.0.0").expect("a valid version"),
                },
            }),
        }
    };
    let speak_signature = {
        let text = gen_uuid_from_str("text");
        let mut parameters = std::collections::HashMap::new();
        parameters.insert(
            text,
            Parameter {
                name: "text".to_string(),
                ty: FrozenTy::from(PrimitiveKind::String),
                mutable: false,
            },
        );
        Function {
            parameters,
            parameter_ordering: vec![text],
            return_ty: FrozenTy::from(PrimitiveKind::F64),
        }
    };
    arora::ModuleBuilder::new(gen_uuid_from_str("gaze-module"))
        // The gaze skill: every invocation steers toward its target and
        // reports `Running` — indefinite tracking, like the real LookAt.
        .described_function(
            gen_uuid_from_str("look_at"),
            "look_at",
            look_at_signature,
            |_call| {
                Ok(CallResult {
                    ret: task::running(),
                    mutated: Vec::new(),
                })
            },
        )
        // A plain method: it rides the service plane in the same run — the
        // discriminator between "raw services are broken live" and "the
        // action assembly is at fault".
        .described_function(
            gen_uuid_from_str("speak"),
            "speak",
            speak_signature,
            |_call| {
                Ok(CallResult {
                    ret: Value::F64(1.0),
                    mutated: Vec::new(),
                })
            },
        )
        .build()
}

fn create_test_node(domain_id: u16, name_suffix: &str) -> (Context, ros2_client::Node) {
    let ctx = Context::with_options(ContextOptions::new().domain_id(domain_id))
        .expect("create the test context");
    let node_name = NodeName::new("/", &format!("test_{name_suffix}")).expect("a valid node name");
    let node = ctx
        .new_node(node_name, NodeOptions::new())
        .expect("create the test node");
    (ctx, node)
}

#[tokio::test(flavor = "current_thread")]
#[cfg_attr(
    target_os = "macos",
    ignore = "DDS multicast SPDP discovery is unreliable on macOS loopback (rustdds has no \
              unicast-peer/interface config); this runs on Linux CI. To run locally, ensure an \
              active multicast-capable interface and use --ignored."
)]
async fn a_look_at_action_runs_on_the_vizij_device_over_dds() {
    let _ = env_logger::builder()
        .parse_filters("warn")
        .is_test(true)
        .try_init();
    let domain_id: u16 = rand::rng().random_range(1..=200);

    // The real device: node-graph interpreter, gaze host module, ROS 2 bridge.
    let bridge = arora_bridge_ros2::Ros2Bridge::new(arora_bridge_ros2::Ros2BridgeConfig::new(
        "robot", domain_id,
    ))
    .await;
    let mut arora = builder_for(
        r#"{ "nodes": [], "edges": [] }"#,
        RigHal::new(),
        BlackboardStore::new(),
    )
    .expect("build the device")
    .with_host_module(gaze_module())
    .with_bridge(Box::new(bridge))
    .build()
    .expect("build arora");

    // The device loop: real steps, the cadence a running vizij holds.
    let device = async {
        loop {
            arora.step(Duration::from_millis(16)).expect("step");
            tokio::time::sleep(Duration::from_millis(20)).await;
        }
    };
    tokio::pin!(device);

    let client_flow = async {
        let (_ctx, mut node) = create_test_node(domain_id, "action_client");
        // The reliable service profile ros2-client's own action examples use —
        // the best-effort default drops service requests.
        let service_qos = {
            use ros2_client::ros2::{policy, QosPolicyBuilder};
            QosPolicyBuilder::new()
                .reliability(policy::Reliability::Reliable {
                    max_blocking_time: ros2_client::ros2::Duration::from_millis(100),
                })
                .history(policy::History::KeepLast { depth: 4 })
                .durability(policy::Durability::TransientLocal)
                .build()
        };

        // Probe the method-service plane first: speak() on /robot/methods.
        #[derive(Serialize, Deserialize, Clone)]
        struct SpeakRequest {
            text: String,
        }
        impl Message for SpeakRequest {}
        #[derive(Serialize, Deserialize, Debug)]
        struct SpeakResponse {
            result: f64,
        }
        impl Message for SpeakResponse {}
        let speak_client = node
            .create_client::<ros2_client::AService<SpeakRequest, SpeakResponse>>(
                ServiceMapping::Enhanced,
                &Name::parse("/robot/methods/speak").expect("a valid service name"),
                &ros2_client::ServiceTypeName::new("arora", "speak"),
                service_qos.clone(),
                service_qos.clone(),
            )
            .expect("the speak client creates");
        eprintln!("[client] probing the method service");
        loop {
            let sent = speak_client.async_send_request(SpeakRequest {
                text: "hello".to_string(),
            });
            match tokio::time::timeout(Duration::from_secs(2), sent).await {
                Ok(Ok(req_id)) => {
                    match tokio::time::timeout(
                        Duration::from_secs(2),
                        speak_client.async_receive_response(req_id),
                    )
                    .await
                    {
                        Ok(Ok(response)) => {
                            assert!((response.result - 1.0).abs() < f64::EPSILON);
                            eprintln!("[client] method service OK");
                            break;
                        }
                        other => eprintln!("[client] speak response attempt: {other:?}"),
                    }
                }
                other => eprintln!("[client] speak send attempt: {other:?}"),
            }
            tokio::time::sleep(Duration::from_millis(200)).await;
        }

        // The typed action client on /robot/actions/look_at.
        let qos = ros2_client::action::ActionClientQosPolicies {
            goal_service: service_qos.clone(),
            result_service: service_qos.clone(),
            cancel_service: service_qos.clone(),
            feedback_subscription: service_qos.clone(),
            status_subscription: service_qos.clone(),
        };
        let client = node
            .create_action_client::<ros2_client::Action<LookAtGoal, LookAtResult, LookAtFeedback>>(
                ServiceMapping::Enhanced,
                &Name::parse("/robot/actions/look_at").expect("a valid action name"),
                &ros2_client::ActionTypeName::new("arora", "look_at"),
                qos,
            )
            .expect("the action client creates");

        // Send the goal until the graph connects and the server accepts. The
        // accepted goal SPAWNs a run into the device's node graph.
        let goal = LookAtGoal {
            policy: "track".to_string(),
            x: 1.5,
        };
        eprintln!("[client] sending goal");
        let goal_id = loop {
            match tokio::time::timeout(Duration::from_secs(2), client.async_send_goal(goal.clone()))
                .await
            {
                Ok(Ok((goal_id, response))) if response.accepted => break goal_id,
                other => {
                    eprintln!("[client] send_goal attempt: {other:?}");
                    tokio::time::sleep(Duration::from_millis(200)).await;
                }
            }
        };
        eprintln!("[client] goal accepted — the run is live in the graph");

        // The run tracks indefinitely; cancel ends it. The bridge halts the
        // run (its status key goes terminal) and reports CANCELED — it is the
        // party that issued the halt.
        tokio::time::sleep(Duration::from_millis(300)).await;
        eprintln!("[client] canceling");
        client
            .async_cancel_goal(goal_id, ros2_client::builtin_interfaces::Time::ZERO)
            .await
            .expect("cancel round-trips");
        eprintln!("[client] cancel answered; requesting result");
        let (status, LookAtResult {}) = client
            .async_request_result(goal_id)
            .await
            .expect("the result arrives");
        assert_eq!(status, GoalStatusEnum::Canceled);
    };

    tokio::select! {
        _ = &mut device => unreachable!("the device loop never returns"),
        result = tokio::time::timeout(Duration::from_secs(60), client_flow) => {
            result.expect("the action lifecycle timed out");
        }
    }
}
