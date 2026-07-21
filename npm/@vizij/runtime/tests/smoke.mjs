// Values flow JS → device store → arora tick → store → JS, through the
// published wrapper surface (dist/), exactly as a browser consumer uses it.
import assert from "node:assert/strict";
import { startDevice } from "../dist/runtime/src/index.js";

const device = await startDevice(); // built-in passthrough graph: sensor/x -> actuator/y

// The first drain is the store's whole current state (empty at boot).
assert.deepEqual(device.drainChanges(), {}, "opening state of an empty store");

device.setValue("sensor/x", { f32: 0.5 });
device.step(16);
assert.deepEqual(device.readValues(["actuator/y"]), { "actuator/y": { f32: 0.5 } });

device.writeValues({ "sensor/x": { f32: 0.25 } });
device.step(16);
assert.deepEqual(device.readValues(["actuator/y"]), { "actuator/y": { f32: 0.25 } });

const changes = device.drainChanges();
assert.deepEqual(changes["actuator/y"], { f32: 0.25 }, "change feed saw the graph write");

const snapshot = device.snapshot();
assert.deepEqual(snapshot["sensor/x"], { f32: 0.25 });

// In-place graph swap (VIZ-57): the device and its store survive.
await device.loadGraph({
  nodes: [
    { id: "in", type: "input", params: { path: "sensor/b", value: { float: 0 } } },
    { id: "out", type: "output", params: { path: "actuator/b" } },
  ],
  edges: [{ from: { node_id: "in" }, to: { node_id: "out", input: "in" } }],
});
device.setValue("sensor/b", { f32: 0.75 });
device.step(16);
assert.deepEqual(device.readValues(["actuator/b"]), { "actuator/b": { f32: 0.75 } });
assert.deepEqual(
  device.readValues(["sensor/x"]),
  { "sensor/x": { f32: 0.25 } },
  "the store carried across the swap",
);

// run(): the device paces itself; the store surface stays live.
assert.equal(device.running, false);
const rejected = new Promise((resolve) => {
  device.run(5).catch(resolve);
});
assert.equal(device.running, true);
let stepError = null;
try {
  device.step(16);
} catch (err) {
  stepError = err;
}
assert.match(String(stepError), /run\(\) drives the device/, "step is gone under run()");
device.setValue("sensor/b", { f32: 0.5 });
await new Promise((resolve) => setTimeout(resolve, 50));
assert.deepEqual(
  device.readValues(["actuator/b"]),
  { "actuator/b": { f32: 0.5 } },
  "the self-paced loop ticked the graph",
);
void rejected; // only ever rejects — when stepping fails, which this test never triggers

device.dispose();
console.log("@vizij/runtime smoke: ok");
process.exit(0); // the run() loop never returns; don't wait on its timers
