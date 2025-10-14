# Tutorial: Getting Productive with `@vizij/orchestrator-wasm`

This guide walks you from zero to confident user of Vizij’s orchestrator bindings. You will
initialize the wasm module, register graphs and animations, merge multiple graph specs into a single
controller, drive the blackboard, and inspect outputs. The tutorial assumes TypeScript, but the API
works in plain JavaScript as well.

---

## 1. Installation & Environment

```bash
npm install @vizij/orchestrator-wasm
# or pnpm add @vizij/orchestrator-wasm
```

The package ships pre-built wasm binaries (`pkg/`) and ESM entry points (`dist/`). No Rust toolchain
is required at runtime.

---

## 2. Basic Setup

Always initialize the wasm module before constructing an orchestrator:

```ts
import { init, createOrchestrator, abi_version } from "@vizij/orchestrator-wasm";

await init();
console.log("ABI", abi_version()); // 2

const orchestrator = await createOrchestrator({ schedule: "SinglePass" });
```

`schedule` accepts `"SinglePass"`, `"TwoPass"`, or `"RateDecoupled"` (currently an alias). If you
need deterministic ordering with animation feedback, choose `"TwoPass"`.

---

## 3. Registering Graphs

Graph specs mirror the JSON consumed by `vizij-graph-core`. Shorthand such as `inputs`, legacy value
shapes, and string `spec` are normalized automatically.

```ts
const gainGraph = {
  spec: {
    nodes: [
      { id: "input", type: "input", params: { path: "demo/gain" } },
      { id: "offset", type: "constant", params: { value: 0.5 } },
      { id: "mix", type: "add" },
      { id: "publish", type: "output", params: { path: "demo/output/value" } },
    ],
    links: [
      { from: { node_id: "input" }, to: { node_id: "mix", input: "lhs" } },
      { from: { node_id: "offset" }, to: { node_id: "mix", input: "rhs" } },
      { from: { node_id: "mix" }, to: { node_id: "publish", input: "in" } },
    ],
  },
  subs: {
    inputs: ["demo/gain"],
    outputs: ["demo/output/value"],
  },
};

const gainGraphId = orchestrator.registerGraph(gainGraph);
console.log("Graph registered:", gainGraphId);
```

Subscriptions constrain which blackboard paths are staged (`inputs`) and exposed (`outputs`).

---

## 4. Animation Controllers

```ts
const animId = orchestrator.registerAnimation({
  setup: {
    animation: await loadAnimationJSON(), // supply a vizij animation payload
    player: { name: "demo-player", loop_mode: "loop" },
  },
});
```

Animations consume blackboard commands with the `anim/player/<id>/` convention. Use
`orchestrator.prebind(resolver)` to map typed paths to animation targets:

```ts
orchestrator.prebind((path) => {
  if (path === "robot/arm.joint") return "arm_joint_node";
  return null;
});
```

---

## 5. Merging Graphs

`registerMergedGraph` rewires multiple graph specs into a single controller, namespacing node IDs
and replacing blackboard hops with direct links when possible:

```ts
const mergedId = orchestrator.registerMergedGraph({
  graphs: [
    {
      spec: {
        nodes: [
          { id: "source", type: "constant", params: { value: 1 } },
          { id: "publish", type: "output", params: { path: "shared/value" } },
        ],
        links: [{ from: { node_id: "source" }, to: { node_id: "publish", input: "in" } }],
      },
      subs: { outputs: ["shared/value"] },
    },
    {
      spec: {
        nodes: [
          { id: "input", type: "input", params: { path: "shared/value" } },
          {
            id: "double",
            type: "multiply",
            input_defaults: { rhs: { value: 2 } },
          },
          { id: "publish", type: "output", params: { path: "shared/doubled" } },
        ],
        links: [
          { from: { node_id: "input" }, to: { node_id: "double", input: "lhs" } },
          { from: { node_id: "double" }, to: { node_id: "publish", input: "in" } },
        ],
      },
      subs: { inputs: ["shared/value"], outputs: ["shared/doubled"] },
    },
  ],
  strategy: {
    outputs: "namespace",
    intermediate: "blend",
  },
});

console.log("Merged graph:", mergedId);
```

`strategy` lets you decide how to resolve overlapping paths:

- `"error"` (default) preserves the legacy behaviour and aborts the merge.
- `"blend"` inserts a `default-blend` node (plus equal weights) so downstream graphs receive a
  single averaged value.
- `"namespace"` rewrites final output paths to `graphId/original/path` so parallel values stay
  separate.

`outputs` applies to host-facing writes; `intermediate` covers paths that another merged graph
consumes. Namespace is only supported for final outputs—intermediate overlaps should be blended or
left as errors.

---

## 6. Driving the Blackboard

Use `setInput(path, value, shape?)` to stage host values. Values accept either canonical `{ type,
data }` envelopes or ergonomic JSON (`{ float: 1.0 }`, numbers, vectors).

```ts
orchestrator.setInput("demo/gain", { float: 2.0 });

const frame = orchestrator.step(1 / 60);
console.log("Merged writes:", frame.merged_writes);
frame.conflicts.forEach((log) => {
  console.warn("Conflict on", log.path, "previous:", log.previous_value, "new:", log.new_value);
});
```

`frame.merged_writes` is an ordered array of `{ path, value, shape? }` suitable for UI updates or
network replication.

---

## 7. Inspecting Controllers

```ts
const { graphs, anims } = orchestrator.listControllers();
console.log("Graphs:", graphs);
console.log("Animations:", anims);

const removed = orchestrator.removeGraph(graphs[0]);
console.log("Removed graph:", removed);
```

---

## 8. TypeScript Tips

- Import types from `@vizij/orchestrator-wasm/src/types` if you need advanced annotations:

  ```ts
  import type {
    GraphRegistrationConfig,
    GraphSubscriptions,
    MergedGraphRegistrationConfig,
    OrchestratorAPI,
  } from "@vizij/orchestrator-wasm";
  ```

- `normalizeGraphSpec` is a convenience wrapper if you want to pre-process specs before persisting:

  ```ts
  const normalized = await orchestrator.normalizeGraphSpec(specObject);
  localStorage.setItem("graph", JSON.stringify(normalized));
  ```

- Generated TypeScript definitions live in `dist/src/index.d.ts`.

---

## 9. Testing Locally

Smoke tests live next to the package:

```bash
pnpm --dir npm/@vizij/orchestrator-wasm run build
pnpm --dir npm/@vizij/orchestrator-wasm test
```

During development, rebuild the wasm bundle when core Rust changes:

```bash
pnpm run build:wasm:orchestrator   # from repo root (requires wasm-pack + Rust toolchain)
```

---

## 10. Troubleshooting

| Symptom | Resolution |
|---------|------------|
| `registerGraph` throws `graph json parse error` | Ensure `spec` is valid JSON; wrap objects with `{ spec: {...} }`. |
| Multiple graphs target the same output path | Catch the error, rename the graphs or explicitly namespace the output path (e.g., `graphA/shared/...`). |
| `init()` throws when loading wasm | Verify the `pkg/` directory is available (run `pnpm run build:wasm:orchestrator` after Rust changes). |
| No writes appear after `step` | Confirm controllers are registered, inputs are staged, and `step` receives a finite `dt`. |

---

## 11. Where to Go Next

- Explore the Rust tutorial (`crates/orchestrator/vizij-orchestrator-core/tutorial.md`) for deeper
  insight into schedules, blackboard internals, and testing strategies.
- Combine the orchestrator with `@vizij/node-graph-wasm` to author graphs dynamically in the browser.
- Integrate with `@vizij/orchestrator-react` to feed updates directly into React state.

You now have a solid foundation for orchestrating complex Vizij graph + animation flows in
JavaScript environments. Happy building! 🎛️🚀
