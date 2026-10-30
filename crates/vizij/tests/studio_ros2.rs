//! A Studio-exported face's keys on the ROS 2 bridge: the device serving
//! Studio's keys (`studio`) subscribes each of its inputs on
//! `/{namespace}/keys/<path>`, the path escaped into ROS 2 name tokens, and
//! publishes each feedback key the same way. Every form of a target — a
//! number's `<id>/target.position` and `<id>.target_position`, a component's
//! `<id>/target.position.<axis>` and `<id>.<axis>.target_position` — escapes
//! to a topic of its own, none shared with another key of the face, so each
//! is a way in and each feedback is published. An id starting with a digit
//! gains a leading `_`, as no ROS 2 name token starts with one; ROS 2 tools
//! then count the topic hidden (`ros2 topic list --include-hidden-topics`).
//!
//! The tests build under `ros2-dds`: their ROS 2 peer, the `ros2-client` dev
//! dependency, is the DDS backend, which excludes the Zenoh one.

#![cfg(all(feature = "studio", feature = "ros2-dds"))]

mod common;

use std::collections::HashMap;
use std::time::Duration;

use arora_bridge_ros2::conversions::topic_name;
use futures::{SinkExt, StreamExt};
use serde_json::{json, Value as Json};
use tokio_tungstenite::tungstenite::Message;
use uuid::Uuid;
use vizij::face::{FaceConfig, ProgramSelect};
use vizij::native::{start, BridgeConfig, Mode, Runtime};

use common::studio_face_glb;

// A Studio-exported face's animatables: `X`, a number whose id starts with a
// digit, and `COLOR`, a compound whose id starts with a letter.
const X: &str = "0f8fad5b-d9cb-469f-a165-70867728950e";
const Y: &str = "7c9e6679-7425-40de-944b-e07fc1f90ae7";
const YAW: &str = "e4eaaaf2-d142-41e1-b3e4-080027620cdd";
const COLOR: &str = "c56a4180-65aa-42ec-a945-5fd21dec0538";

/// A port nobody listens on right now (bound and released; the bridge binds
/// it next).
fn free_port() -> u16 {
    std::net::TcpListener::bind("127.0.0.1:0")
        .expect("bind an ephemeral port")
        .local_addr()
        .unwrap()
        .port()
}

/// A device serving the face's Studio keys, its ROS 2 bridge on
/// `ros2` (namespace, domain) when given, and the port of its local bridge.
fn serve(ros2: Option<(String, u16)>) -> (Runtime, u16) {
    let id = |id: &str| Uuid::parse_str(id).expect("a UUID");
    let glb = studio_face_glb(id(X), id(Y), id(YAW), id(COLOR), None);
    let port = free_port();
    let config = FaceConfig {
        wanted: ["rig", "pose-driver", "pose", "standard-adaptation"]
            .map(String::from)
            .to_vec(),
        program: ProgramSelect::None,
        stage_neutral: true,
        ros4hri: false,
        studio: true,
        speech: None,
        frame_rate: None,
    };
    let bridges = BridgeConfig {
        local: arora::bridge_ws::ServerConfig::with_port(port),
        ros2,
        ..BridgeConfig::default()
    };
    (
        start(&glb, config, bridges, Mode::Operator).expect("start"),
        port,
    )
}

/// The device's keys as its local bridge lists them — what the ROS 2 bridge
/// asks of the device too — once `ready` holds of them, within 10 s.
async fn list_keys(port: u16, ready: impl Fn(&[Json]) -> bool) -> Vec<Json> {
    let url = format!("ws://127.0.0.1:{port}");
    let mut socket = None;
    for _ in 0..100 {
        if let Ok((connected, _)) = tokio_tungstenite::connect_async(&url).await {
            socket = Some(connected);
            break;
        }
        tokio::time::sleep(Duration::from_millis(100)).await;
    }
    let mut socket = socket.expect("the local bridge listens within 10 s");
    let mut keys = Vec::new();
    for _ in 0..100 {
        socket
            .send(Message::text(json!({"type": "list_keys"}).to_string()))
            .await
            .expect("send");
        keys = loop {
            let next = tokio::time::timeout(Duration::from_secs(10), socket.next())
                .await
                .expect("an answer within 10 s")
                .expect("the socket stays open")
                .expect("a frame");
            let Message::Text(text) = next else { continue };
            let answer: Json = serde_json::from_str(&text).expect("JSON");
            if answer["type"] == "list_keys_resp" {
                break answer["keys"].as_array().expect("keys").clone();
            }
        };
        if ready(&keys) {
            break;
        }
        tokio::time::sleep(Duration::from_millis(100)).await;
    }
    keys
}

/// Each form of a target, the topic it escapes to under namespace `robot`.
fn target_topics() -> [(String, &'static str); 4] {
    [
        (
            format!("{X}/target.position"),
            "/robot/keys/_0f8fad5b_d9cb_469f_a165_70867728950e/target_position",
        ),
        (
            format!("{X}.target_position"),
            "/robot/keys/_0f8fad5b_d9cb_469f_a165_70867728950e_target_position",
        ),
        (
            format!("{COLOR}/target.position.r"),
            "/robot/keys/c56a4180_65aa_42ec_a945_5fd21dec0538/target_position_r",
        ),
        (
            format!("{COLOR}.r.target_position"),
            "/robot/keys/c56a4180_65aa_42ec_a945_5fd21dec0538_r_target_position",
        ),
    ]
}

/// Each feedback key, the topic it escapes to under namespace `robot`.
fn feedback_topics() -> [(String, &'static str); 3] {
    [
        (
            format!("{X}.position"),
            "/robot/keys/_0f8fad5b_d9cb_469f_a165_70867728950e_position",
        ),
        (
            format!("{COLOR}.position.r"),
            "/robot/keys/c56a4180_65aa_42ec_a945_5fd21dec0538_position_r",
        ),
        (
            format!("{COLOR}.r.position"),
            "/robot/keys/c56a4180_65aa_42ec_a945_5fd21dec0538_r_position",
        ),
    ]
}

/// Every form of a target is an input of its own topic, and every feedback
/// key is published on a topic no other key of the face takes.
#[tokio::test]
async fn each_form_of_a_studio_target_escapes_to_a_topic_of_its_own() {
    let (_device, port) = serve(None);
    let feedbacks = feedback_topics().map(|(key, _)| key);
    let keys = list_keys(port, |keys| {
        feedbacks
            .iter()
            .all(|key| keys.iter().any(|listed| listed["path"] == key.as_str()))
    })
    .await;
    let inputs: Vec<&str> = keys
        .iter()
        .filter(|key| key["__meta"]["editable"] == true)
        .filter_map(|key| key["path"].as_str())
        .collect();

    for (key, topic) in target_topics() {
        assert!(inputs.contains(&key.as_str()), "{key} is an input");
        assert_eq!(topic_name("robot", &key), topic, "{key}");
    }
    for (key, topic) in feedback_topics() {
        assert!(!inputs.contains(&key.as_str()), "{key} is no input");
        assert_eq!(topic_name("robot", &key), topic, "{key}");
    }

    // No two inputs share a topic: the bridge subscribes each.
    let mut owners: HashMap<String, &str> = HashMap::new();
    for input in &inputs {
        if let Some(other) = owners.insert(topic_name("robot", input), input) {
            panic!("{input} and {other} escape to one topic");
        }
    }
    // No feedback is published on an input's topic, or on another key's of
    // the face: the bridge publishes each.
    for (key, topic) in feedback_topics() {
        assert!(
            !owners.contains_key(topic),
            "{key} escapes to the topic of input {}",
            owners[topic]
        );
    }
    let mut published: HashMap<String, &str> = HashMap::new();
    let outputs = keys
        .iter()
        .filter(|key| key["__meta"]["editable"] == false)
        .filter_map(|key| key["path"].as_str())
        .filter(|path| path.starts_with(X) || path.starts_with(COLOR));
    for output in outputs {
        if let Some(other) = published.insert(topic_name("robot", output), output) {
            panic!("{output} and {other} escape to one topic");
        }
    }
}

/// A feedback is its target's number, an `f64` from the face's rest on: the
/// bridge types a key's topic once, from the first value it publishes, so a
/// feedback resting at another type would keep that topic type and drop
/// every feedback after it.
#[tokio::test]
async fn a_feedback_holds_its_target_s_type_from_the_rest_on() {
    let (_device, port) = serve(None);
    let feedbacks = feedback_topics().map(|(key, _)| key);
    let keys = list_keys(port, |keys| {
        feedbacks
            .iter()
            .all(|key| keys.iter().any(|listed| listed["path"] == key.as_str()))
    })
    .await;
    for key in &feedbacks {
        let listed = keys
            .iter()
            .find(|listed| listed["path"] == key.as_str())
            .unwrap_or_else(|| panic!("{key} is listed"));
        assert_eq!(listed["__meta"]["ty"], "f64", "{key}: {listed}");
    }
}

/// A ROS 2 node publishing a `std_msgs/Float64` on each form's topic moves
/// the face, and reads the feedback back on its topics, as `std_msgs/Float64`
/// like the target.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
#[cfg_attr(
    target_os = "macos",
    ignore = "DDS multicast SPDP discovery is unreliable on macOS loopback (rustdds has no \
              unicast-peer/interface config); this runs on Linux CI. To run locally, ensure an \
              active multicast-capable interface and use --ignored."
)]
async fn a_float_published_on_each_form_s_topic_moves_the_face_and_its_feedback() {
    use arora_bridge_ros2::msg_types::{Float64, MessageType};
    use rand::RngExt;
    use ros2_client::{
        Context, ContextOptions, Name, NodeName, NodeOptions, DEFAULT_PUBLISHER_QOS,
        DEFAULT_SUBSCRIPTION_QOS,
    };
    use vizij_api_core::value::{as_float, as_vector};

    let domain: u16 = rand::rng().random_range(1..=200);
    let namespace = format!("studio_ros2_{}", rand::rng().random_range(0..1_000_000u32));
    let (device, port) = serve(Some((namespace.clone(), domain)));
    // The device is up once its local bridge answers.
    list_keys(port, |_| true).await;

    let context =
        Context::with_options(ContextOptions::new().domain_id(domain)).expect("a ROS 2 context");
    let mut node = context
        .new_node(
            NodeName::new("/", "studio_ros2_peer").expect("a node name"),
            NodeOptions::new(),
        )
        .expect("a ROS 2 node");
    tokio::spawn(node.spinner().expect("a spinner").spin());

    let topic_of = |key: &str| Name::parse(&topic_name(&namespace, key)).expect("a valid topic");
    // The feedback, read as the bridge publishes it.
    let mut feedback = Vec::new();
    for (key, _) in feedback_topics() {
        let topic = node
            .create_topic(
                &topic_of(&key),
                Float64::message_type_name(),
                &DEFAULT_SUBSCRIPTION_QOS,
            )
            .expect("a feedback topic");
        let subscription = node
            .create_subscription::<Float64>(&topic, None)
            .expect("a feedback subscription");
        feedback.push((key, subscription));
    }

    let flow = async {
        for (round, (key, _)) in target_topics().into_iter().enumerate() {
            let value = 0.125 * (round + 1) as f64;
            let topic = node
                .create_topic(
                    &topic_of(&key),
                    Float64::message_type_name(),
                    &DEFAULT_PUBLISHER_QOS,
                )
                .expect("a target topic");
            let publisher = node
                .create_publisher::<Float64>(&topic, None)
                .expect("a target publisher");
            tokio::time::timeout(
                Duration::from_secs(30),
                publisher.wait_for_subscription(&node),
            )
            .await
            .unwrap_or_else(|_| panic!("the device subscribes {key}"));
            // The face moves: the number, or the colour's red component.
            let moved = || {
                device.rig.pose().into_iter().any(|(path, pose)| {
                    let path = path.to_string();
                    (path == X && as_float(&pose) == Some(value as f32))
                        || (path == COLOR
                            && as_vector(&pose).and_then(|rgb| rgb.first().copied())
                                == Some(value as f32))
                })
            };
            let mut landed = false;
            for _ in 0..100 {
                let _ = publisher.async_publish(Float64 { data: value }).await;
                tokio::time::sleep(Duration::from_millis(50)).await;
                if moved() {
                    landed = true;
                    break;
                }
            }
            assert!(
                landed,
                "a publish on {key}'s topic moves the face to {value}"
            );
            // Its feedback reads back on each of its topics.
            for (feedback_key, subscription) in &feedback {
                let is_x = key.starts_with(X);
                if is_x != feedback_key.starts_with(X) {
                    continue;
                }
                let read = tokio::time::timeout(Duration::from_secs(10), async {
                    loop {
                        if let Ok((message, _)) = subscription.async_take().await {
                            if message.data == value {
                                return;
                            }
                        }
                    }
                })
                .await;
                assert!(read.is_ok(), "{feedback_key} reads {value} after {key}");
            }
        }
    };
    tokio::time::timeout(Duration::from_secs(120), flow)
        .await
        .expect("the ROS 2 flow ends within 120 s");
}
