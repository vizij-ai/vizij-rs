# Merge Orchestration Deep Dive

This guide teaches everything you need to know about `GraphControllerConfig::merged` and the
single-pass orchestration model built on merged graphs. It covers edge cases, implementation
details, performance heuristics, and debugging strategies so you can confidently integrate the
merger in production systems.

---

## 1. Why Merge Graphs?

### Problem
Multiple graphs often communicate via blackboard staging:

1. Graph A publishes to `shared/value`.
2. Graph B reads the same path via an `Input` node.
3. The orchestrator stages writes from A, then B consumes them on the next evaluation.

This introduces frame latency, extra blackboard traffic, and complex pass ordering.

### Merge Solution
`GraphControllerConfig::merged` (or `merged_with_options`) builds a single `GraphSpec` that rewires compatible input/output
pairs directly:

- `Output` → `Input` edges become explicit graph edges.
- Both nodes execute in the same topological order.
- Shared values no longer traverse the blackboard, eliminating pass dependencies.

Use merging when:

- Graphs form a clear producer → consumer pipeline.
- You need deterministic, single-pass evaluation.
- Intermediate outputs should remain internal (not exposed to hosts).

Keep separate controllers when:

- You intentionally require blackboard persistence across frames.
- Graphs live in different runtime domains (e.g., heavy compute vs. streaming IO).
- You need visibility into intermediate values for tooling or debugging.

---

## 2. Merge Algorithm Overview

### Configuring conflict strategies

`GraphMergeOptions` controls what happens when multiple graphs produce the same `TypedPath`.

```rust
use vizij_orchestrator_core::{
    controllers::{GraphControllerConfig, GraphMergeOptions, OutputConflictStrategy},
    Orchestrator, Schedule,
};

let merged = GraphControllerConfig::merged_with_options(
    "motion-bundle",
    vec![graph_a_cfg, graph_b_cfg, graph_c_cfg],
    GraphMergeOptions {
        output_conflicts: OutputConflictStrategy::Namespace,
        intermediate_conflicts: OutputConflictStrategy::BlendEqualWeights,
    },
)?;

let orch = Orchestrator::new(Schedule::SinglePass)
    .with_graph(merged);
```

| Strategy | Effect |
|----------|--------|
| `Error` | Preserve the legacy behaviour; merging fails with `GraphMergeError::ConflictingOutputs`. |
| `Namespace` | Keep each producer’s output separate by renaming the final `Output` paths to `graph_id/original/path`. |
| `BlendEqualWeights` | Inject a `default-blend` node (plus equal-weight constant) so downstream consumers see a single averaged value. |

Separate `output_conflicts` and `intermediate_conflicts` let you choose different policies for
terminal outputs (host-facing) vs. intermediate edges consumed by another graph in the merged set.
Namespace is only supported for final outputs—intermediate overlaps must be blended or produce an
error.

`GraphControllerConfig::merged(merged_id, graphs)` performs several phases:

1. **Namespace & clone nodes.**
   - Each graph’s node IDs receive a deterministic namespace prefix (`g{index}_{sanitized_id}`).
   - Collisions inside the refined spec are impossible.

2. **Track input/output nodes.**
   - Input nodes record their `TypedPath`.
   - Output nodes record the `TypedPath` they publish.

3. **Build link bindings.**
   - For every `Output` node with a `path`, the merger finds incoming edges and associates them with
     that path (`OutputBinding`).
   - If multiple graphs produce the same path, the merge aborts with
     `GraphMergeError::ConflictingOutputs`.

4. **Validate outputs.**
   - Each output must have at least one upstream connection in the original graph; otherwise the
     merge fails with `GraphMergeError::OutputMissingUpstream`.

5. **Rewire inputs.**
   - For every `Input` node whose path matches a known binding, the merger rewrites all outgoing
     edges so they originate from the upstream node.
   - Selectors compose: binding selector is prepended to the original link selector, preserving
     projection semantics.
   - Inputs replaced by direct edges are removed from the merged node list and their paths are
     filtered out of the subscription list.

6. **Subscriptions and mirror writes.**
   - All source graph subscriptions are unioned (`IndexSet` preserves deterministic ordering).
   - Removed inputs’ paths are filtered out; remaining inputs stay staged by the host.
   - `mirror_writes` is true if any source config enabled it.

7. **Result assembly.**
   - Returns a new `GraphControllerConfig` with the merged spec and consolidated subscriptions.

---

## 3. Namespacing Strategy

- Namespace format: `g{index}_{graph_id}` where `graph_id` has non-alphanumeric characters replaced
  with `_`.
- If a node ID clashes after namespacing, a stable suffix `__{counter}` is appended.
- Example:

  ```
  original: "blend"
  namespace: "g0_motion::blend"
  collision -> "g0_motion::blend__1"
  ```

Practical tips:

- Keep source graph IDs short and descriptive; they appear in diagnostics.
- For debugging, search for the namespace prefix in merged specs to pinpoint node lineage.

---

## 4. Handling Selectors

Selectors (field/index projections) are stored as `Vec<SelectorSegment>`. The merger composes them
when rewiring:

```
Output selector: [Index(1)]
Consumer selector: [Field("y")]
Merged selector: [Index(1), Field("y")]
```

Implications:

- Order matters: binding selector executes first, consumer selector afterwards.
- If the consumer already had a selector, the binding selector is prepended.
- Missing selectors remain untouched.

Testing coverage: `merged_graph_composes_selectors` in `controllers::graph::tests`.

---

## 5. Subscription Semantics

After rewiring, only inputs without bindings remain as host-facing subscriptions. The merger does:

```rust
let inputs: Vec<TypedPath> = merged_input_paths
    .into_iter()
    .filter(|tp| !removed_input_paths.contains(&tp.to_string()))
    .collect();
```

Tips:

- Ensure graphs declare subscriptions for every external path they require.
- When merging, consider explicitly setting subscriptions before invoking `merged` to avoid staging
  unused paths.
- `mirror_writes` ORs across configs—disable it in all merged graphs if you do not need mirrored
  internal state.

---

## 6. Error Handling

`GraphMergeError` variants:

- `Empty`: no graphs supplied.
- `ConflictingOutputs { path, graphs }`: multiple graphs publish the same `TypedPath`.
- `OutputMissingUpstream { node_id, graph }`: an output node had no incoming connection.

Guidance:

- Catch errors when calling `GraphControllerConfig::merged` or `Orchestrator::with_merged_graph`.
- Surface them to users so they can rename paths or fix graph specs.
- For debugging, inspect the merged spec by serializing `merged.spec` to JSON.

---

## 7. Orchestrator Integration

### With fluent API

```rust
let merged_cfg = GraphControllerConfig::merged("graph:merged", vec![cfg_a, cfg_b])?;
let mut orchestrator = Orchestrator::new(Schedule::SinglePass)
    .with_graph(merged_cfg);
```

### Using helper

```rust
let orchestrator = Orchestrator::new(Schedule::SinglePass)
    .with_merged_graph("graph:merged", vec![cfg_a, cfg_b])?;
```

The helper ensures the merged config is inserted as a controller with the merged ID.

---

## 8. Debugging & Inspection

1. **Serialize the merged spec.**

   ```rust
    let json_spec = serde_json::to_string_pretty(&merged_cfg.spec)?;
    println!("{json_spec}");
   ```

2. **Check subscriptions.**

   ```rust
   for path in &merged_cfg.subs.inputs {
       println!("Input subscription: {}", path);
   }
   ```

3. **Inspect runtime outputs.**
   Use `frame.merged_writes` with the merged controller to ensure expected paths appear.

4. **Log GraphRuntime state.**
   After evaluation, `controller.rt.outputs` contains the last frame outputs keyed by (namespaced)
   node ID.

---

## 9. Performance Considerations

- Merged graphs reduce blackboard traffic and avoid cross-frame dependencies.
- Graph evaluation traverses the union of all nodes; redundant inputs removed during merge lower the
  graph size.
- Use `SinglePass` schedule whenever merged graphs represent the bulk of your pipeline; reserve
  multi-pass scheduling for unavoidable feedback loops.

---

## 10. Testing Checklist

Leverage provided tests as references:

- **Unit** (`controllers::graph::tests`):
  - `merged_graph_errors_on_empty`
  - `merged_graph_errors_when_output_missing_source`
  - `merged_graph_namespaces_node_ids`
  - `merged_graph_preserves_unmatched_inputs`
  - `merged_graph_composes_selectors`

- **Integration** (`tests/integration_passes.rs`):
  - `merged_graph_rewires_shared_output`
  - `merge_reports_conflicting_outputs`

Recommended custom tests:

- Run `evaluate_all` on the merged spec to confirm expected outputs.
- Validate blackboard subscriptions match your host staging requirements.
- Include serialization/deserialization round-trips if persisting merged specs.

---

## 11. Best Practices

- **Namespace proactively**: Choose descriptive IDs (`graph:io`, `graph:rig`). The namespace prefix
  remains visible after merge, simplifying debugging.
- **Guard merges with tests**: especially when adding new graphs to a shared bundle.
- **Check for shared outputs**: If multiple graphs intentionally write to the same path, either keep
  them as separate controllers or rename the outputs before merging.
- **Document merged specs**: Persist normalized merged JSON for tooling and offline inspection.
- **Leverage `Orchestrator::with_merged_graph`**: encapsulates merging and registration in one step.

You’re now prepared to wield the merge system with confidence—refactor pipelines into single-pass
graphs, eliminate redundant blackboard chatter, and deliver deterministic, high-performance
orchestration flows.
