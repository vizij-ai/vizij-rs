//! The node-graph fixtures, by name: the graphs the tests and the benchmark
//! read. Each is a JSON document whose `spec` field holds the graph.

/// Every fixture: its name and its JSON text.
pub const ALL: &[(&str, &str)] = &[
    (
        "logic-gate",
        include_str!("../../../../fixtures/node_graphs/logic-gate.json"),
    ),
    (
        "math-toolbox",
        include_str!("../../../../fixtures/node_graphs/math-toolbox.json"),
    ),
    (
        "merged-consumer",
        include_str!("../../../../fixtures/node_graphs/merged-consumer.json"),
    ),
    (
        "merged-source-primary",
        include_str!("../../../../fixtures/node_graphs/merged-source-primary.json"),
    ),
    (
        "merged-source-secondary",
        include_str!("../../../../fixtures/node_graphs/merged-source-secondary.json"),
    ),
    (
        "merged-source-tertiary",
        include_str!("../../../../fixtures/node_graphs/merged-source-tertiary.json"),
    ),
    (
        "piecewise-remap-demo",
        include_str!("../../../../fixtures/node_graphs/piecewise-remap.json"),
    ),
    (
        "rig-blend",
        include_str!("../../../../fixtures/node_graphs/rig-blend.json"),
    ),
    (
        "sign-graph",
        include_str!("../../../../fixtures/node_graphs/sign-graph.json"),
    ),
    (
        "simple-gain-offset",
        include_str!("../../../../fixtures/node_graphs/simple-gain-offset.json"),
    ),
    (
        "sine-driver",
        include_str!("../../../../fixtures/node_graphs/sine-driver.json"),
    ),
    (
        "slew-graph",
        include_str!("../../../../fixtures/node_graphs/slew-graph.json"),
    ),
    (
        "urdf-fk-ik-roundtrip",
        include_str!("../../../../fixtures/node_graphs/urdf-fk-ik-roundtrip.json"),
    ),
    (
        "urdf-ik-position",
        include_str!("../../../../fixtures/node_graphs/urdf-ik-position.json"),
    ),
    (
        "weighted-average",
        include_str!("../../../../fixtures/node_graphs/weighted-blend-graph.json"),
    ),
    (
        "weighted-layers",
        include_str!("../../../../fixtures/node_graphs/weighted-layers.json"),
    ),
    (
        "weighted-profile-blend",
        include_str!("../../../../fixtures/node_graphs/blend-graph.json"),
    ),
    (
        "weighted-sum-helper",
        include_str!("../../../../fixtures/node_graphs/weighted-sum-helper-graph.json"),
    ),
];

/// The JSON text of the fixture `name`.
pub fn json(name: &str) -> &'static str {
    ALL.iter()
        .find(|(known, _)| *known == name)
        .map(|(_, json)| *json)
        .unwrap_or_else(|| panic!("unknown node-graph fixture '{name}'"))
}

/// The fixture `name`, parsed.
pub fn spec<T: serde::de::DeserializeOwned>(name: &str) -> serde_json::Result<T> {
    serde_json::from_str(json(name))
}
