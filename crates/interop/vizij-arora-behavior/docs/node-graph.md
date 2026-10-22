# Vizij's node graph as an Arora behavior interpreter

Vizij's node graph is a **behavior interpreter** in the Arora sense. Read Arora's **[how a behavior interpreter works](https://github.com/semio-ai/arora-sdk/blob/main/crates/arora-behavior/docs/interpreter-workflow.md)** first — it defines the `BehaviorInterpreter` contract (the `tick` / `apply` / `load` lifecycle, the `BehaviorContext` store, timing as built-in store keys) that this page assumes. Here we cover what is *specific* to the node graph, and draw the parallel with the other reference interpreter, [`arora-behavior-tree`](https://github.com/semio-ai/arora-sdk/blob/main/crates/arora-behavior-tree/docs/nodes.md).

The interpreter type is [`ProcessingGraph`](../src/lib.rs#L75), which implements `arora_behavior::BehaviorInterpreter` ([`lib.rs:176`](../src/lib.rs#L176)). The evaluator underneath it is arora-sdk's behavior tree: [`arora-node-graph`](https://github.com/semio-ai/arora-sdk/blob/main/crates/arora-node-graph/README.md) imports the graph as a tree's detached nodes and runs it (`TreeRuntime`). Vizij and Arora share **one runtime value type** (`arora_shape::Value` *is* `arora_types::value::Value`), so values cross the store and call boundaries with no conversion ([`vizij-arora/src/lib.rs:1-13`](../../vizij-arora/src/lib.rs#L1-L13)).

## Lifecycle mapping

The interpreter's spine is `tick_store` ([`lib.rs:135-177`](../src/lib.rs#L135-L177)), which `tick` calls each frame:

| Interpreter lifecycle | Vizij node graph |
|---|---|
| load | `load` retains the new structural graph and re-lowers it to the spec on the next tick, keeping the graph runtime **warm** — surviving nodes retain their integration state (springs/dampers/URDF) and the graph clock stays continuous; a recompose never rebuilds the device ([`load`, `lib.rs`](../src/lib.rs)) |
| time update | `dt` is read from the built-in `arora/dt` store key and advances the graph clock (`rt.t += dt`) ([`built_in_dt_seconds`, `lib.rs`](../src/lib.rs)) |
| tick | if edited, re-lower the retained graph and load the tree again, then stage subscribed inputs → tick the tree → flush outputs; **always returns `BehaviorStatus::Running`** ([`lib.rs`](../src/lib.rs)) |
| graph update | `apply(GraphDiff)` edits the retained structural graph and re-lowers on the next tick — an unedited node keeps its id, so its runtime state survives (see [Editing](#editing-structural-load-or-graphdiff)) |

The single most important contrast with a behavior tree: **a node graph is continuous.** It reports `Running` every frame ([`lib.rs:185`](../src/lib.rs#L185)), whereas a tree runs to a terminal status and reports `Done`. Timing is data for both — the graph reads `arora/dt` exactly as the tree does.

## What a node is in Vizij

A node graph is **dataflow**, not control flow. A node is one [`NodeSpec { id, kind, params, output_shapes, input_defaults }`](https://github.com/semio-ai/arora-sdk/blob/main/crates/arora-node-graph/src/types.rs); edges ([`EdgeSpec`](https://github.com/semio-ai/arora-sdk/blob/main/crates/arora-node-graph/src/types.rs)) wire a named **output port** of one node to a named **input port** of another, with an optional field/index selector. `NodeType` is a large value-producing vocabulary — arithmetic, trig, time/oscillators, springs/damping, logic, vectors, IK/FK, records — plus the three that form the Arora seam ([`types.rs`](https://github.com/semio-ai/arora-sdk/blob/main/crates/arora-node-graph/src/types.rs)): `Input` (reads a staged host value by path), `Output` (writes a value to a host path sink), and `ExternalFunction` (invokes a module by opaque id).

On the tree each node is a detached call of its kind's function ([`functions.rs`](https://github.com/semio-ai/arora-sdk/blob/main/crates/arora-node-graph/src/functions.rs)): it runs when what it outputs is read, once per tick, and only when its arguments changed. The graph's `output` nodes are the tree's sinks, brought up to date at every tick, so a tick runs what the outputs read and changed since the last; a node nothing reads does not run. A node reads named input ports and writes named output ports, and its function runs the node's evaluation as the reference evaluator defines it.

### Vizij node vs. behavior-tree node

| | [arora-behavior-tree node](https://github.com/semio-ai/arora-sdk/blob/main/crates/arora-behavior-tree/docs/nodes.md) | Vizij graph node |
|---|---|---|
| A tick produces | a `Status` — `Success` / `Failure` / `Running` | `PortValue`s on its output ports (no status) |
| An edge means | an argument/child edge (control flow) — a parent ticks children and reacts to status | a dataflow edge — an output value feeds an input port |
| Traversal | recursive tick driven by control nodes (seq/fallback) | detached nodes, pulled by the sinks, each run when its inputs changed |
| Termination | runs to a terminal status → interpreter reports `Done` | continuous → interpreter reports `Running` forever |

This duality is exactly what the shared behavior-graph model anticipates: it "does not know how any particular interpreter walks the links — the behavior tree reads them as argument/child edges; a node graph reads them as dataflow" ([`arora-behavior/src/graph.rs:12-16`](https://github.com/semio-ai/arora-sdk/blob/main/crates/arora-behavior/src/graph.rs#L12-L16)).

## How nodes touch the data store

The graph does **not** touch the store during node evaluation — it *stages in* and *flushes out*, both inside `tick_store`:

```mermaid
flowchart LR
  store[("Arora data store")]
  subgraph tick["ProcessingGraph::tick_store"]
    direction TB
    stage["stage subscribed Input paths<br/>(store.read → rt.set_input)"]
    eval["TreeRuntime::evaluate_with<br/>(one tick of the tree)"]
    flush["drain rt.writes → one StateChange<br/>(store.write)"]
    stage --> eval --> flush
  end
  store -- "read subscribed paths" --> stage
  flush -- "one write per tick" --> store
```

- **Read (store → graph):** each subscribed input path (the graph's `Input` nodes, [`input_paths`, `lib.rs:98-104`](../src/lib.rs#L98-L104)) is read from the store and staged into the runtime ([`tick_store`](../src/lib.rs)); an `Input` node's call reads it.
- **Write (graph → store):** only sink `Output` nodes emit writes; they are collected after the tick and drained into a **single `StateChange`** written once per tick ([`tick_store`](../src/lib.rs)). A path-less `Output` with `key_field`/`value_field` expands an array-of-records into one write per record, which the runtime applies, as the tree binds an output to one static key.

This is the same store seam a behavior tree uses (read inputs, write outputs), but staged at the tick boundary rather than per-node — where the tree binds each `{var}` to a store `Slot` and reads/writes it mid-tick, the graph batches a read pass before and a write pass after evaluation. Round-trip test: [`graph_reads_and_writes_the_arora_store`](../src/lib.rs#L269-L293).

## How nodes call modules (and animation)

A node reaches a module through an `ExternalFunction` node, which carries only a stable **function UUID** plus opaque arg-key handles. On the tree its place follows the description the device's function index gives of its function, which `ProcessingGraph::tick` hands the importer: a call to a function that returns nothing and writes nothing back is left out, as nothing could read it; one to a function that takes no parameter (`player_states()`) is a leaf under the root, ticked at every tick, as its result can only come from outside its arguments; any other is a detached node, called when what it returns is read and its arguments changed, as the animation module's `step` with the clock is. A function the index does not describe is ticked at every tick. Its call goes through `@arora/node-graph`'s `external` function:

```mermaid
flowchart LR
  extnode["ExternalFunction node<br/>(function UUID + args)"] --> hostcall["@arora/node-graph external(…)"]
  hostcall --> adapter["NodeGraphModule<br/>(function → module map)"]
  adapter --> call["CallBridge::arora_call<br/>Call { module_id, id, args }"]
  call --> engine["Arora engine module<br/>(e.g. animation)"]
  engine -- "result.ret" --> extnode
```

The node-graph module looks up the `function → module` id the interpreter set (`ProcessingGraph::set_function_modules`; the engine routes a `Call` by its `module_id`), builds `Call { module_id, id: function, args }` from the opaque UUIDs, dispatches through the `CallBridge` the interpreter got in its `BehaviorContext`, and returns `result.ret`. **Animation** is one such module: `vizij-animation-core` packaged as a standalone Arora wasm module ([`vizij-animation-module/src/lib.rs:1-31`](../../vizij-animation-module/src/lib.rs#L1-L31)), reached the same way as any other — an `ExternalFunction` node whose function/module UUIDs point at it.

This is the UUID-keyed analogue of the behavior tree's module dispatch (a non-built-in node id resolved through the engine): both go through `CallBridge::arora_call`; the graph routes by an explicit `function → module` map, the tree by a `function_index`. Dispatch test: [`external_function_dispatches_through_host_and_sets_output`](https://github.com/semio-ai/arora-sdk/blob/main/crates/arora-node-graph/src/tree_tests/external_function_tests.rs).

## Editing: structural load or GraphDiff

`ProcessingGraph` edits exactly like the behavior tree — a whole-graph `load` or an incremental `apply(GraphDiff)` — because it holds the shared model's **structural** graph (one shared `Node` per Vizij node — see [`graph_codec`](../src/graph_codec.rs)) as its editable source of truth, and lowers it to the spec it evaluates (`graph_codec::decode`) on the next tick after a change.

- **load** installs a new structural graph whole as the main behavior; the live task runs (fragments under `task/<run id>`) carry over into it, with their state.
- **apply(GraphDiff)** adds/removes nodes and links on the retained graph. The wasm boundary takes a Vizij spec-level diff (`GraphSpecDiff`: `upsert_nodes` / `remove_nodes` / `upsert_edges` / `remove_edges`) and translates it to an `arora_behavior::GraphDiff` with the *same* per-node/per-edge encoders as `encode` (`graph_codec::spec_diff_to_graph_diff`); it reaches the interpreter as the engine's EDIT call. An upserted node is removed then re-added, so every edge incident to it rides along in `upsert_edges`.

Both keep the graph runtime **warm**: the tree is loaded again from the changed spec, and every node it keeps takes over its state (springs/dampers/URDF chains) while the graph clock stays continuous — a recompose restarts no stateful node, and an edit restarts only what it touched (an unedited node keeps its id, and so its state). A spec of a new version is loaded at the next tick. Tests: in-place swap ([`load_swaps_the_graph_in_place`](../src/lib.rs)), incremental edit ([`apply_edits_the_running_graph`](../src/lib.rs)), warm continuity ([`load_keeps_the_graph_runtime_warm`](../src/lib.rs)).

## Source map

| Concept | File |
|---|---|
| `ProcessingGraph` (the interpreter), `tick_store` | [`crates/interop/vizij-arora-behavior/src/lib.rs`](../src/lib.rs) |
| Structural encoding (load/edit), `GraphSpecDiff` → `GraphDiff` | [`crates/interop/vizij-arora-behavior/src/graph_codec.rs`](../src/graph_codec.rs) |
| Graph spec / node / edge types | [`arora-node-graph/src/types.rs`](https://github.com/semio-ai/arora-sdk/blob/main/crates/arora-node-graph/src/types.rs) |
| The importer onto the tree | [`arora-node-graph/src/import.rs`](https://github.com/semio-ai/arora-sdk/blob/main/crates/arora-node-graph/src/import.rs) |
| The node functions (`@arora/node-graph`) | [`arora-node-graph/src/functions.rs`](https://github.com/semio-ai/arora-sdk/blob/main/crates/arora-node-graph/src/functions.rs) |
| The tree runtime | [`arora-node-graph/src/tree_runtime.rs`](https://github.com/semio-ai/arora-sdk/blob/main/crates/arora-node-graph/src/tree_runtime.rs) |
| Animation packaged as an Arora module | [`crates/interop/vizij-animation-module/src/lib.rs`](../../vizij-animation-module/src/lib.rs) |
| The Arora interpreter contract this satisfies | [arora-behavior interpreter workflow](https://github.com/semio-ai/arora-sdk/blob/main/crates/arora-behavior/docs/interpreter-workflow.md) |
| The other reference interpreter (parallels drawn here) | [arora-behavior-tree nodes](https://github.com/semio-ai/arora-sdk/blob/main/crates/arora-behavior-tree/docs/nodes.md) |

Tests worth reading: store round-trip ([`graph_reads_and_writes_the_arora_store`](../src/lib.rs)), in-place graph swap ([`load_swaps_the_graph_in_place`](../src/lib.rs)), external-function dispatch ([`external_function_tests.rs`](https://github.com/semio-ai/arora-sdk/blob/main/crates/arora-node-graph/src/tree_tests/external_function_tests.rs)), and the equivalence of the tree and the reference evaluator on every fixture ([`equivalence.rs`](https://github.com/semio-ai/arora-sdk/blob/main/crates/arora-node-graph/src/tree_tests/equivalence.rs)).
