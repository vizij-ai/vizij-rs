# vizij-graph-core

Vizij's node graph: its format (`GraphSpec`, its JSON form), its editor
registry, and its evaluation as the detached nodes of an
[`arora-behavior-tree`](https://github.com/semio-ai/arora-sdk/tree/main/crates/arora-behavior-tree)
([proposal](https://github.com/semio-ai/arora-sdk/blob/main/docs/proposal-behavior-languages.md)).
A graph's nodes are calls of
[`arora-dataflow-nodes`](https://github.com/semio-ai/arora-sdk/tree/main/crates/arora-dataflow-nodes)'
functions, the node vocabulary of three Arora modules: `@arora/dataflow` for
the general kinds, `@arora/geometry` for `vec3cross`, and
`@arora/kinematics` for `inversekinematics` and the URDF nodes. This crate
re-exports its modules (`functions`, `kind`, `params`, `ports`, `select`,
`task`, `vocab`, and `eval` with the per-node code). The crate holds:

- **the graph format** (`types`): `GraphSpec`, `NodeSpec`, `NodeParams`, and
  `NodeType`, the graph JSON's node kinds, which are the vocabulary's
  (`kind::Kind`) and `input` and `output`; the editor registry (`schema`),
  and the topological order (`topo`);
- **the graph format's own functions** (`vizij_functions`), the module
  `@vizij/graph`: `input`, reading a store key and fitting its value to the
  node's declared shape, `output`, checking its value against its own, and
  `check`, the check of a node's declared output shapes;
- **the importer** (`import`): a graph translated node for node into a tree
  and its detached nodes, as `BehaviorTree::try_from_groot_xml` reads a Groot
  file;
- **`TreeRuntime`** (`tree_runtime`): a graph run on the tree, for a host
  such as an editor or Vizij's `ProcessingGraph`.

## On the behavior tree

```rust
use vizij_graph_core::import::import_json;
use vizij_graph_core::tree_runtime::TreeRuntime;
use vizij_api_core::{TypedPath, Value};

// A graph's JSON as Vizij writes it: a spec, or a document holding one.
let (spec, _) = import_json(include_str!("../../../fixtures/node_graphs/sine-driver.json"), &Default::default())?;
let mut rt = TreeRuntime::default();
rt.load(&spec)?;
rt.dt = 1.0 / 60.0;
rt.t += rt.dt;
rt.set_input(TypedPath::parse("driver/time.seconds")?, Value::F32(0.5), None);
rt.evaluate()?;
for write in rt.writes.iter() {
    println!("{} = {:?}", write.path, write.value);
}
```

How a graph translates:

| In the graph | On the tree |
|---|---|
| a node | a detached call of its kind's function: it runs when what it outputs is read, once per tick, and only when its arguments changed |
| an input port | an optional parameter of the port's name; an edge reads the source's port: `@node`, or a field of the structure a node with ports beyond `out` returns |
| an edge's selector | a `select` node between the two |
| an inline default | a literal argument |
| a node parameter | a literal argument, `param.<field>` |
| hidden state (`spring`, `damp`, `slew`, IK) | the mutable parameter `state`, as a value: a smoother's buffers in its input's shape, an IK node's joints and last solve |
| `time`, `oscillator`, the smoothers' step | the parameters `t` and `dt`, read from the variables `ImportOptions` names (`arora/time` and `arora/dt` by default) |
| an `input` node | a call to `@vizij/graph`'s `input`, reading the store key its path names and fitting its value to its declared shape |
| an `output` node with a path | a call to `@vizij/graph`'s `output`, checking its value against its declared shape; a sink: its return value bound to the store key of its path, written after the tree ticks; the later-listed writer of a key wins |
| an `output` node with no path (a keyed batch) | listed in `ImportedGraph::keyed_outputs`, for its host to apply |
| a node's declared output shapes | a call to `@vizij/graph`'s `check` after it, which its readers read in its place; an `input` or `output` node checks its own `out`, and one declaring the shape of another port is refused |
| `taskrun`, `spawn` | leaves under the root `PARALLEL`, ticked at every tick, as they act at each evaluation |
| `externalfunction` | placed by its function's description (`placement`): left out when the function returns nothing and writes nothing back; otherwise a leaf ticked at every tick, as Vizij calls it, since a module's function may keep state its arguments do not show (the animation module's `step(dt)`) |

How it evaluates:

- **What runs.** A node runs when something reads it and its arguments
  changed. `Evaluation::Every` observes every node, as an editor does, and
  `Evaluation::Demanded` runs what the outputs read, as a device does. A
  node nothing reads does not fail a demanded evaluation.
- **Order.** A node runs after every node it reads from, and otherwise in
  the order the spec lists it, so of two outputs writing one path the
  later-listed wins.
- **Inputs.** A value staged with `TreeRuntime::set_input` is seen by the
  next evaluation alone; an input staged nothing reads its default.
- **Shapes.** Ports carry values; a port's shape is its declared output
  shape, the shape staged with an input, or the one its value infers. A
  node's declared output shapes are checked where it is read: a value that
  does not fit fails the evaluation, as a device runs it or an editor.
- **Variadic ports.** A function declares `VARIADIC_PORTS` (64) ports of its
  kind's variadic group; the importer refuses a graph wiring one past them.
- **Edits.** A node's parameter value is set in place on the loaded tree
  (`TreeRuntime::set_params`). An edit that changes what the tree is (a
  node's ports, the path an input or output binds, a node added or rewired)
  takes a new load of the whole graph, which keeps every node's state.
- **Loads.** `TreeRuntime::evaluate_spec` loads a spec when it is not the
  graph loaded. `GraphSpec::with_cache` bumps a spec's `version`, for a host
  that reloads on a new version.

The stress example times loading and stepping, and counts each step's calls:

```bash
cargo run --release -p vizij-graph-core --example stress -- chain 10000
cargo run --release -p vizij-graph-core --example stress -- file graph.json
```

---

## Nodes

The node library covers math, logic, vector operations, time and transitions
(`Spring`, `Damp`, `Slew`), blending, ranges (including multi-segment
piecewise remaps), records, calls to other modules, and robotics IK/FK behind
the `urdf_ik` feature. Edges can select part of a value (`field`, `index`),
and `Output` nodes write a `WriteBatch` the host applies.

## Installation

```bash
cargo add vizij-graph-core
```

Features:

| Feature   | Default | Description                                           |
|-----------|---------|-------------------------------------------------------|
| `urdf_ik` | ✔       | Enables URDF chain parsing + IK/FK nodes (depends on `k` and `urdf-rs`). |

Disable defaults with `--no-default-features` if you want a minimal build.

---

## Usage

1. **Load or construct a `GraphSpec`.** `import::import_json` reads a graph's JSON as Vizij writes it; build specs programmatically in tests.
   - `spec.validate()` refuses a graph that would fail every evaluation — an edge to a missing node, two edges into one input, two nodes sharing an id, a cycle, or a node leaving one of its `schema::required_inputs` with neither an edge nor an inline default — naming the node at fault. A host checks it before installing a graph.
2. **Create and reuse a `TreeRuntime`**, and `load` the spec. It keeps every node's state across evaluations and loads.
3. **Set the clock and stage host inputs** before each evaluation:
   ```rust
   rt.t += rt.dt;
   rt.set_input(path, value, declared_shape);
   ```
   - Declared shapes help coerce numeric data or produce deterministically “null-of-shape” placeholders when staging fails.
4. **Evaluate**: `rt.evaluate()?`.
5. **Consume results.**
   - `rt.writes` – `WriteBatch` of `WriteOp { path, value, shape }` emitted by `Output` nodes.
   - `rt.outputs` – map of node → output key → `PortValue { value, shape }`, for every observed node, unless `set_collect_outputs(false)`.
6. **Integrate with hosts.** Apply writes to your own blackboard, engine, or animation runtime as needed.

---

## Key Concepts

### The clock

- `TreeRuntime::t` and `dt` are what the next evaluation reads, in seconds; a host such as the Arora behavior interpreter (`vizij-arora-behavior`) sets them each frame.
- A `dt` of zero is a step with no time in it, and moves nothing that time drives: `Time` and `Oscillator` read the unchanged `t`, `Spring` records its new target and keeps its position and velocity, and `Damp` and `Slew` keep their value (so does a negative or non-finite `dt`). An evaluation at `dt = 0` therefore carries a changed input through every node that does not depend on time — a paused scene showing an edit — and a smoother it reaches takes it as its new target, which shows once time passes; the next step with time in it moves on as if the zero step had not been. An IK node answers a request equal to its last one with its last solution, so it holds too.

### Selectors

- Edges can include selectors composed of field/index segments.
- Field selectors pick a record's field, a transform's part (`translation` or `position`, `rotation`, `scale`) or an enumeration's payload (`["state", "translation"]`). Index selectors pick an element of a numeric vector, an item of a sequence, or a structure's field in declaration order, whatever the structure's type (`["translation", 1]`).
- Example: `{"selector": ["translation", 1]}` on a `Transform` output lifts the Y component; chaining `["record_field", 2, "nested_field"]` is also supported.
- A selection reads the value alone (`select::select`). An invalid path fails with a descriptive error (“selector index 5 out of bounds for vec3”) instead of guessing. Catch these errors during development to keep graph definitions deterministic.

### Shapes & Values

- Values are Arora's `Value` (`vizij_api_core::Value`, re-exported). The geometry types (vec2, vec3, vec4, quat, transform) are `arora-geometry`'s, and only the geometry and kinematics nodes and edge selection name them. `vizij_api_core::value` re-exports them with Vizij's colour, records and enumerations.
- The numeric nodes read a value's form off the value itself, so they need no list of value types. A value whose leaves are numbers flattens: a structure field by field in declaration order, a record in key-name order, a sequence item by item. The result has the same form, with the same structure ids, field ids and keys. A structure flattens when its number fields are floats, and keeps their types (an `F64` field stays `F64`), though every number computes in `f32`, as everywhere in a graph. A structure with an integer field has no numeric form, as one with a text field: rebuilding it between a node's steps would round it. A number on its own, in a record or in a sequence, of any width, is an `F32`. Two operands combine when they have the same form, or when one is a number, which applies to every component of the other: a vec4, a quaternion and a colour each hold four numbers, and do not combine.
- Shapes (`ShapeId`) describe numeric layouts and support inference. Sequences travel as one wire kind, so declared `Array`/`List`/`Tuple` shapes carry the distinction.
- Declared shapes on node outputs guard against schema drift and provide better error messages.

### Parameter updates

- `TreeRuntime::set_params` sets a node's parameters in place on the loaded tree, the edit an editor's slider makes:
  ```rust
  let mut params = spec.nodes[0].params.clone();
  params.value = Some(vizij_api_core::value::float(0.5));
  let in_place = rt.set_params("gain", &params)?;
  ```
  It returns `false`, changing nothing, for an edit only a load can make; the host then loads the edited spec.
- Shapes are checked during evaluation; incompatible updates surface as detailed `Err(String)` messages (`"set_param: node 'gain' key 'value' expects Float"` in wasm bindings).

### Outputs

- `TreeRuntime::outputs` holds, after each evaluation, every observed node's ports by node id and port name. Each `PortValue` carries both the `Value` and its `Shape`:
  ```rust
  if let Some(port) = rt.outputs.get("oscillator").and_then(|ports| ports.get("out")) {
      println!("shape: {:?}, value: {:?}", port.shape.id, port.value);
  }
  ```

### External Writes

- `Output` nodes enqueue writes automatically when `params.path` is set.
- Writes include optional shapes so hosts can validate or coerce downstream.
- Use hosts like the Arora behavior interpreter (`vizij-arora-behavior`) or your own glue to apply them.

### URDF Feature

- Enable `urdf_ik` to pull in robotics helpers (`UrdfIkPosition`, `UrdfIkPose`, `UrdfFk` nodes).
- The WASM build ships with the feature enabled; native builds can opt out to reduce dependencies.

---

## Development & Testing

Run the crate tests (unit + integration):

```bash
cargo test -p vizij-graph-core
```

Test the robotics nodes explicitly:

```bash
cargo test -p vizij-graph-core --features urdf_ik
```

The benchmark loads and steps chains, a mixed "kitchen" graph and a fixture on the tree:

```bash
cargo bench -p vizij-graph-core
```

The fixture tests run every graph in the repository's `fixtures/node_graphs/` evaluating every node and what its outputs read, and again with its parameters edited in place.

---

## Related Packages

- [`arora-dataflow-nodes`](https://github.com/semio-ai/arora-sdk/tree/main/crates/arora-dataflow-nodes): the node vocabulary, as the functions of Arora modules.
- [`vizij-api-core`](../../api/vizij-api-core/README.md): Vizij's value vocabulary, shapes, typed paths and write batches.
- [`vizij-graph-wasm`](https://github.com/vizij-ai/vizij-rs/blob/main/crates/node-graph/vizij-graph-wasm/README.md): wasm-bindgen binding that exposes JSON-friendly APIs plus normalization helpers.
- [`@vizij/node-graph`](https://github.com/vizij-ai/vizij-rs/blob/main/npm/@vizij/node-graph/README.md): npm wrapper around the wasm build with ABI guards and utilities.


