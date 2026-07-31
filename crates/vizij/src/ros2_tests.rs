//! Live ROS 2 end-to-end: a typed `interaction_skills/LookAt` client drives
//! the gaze skill on the real vizij device over DDS.
//!
//! The whole production chain is under test — discovery (`DescribeMethods`
//! over the device's gaze module), the ros4hri exposure profile's
//! `/skill/look_at` action binding, SPAWN into the node-graph interpreter
//! (the shipped look_at fragment grafts as graph structure, writes the
//! `standard/ros4hri/gaze/*` surface, and reports on its status key),
//! cancel → halt → `CANCELED` with the `std_skills` errno, and an
//! unsupported policy answering `ROS_ENOTSUP` straight from the fragment.
//! The only test double is a `speak` module probing the method-service
//! plane — the discriminator between "raw services are broken live" and
//! "the skill assembly is at fault".

use std::time::Duration;

use arora_types::call::CallResult;
use arora_types::data::{DataStore, Key};
use arora_types::gen_uuid_from_str;
use arora_types::record::module::frozen::{Function, Parameter};
use arora_types::record::ty::{FrozenTy, PrimitiveKind};
use arora_types::value::Value;
use rand::Rng;
use ros2_client::action_msgs::GoalStatusEnum;
use ros2_client::{Context, ContextOptions, Message, Name, NodeName, NodeOptions, ServiceMapping};
use serde::{Deserialize, Serialize};
use vizij_arora_hal::RigHal;
use vizij_arora_store::BlackboardStore;

use crate::device::builder_for;

// The typed client's view of `interaction_skills/LookAt` — local mirrors of
// the standard messages (`ros2_client::Message` is a foreign marker trait).
#[derive(Serialize, Deserialize, Clone)]
struct Meta {
    caller: String,
    priority: u8,
}
#[derive(Serialize, Deserialize, Clone)]
struct Header {
    stamp: ros2_client::builtin_interfaces::Time,
    frame_id: String,
}
#[derive(Serialize, Deserialize, Clone)]
struct Point {
    x: f64,
    y: f64,
    z: f64,
}
#[derive(Serialize, Deserialize, Clone)]
struct PointStamped {
    header: Header,
    point: Point,
}
#[derive(Serialize, Deserialize, Clone)]
struct LookAtGoal {
    meta: Meta,
    policy: String,
    target: PointStamped,
}
impl Message for LookAtGoal {}
#[derive(Serialize, Deserialize, Debug, Clone)]
struct SkillResult {
    error_code: u8,
    error_msg: String,
}
#[derive(Serialize, Deserialize, Debug, Clone)]
struct LookAtResult {
    result: SkillResult,
}
impl Message for LookAtResult {}
#[derive(Serialize, Deserialize, Clone)]
struct SkillFeedback {
    data_bool: bool,
    data_int: u16,
    data_float: f32,
    data_str: String,
}
#[derive(Serialize, Deserialize, Clone)]
struct LookAtFeedback {
    feedback: SkillFeedback,
}
impl Message for LookAtFeedback {}

const ROS_ECANCELED: u8 = 125;
const ROS_ENOTSUP: u8 = 134;

/// A plain described method riding the service plane — the test's only
/// double, probing that raw DDS services work before blaming the skill
/// assembly.
fn speak_module() -> arora::HostModule {
    let signature = {
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
    arora::ModuleBuilder::new(gen_uuid_from_str("speak-module"))
        .described_function(gen_uuid_from_str("speak"), "speak", signature, |_call| {
            Ok(CallResult {
                ret: Value::F64(1.0),
                mutated: Vec::new(),
            })
        })
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
async fn the_look_at_skill_serves_the_standard_contract_on_the_vizij_device() {
    let _ = env_logger::builder()
        .parse_filters("warn")
        .is_test(true)
        .try_init();
    let domain_id: u16 = rand::rng().random_range(1..=200);

    // The real device: node-graph interpreter with the shipped look_at
    // fragment, the described gaze module, and the ROS 2 bridge exposing the
    // ros4hri profile. The store clone watches the gaze surface from the
    // test.
    let store = BlackboardStore::new();
    let bridge = arora_bridge_ros2::Ros2Bridge::new(
        arora_bridge_ros2::Ros2BridgeConfig::new("robot", domain_id)
            .with_profile(arora_bridge_ros2::ExposureProfile::ros4hri()),
    )
    .await;
    let mut arora = builder_for(
        r#"{ "nodes": [], "edges": [] }"#,
        RigHal::new(),
        store.clone(),
        &[],
    )
    .expect("build the device")
    .with_host_module(speak_module())
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
        let (_ctx, mut node) = create_test_node(domain_id, "skill_client");
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

        // The typed skill client on the profile's absolute action name.
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
                &Name::parse("/skill/look_at").expect("a valid action name"),
                &ros2_client::ActionTypeName::new("interaction_skills", "LookAt"),
                qos,
            )
            .expect("the skill client creates");

        let goal = |policy: &str| LookAtGoal {
            meta: Meta {
                caller: "test".to_string(),
                priority: 128,
            },
            policy: policy.to_string(),
            target: PointStamped {
                header: Header {
                    stamp: ros2_client::builtin_interfaces::Time::ZERO,
                    frame_id: "sellion_link".to_string(),
                },
                point: Point {
                    x: 0.5,
                    y: -0.25,
                    z: 1.0,
                },
            },
        };

        // A tracking goal: accepted, SPAWNed, and the shipped fragment
        // writes the goal onto the standard gaze surface.
        eprintln!("[client] sending the tracking goal");
        let goal_id = loop {
            match tokio::time::timeout(Duration::from_secs(2), client.async_send_goal(goal("")))
                .await
            {
                Ok(Ok((goal_id, response))) if response.accepted => break goal_id,
                other => {
                    eprintln!("[client] send_goal attempt: {other:?}");
                    tokio::time::sleep(Duration::from_millis(200)).await;
                }
            }
        };
        eprintln!("[client] goal accepted — the fragment is live in the graph");
        let deadline = tokio::time::Instant::now() + Duration::from_secs(10);
        loop {
            let gaze = store
                .read(&[Key::from("standard/ros4hri/gaze/target")])
                .into_iter()
                .next()
                .flatten();
            if gaze == Some(Value::ArrayF32(vec![0.5, -0.25, 1.0])) {
                let frame = store
                    .read(&[Key::from("standard/ros4hri/gaze/frame")])
                    .into_iter()
                    .next()
                    .flatten();
                assert_eq!(frame, Some(Value::String("sellion_link".to_string())));
                break;
            }
            assert!(
                tokio::time::Instant::now() < deadline,
                "the gaze surface never took the goal target (got {gaze:?})"
            );
            tokio::time::sleep(Duration::from_millis(50)).await;
        }
        eprintln!("[client] the gaze surface tracks the goal");

        // Cancel: the bridge halts the run and answers CANCELED with the
        // std_skills errno in the standard Result message.
        client
            .async_cancel_goal(goal_id, ros2_client::builtin_interfaces::Time::ZERO)
            .await
            .expect("cancel round-trips");
        let (status, result) = client
            .async_request_result(goal_id)
            .await
            .expect("the result arrives");
        assert_eq!(status, GoalStatusEnum::Canceled);
        assert_eq!(result.result.error_code, ROS_ECANCELED);
        eprintln!("[client] canceled with ECANCELED");

        // An unimplemented policy: the fragment itself answers ROS_ENOTSUP
        // on its result key, and the goal aborts.
        let (goal_id, response) = client
            .async_send_goal(goal("social"))
            .await
            .expect("the social goal sends");
        assert!(response.accepted);
        let (status, result) = client
            .async_request_result(goal_id)
            .await
            .expect("the social result arrives");
        assert_eq!(status, GoalStatusEnum::Aborted);
        assert_eq!(result.result.error_code, ROS_ENOTSUP);
        eprintln!("[client] social answered ENOTSUP");
    };

    tokio::select! {
        _ = &mut device => unreachable!("the device loop never returns"),
        result = tokio::time::timeout(Duration::from_secs(60), client_flow) => {
            result.expect("the skill lifecycle timed out");
        }
    }
}
