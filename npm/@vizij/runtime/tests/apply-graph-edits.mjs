// applyGraphEdits (VIZ-79) patches the running graph in place, through the
// published wrapper surface (dist/), exactly as a browser consumer uses it.
import assert from "node:assert/strict";
import { startRuntime } from "../dist/runtime/src/index.js";

// input(sensor/x) -> output(actuator/y): the sink mirrors the sensor.
const runtime = await startRuntime({
  nodes: [
    { id: "in", type: "input", params: { path: "sensor/x", value: { float: 0 } } },
    { id: "out", type: "output", params: { path: "actuator/y" } },
  ],
  edges: [{ from: { node_id: "in" }, to: { node_id: "out", input: "in" } }],
});

runtime.setValue("sensor/x", { f32: 0.25 });
runtime.step(16);
assert.deepEqual(
  runtime.readValues(["actuator/y"]),
  { "actuator/y": { f32: 0.25 } },
  "the sink mirrors the sensor before the edit",
);

// Edit: insert a constant `k = 0.5` and rewire the sink to it. `out` is upserted,
// so its incident edge rides along (the diff contract) — the old `in -> out`
// edge is dropped, `k -> out` replaces it.
await runtime.applyGraphEdits({
  upsert_nodes: [
    { id: "k", type: "constant", params: { value: { f32: 0.5 } } },
    { id: "out", type: "output", params: { path: "actuator/y" } },
  ],
  upsert_edges: [
    { from: { node_id: "k", output: "out" }, to: { node_id: "out", input: "in" } },
  ],
});

// The running graph was patched: the sink now writes the constant, not the
// sensor — no whole-graph reload, and the store carried across the edit.
runtime.setValue("sensor/x", { f32: 0.75 });
runtime.step(16);
assert.deepEqual(
  runtime.readValues(["actuator/y"]),
  { "actuator/y": { f32: 0.5 } },
  "after the edit the sink writes the inserted constant",
);
assert.deepEqual(
  runtime.readValues(["sensor/x"]),
  { "sensor/x": { f32: 0.75 } },
  "the store carried across the edit",
);

runtime.dispose();
console.log("@vizij/runtime applyGraphEdits: ok");
process.exit(0);
