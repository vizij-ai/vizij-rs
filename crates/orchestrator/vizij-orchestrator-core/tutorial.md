# vizij-orchestrator-core Tutorial

Welcome! This guided tour shows how to build reliable orchestration pipelines with
`vizij-orchestrator-core`. By the end you will understand the moving pieces, wire graphs and
animations together, merge multiple graph specs into a single controller, and reason about the
blackboard and scheduling model.

> **Who is this for?** Rust developers who want deterministic coordination between Vizij graph and
> animation controllers in native or host applications.

---

## 1. Conceptual Map

| Concept             | Description                                                                                  |
|---------------------|----------------------------------------------------------------------------------------------|
| **Blackboard**      | A typed key–value store keyed by `TypedPath`. Controllers read staged inputs and push writes.|
| **Graph controller**| Wraps a `GraphSpec` from `vizij-graph-core`, owns a persistent `GraphRuntime`.               |
| **Animation controller** | Bridges blackboard values to `vizij-animation-core` inputs and players.              |
| **Schedule**        | Orchestrator runtime order: `SinglePass`, `TwoPass`, future strategies.                      |
| **Frame**           | A call to `Orchestrator::step(dt)` produces `OrchestratorFrame` with merged writes, conflicts, timings. |

Controllers only see the blackboard paths you subscribe them to, keeping evaluation deterministic.

---

## 2. Project Setup

Add the crate to your workspace (replace `path` with your checkout if you vendor the repo):

```bash
cargo add vizij-orchestrator-core --path crates/orchestrator/vizij-orchestrator-core
```

You will commonly pair it with the companion crates:

- `vizij-graph-core` for authoring graphs.
- `vizij-animation-core` if you need animation controllers.
- `vizij-api-core` for shared `Value`, `TypedPath`, and JSON helpers.

---

## 3. Hello Orchestrator

```rust
use vizij_orchestrator_core::{
    controllers::{GraphControllerConfig, Subscriptions},
    Orchestrator, Schedule,
};
use vizij_graph_core::types::GraphSpec;
use vizij_api_core::{TypedPath, Value};

fn build_graph_spec() -> GraphSpec {
    serde_json::from_value(serde_json::json!({
        "nodes": [
            { "id": "input_gain", "type": "input", "params": { "path": "demo/gain" } },
            { "id": "const_offset", "type": "constant", "params": { "value": { "type": "float", "data": 0.5 } } },
            { "id": "multiply", "type": "multiply" },
            { "id": "publish", "type": "output", "params": { "path": "demo/output/value" } }
        ],
        "links": [
            { "from": { "node_id": "input_gain" }, "to": { "node_id": "multiply", "input": "lhs" } },
            { "from": { "node_id": "const_offset" }, "to": { "node_id": "multiply", "input": "rhs" } },
            { "from": { "node_id": "multiply" }, "to": { "node_id": "publish", "input": "in" } }
        ]
    })).expect("valid graph spec")
}

fn main() -> anyhow::Result<()> {
    let graph_cfg = GraphControllerConfig {
        id: "graph:gain".into(),
        spec: build_graph_spec(),
        subs: Subscriptions {
            inputs: vec![TypedPath::parse("demo/gain")?],
            outputs: vec![TypedPath::parse("demo/output/value")?],
            mirror_writes: true,
        },
    };

    let mut orchestrator = Orchestrator::new(Schedule::SinglePass)
        .with_graph(graph_cfg);

    orchestrator.set_input(
        "demo/gain",
        serde_json::json!({ "type": "float", "data": 2.0 }),
        None,
    )?;

    let frame = orchestrator.step(1.0 / 60.0)?;
    let out = frame.merged_writes.iter()
        .find(|op| op.path.to_string() == "demo/output/value")
        .map(|op| op.value.clone())
        .unwrap();

    println!("Output value: {:?}", out); // Value::Float(1.0)
    Ok(())
}
```

### Key takeaways
- Use JSON shorthand to author graphs; normalization happens automatically via serde.
- Subscriptions constrain which blackboard paths get staged or published.
- `step(dt)` advances the orchestrator epoch, runs the schedule, and returns deterministic writes.

---

## 4. Working with the Blackboard

```rust
use vizij_orchestrator_core::{Blackboard, BlackboardEntry, Orchestrator, Schedule};
use vizij_api_core::{TypedPath, Value, WriteBatch, WriteOp};

let mut orch = Orchestrator::new(Schedule::SinglePass);
let tp = TypedPath::parse("robot/arm.joint")?;

// Manual staging
orch.blackboard.set_entry(tp.clone(), BlackboardEntry::new(Value::Float(0.2)))?;

// Apply a batch (e.g., from graph controller writes)
let mut batch = WriteBatch::new();
batch.push(WriteOp::new(tp.clone(), Value::Float(0.5)));
let conflicts = orch.blackboard.apply_writebatch(batch, orch.epoch, "graph:0".into());

if !conflicts.is_empty() {
    println!("Conflicts: {:?}", conflicts);
}
```

The orchestrator uses last-writer-wins semantics and captures conflict logs for introspection.

---

## 5. Schedules Explained

| Schedule       | Order                                                          | Use case                                                                 |
|----------------|----------------------------------------------------------------|--------------------------------------------------------------------------|
| `SinglePass`   | Animations → Graphs (or whichever controllers you register)    | Most pipelines; deterministic when feedback loops are not required.      |
| `TwoPass`      | Graphs → Animations → Graphs                                   | Allows graphs to react to animation writes from the same frame.          |
| `RateDecoupled`| Placeholder; currently aliases to `SinglePass`.                | Reserved for future multi-rate simulation.                               |

Change schedules when creating the orchestrator or at runtime (if you rebuild controllers accordingly).

---

## 6. Animations + Graphs

```rust
use vizij_orchestrator_core::{
    controllers::{animation::AnimationControllerConfig, GraphControllerConfig, Subscriptions},
};

let anim_cfg = AnimationControllerConfig {
    id: "anim:player".into(),
    setup: serde_json::json!({
        "animation": animations::load("scalar-ramp")?,
        "player": { "name": "demo-player", "loop_mode": "loop" }
    }),
};

let mut orch = Orchestrator::new(Schedule::TwoPass)
    .with_graph(graph_cfg)
    .with_animation(anim_cfg);

orch.prebind(|resolver| resolver.set(String::from("anim/target"), String::from("robot/arm.joint")));
```

Animation controllers map blackboard commands to animation engine operations. When using multi-pass
(`TwoPass`), ensure your graphs subscribe to the animation outputs they depend on.

---

## 7. Merging Graphs

Graph specs often form domains (IO graph, compute graph, etc.). Instead of staging their outputs
via the blackboard every frame, merge them into a single controller:

```rust
use vizij_orchestrator_core::controllers::GraphControllerConfig;

let merged = GraphControllerConfig::merged_with_options(
    "graph:merged",
    vec![io_graph_cfg, compute_graph_cfg],
    GraphMergeOptions {
        output_conflicts: OutputConflictStrategy::Error,
        intermediate_conflicts: OutputConflictStrategy::BlendEqualWeights,
    },
)?;

let mut orch = Orchestrator::new(Schedule::SinglePass)
    .with_graph(merged);
```

### What merge does
- Namespaces node IDs (`g0_io::node`, `g1_compute::node`) to avoid collisions.
- Replaces matching `Input` nodes that read an upstream graph’s output path with direct links.
- Preserves unmatched inputs and their subscriptions so you can still stage host values.
- Emits `GraphMergeError::ConflictingOutputs` if two graphs drive the same path, or
  `GraphMergeError::OutputMissingUpstream` if an output node has no source (e.g., partially defined).
- `GraphMergeOptions` lets you choose how to handle overlaps:
  - `Error` keeps the original behaviour (panic on conflicts).
  - `BlendEqualWeights` injects a `default-blend` node with equal weights so downstream graphs see one value.
  - `Namespace` rewrites final output paths to `graph_id/original/path` to keep parallel values distinct.

See `controllers::graph::tests::*` for edge-case coverage and expected behavior.

---

## 8. Reading Diagnostics

`OrchestratorFrame` contains:

- `merged_writes`: deterministic list of `WriteOp`s for downstream consumers.
- `conflicts`: `ConflictLog` for any overwritten blackboard entries.
- `timings_ms`: synthetic timings (currently derived from `dt`).
- `events`: controller-specific payloads (animations populate this).

Example:

```rust
let frame = orch.step(dt)?;
for log in &frame.conflicts {
    println!(
        "Conflict on {}: previous={:?}, new={:?}",
        log.path, log.previous_value, log.new_value
    );
}
```

---

## 9. Testing Strategies

- **Unit tests**: Place focused tests alongside modules (see `controllers::graph::tests` for examples).
- **Integration tests**: Use `tests/integration_passes.rs` as a template—load shared fixtures, step the
  orchestrator, assert writes, conflict logs, and mirroring.
- **Fixtures**: `vizij-test-fixtures` exposes canonical graph / animation fixtures to reduce ceremony.

```rust
#[test]
fn graph_pipeline_outputs_expected_value() {
    let cfg = graph_fixture("simple-gain-offset");
    let mut orch = Orchestrator::new(Schedule::SinglePass)
        .with_graph(cfg);

    let frame = orch.step(1.0 / 60.0).expect("step ok");
    assert!(frame.merged_writes.iter().any(|op| op.path.to_string() == "demo/output/value"));
}
```

---

## 10. Advanced Tips

- **Mirror writes selectively**: Set `Subscriptions::mirror_writes` to `false` when you want internal
  state to stay private yet still emit filtered writes to consumers.
- **Manual runtime staging**: `GraphController::evaluate` reuses the existing `GraphRuntime`. For
  custom host flows, populate `rt.writes` or `rt.set_input` before calling `evaluate_all`.
- **Custom schedules**: Implement your own scheduler on top of `Orchestrator` by reading controllers
  directly (`orchestrator.graphs` / `orchestrator.anims`) if you need bespoke ordering.
- **Error surfaces**: Graph evaluation returns `String` errors from `vizij-graph-core`. Wrap them in
  your logging/telemetry pipeline to guide authors.

---

## 11. Next Steps

1. Browse `examples/` for more scenarios (`graph_only.rs`, `two_pass.rs`, etc.).
2. Link with the wasm package (`@vizij/orchestrator-wasm`) to share graph specs across JS tooling.
3. Add continuous testing (`cargo test -p vizij-orchestrator-core`, `cargo clippy`, `cargo fmt`) to
   your project to catch regressions.

With these pieces, you can orchestrate rich multi-controller workflows confidently. Happy building! 🚀
