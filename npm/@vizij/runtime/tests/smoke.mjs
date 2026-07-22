// Values flow JS → runtime store → arora tick → store → JS, through the
// published wrapper surface (dist/), exactly as a browser consumer uses it.
import assert from "node:assert/strict";
import { startRuntime } from "../dist/runtime/src/index.js";

const runtime = await startRuntime(); // built-in passthrough graph: sensor/x -> actuator/y

// The first drain is the store's whole current state (empty at boot).
assert.deepEqual(runtime.drainChanges(), {}, "opening state of an empty store");

runtime.setValue("sensor/x", { f32: 0.5 });
runtime.step(16);
assert.deepEqual(runtime.readValues(["actuator/y"]), { "actuator/y": { f32: 0.5 } });

runtime.writeValues({ "sensor/x": { f32: 0.25 } });
runtime.step(16);
assert.deepEqual(runtime.readValues(["actuator/y"]), { "actuator/y": { f32: 0.25 } });

const changes = runtime.drainChanges();
assert.deepEqual(changes["actuator/y"], { f32: 0.25 }, "change feed saw the graph write");

const snapshot = runtime.snapshot();
assert.deepEqual(snapshot["sensor/x"], { f32: 0.25 });

// In-place graph swap (VIZ-57): the runtime and its store survive.
await runtime.loadGraph({
  nodes: [
    { id: "in", type: "input", params: { path: "sensor/b", value: { float: 0 } } },
    { id: "out", type: "output", params: { path: "actuator/b" } },
  ],
  edges: [{ from: { node_id: "in" }, to: { node_id: "out", input: "in" } }],
});
runtime.setValue("sensor/b", { f32: 0.75 });
runtime.step(16);
assert.deepEqual(runtime.readValues(["actuator/b"]), { "actuator/b": { f32: 0.75 } });
assert.deepEqual(
  runtime.readValues(["sensor/x"]),
  { "sensor/x": { f32: 0.25 } },
  "the store carried across the swap",
);

// run(): the runtime paces itself; the store surface stays live.
assert.equal(runtime.running, false);
const rejected = new Promise((resolve) => {
  runtime.run(5).catch(resolve);
});
assert.equal(runtime.running, true);
let stepError = null;
try {
  runtime.step(16);
} catch (err) {
  stepError = err;
}
assert.match(String(stepError), /run\(\) drives the device/, "step is gone under run()");
runtime.setValue("sensor/b", { f32: 0.5 });
await new Promise((resolve) => setTimeout(resolve, 50));
assert.deepEqual(
  runtime.readValues(["actuator/b"]),
  { "actuator/b": { f32: 0.5 } },
  "the self-paced loop ticked the graph",
);
void rejected; // rejects only if the runtime itself fails, which this test never triggers

// Behavior errors stand, they don't stop the loop (arora 9.1): swap in a
// graph whose input has no default and no staged value — ticks fail, the
// error is observable, and the runtime keeps running.
assert.equal(runtime.behaviorError, undefined, "healthy behavior reads undefined");
const failed = runtime.behaviorErrorChanged();
await runtime.loadGraph({
  nodes: [
    { id: "in", type: "input", params: { path: "sensor/missing" } },
    { id: "out", type: "output", params: { path: "actuator/c" } },
  ],
  edges: [{ from: { node_id: "in" }, to: { node_id: "out", input: "in" } }],
});
assert.match(
  String(await failed),
  /missing staged value/,
  "the failing tick's message stands as the behavior error",
);
const recovered = runtime.behaviorErrorChanged();
runtime.setValue("sensor/missing", { f32: 0.125 });
assert.equal(await recovered, undefined, "a recovering tick clears the error");
await new Promise((resolve) => setTimeout(resolve, 30));
assert.deepEqual(
  runtime.readValues(["actuator/c"]),
  { "actuator/c": { f32: 0.125 } },
  "the run loop kept ticking through the failure",
);

runtime.dispose();
console.log("@vizij/runtime smoke: ok");
process.exit(0); // the run() loop never returns; don't wait on its timers
