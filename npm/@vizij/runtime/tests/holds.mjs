// A key held against the device's graph keeps what the page writes, through
// the published wrapper surface (dist/): the passthrough graph writes
// `actuator/y` from `sensor/x` each step, and leaves it alone while it is
// held; released, the graph writes it again at the next step. Holding a key
// the graph does not write changes nothing.
import assert from "node:assert/strict";
import { startRuntime } from "../dist/runtime/src/index.js";

const runtime = await startRuntime(); // built-in passthrough graph: sensor/x -> actuator/y
const y = () => runtime.readValues(["actuator/y"])["actuator/y"];

runtime.setValue("sensor/x", { f32: 0.5 });
runtime.step(16);
assert.deepEqual(y(), { f32: 0.5 });

runtime.holdKeys(["actuator/y", "nobody/writes"]);
runtime.setValue("actuator/y", { f32: 0.75 });
for (const x of [0.5, 0.25]) {
  runtime.setValue("sensor/x", { f32: x });
  runtime.step(16);
  assert.deepEqual(y(), { f32: 0.75 }, "a held key keeps the page's value");
}
assert.deepEqual(
  runtime.readValues(["nobody/writes"]),
  { "nobody/writes": null },
  "a held key nothing writes stays unwritten",
);

runtime.releaseKeys(["actuator/y"]);
runtime.step(16);
assert.deepEqual(y(), { f32: 0.25 }, "released, the graph writes it again");

runtime.holdKeys(["actuator/y"]);
runtime.setValue("actuator/y", { f32: 0.125 });
runtime.step(16);
assert.deepEqual(y(), { f32: 0.125 });
runtime.releaseKeys();
runtime.step(16);
assert.deepEqual(y(), { f32: 0.25 }, "releaseKeys() releases every hold");

runtime.dispose();
console.log("@vizij/runtime holds: ok");
