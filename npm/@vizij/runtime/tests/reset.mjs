// The rest module's `reset`, invoked by name, returns a device's keys to
// rest: a free input of its graph goes back to its authored default — the
// graph then carries its output there — and a key with no declared rest
// keeps its value.
import assert from "node:assert/strict";
import { startRuntime } from "../dist/runtime/src/index.js";

const number = (value) => (value ? Object.values(value)[0] : value);
const close = (actual, expected, message) =>
  assert.ok(Math.abs(number(actual) - expected) < 1e-6, `${message}: ${JSON.stringify(actual)}`);

const runtime = await startRuntime({
  nodes: [
    { id: "in", type: "input", params: { path: "sensor/level", value: { float: 0.25 } } },
    { id: "out", type: "output", params: { path: "actuator/level" } },
  ],
  edges: [{ from: { node_id: "in" }, to: { node_id: "out", input: "in" } }],
});
const read = (path) => runtime.readValues([path])[path];
/** Step the device until `pending` settles: an invoke reads the method's
 * signature on one step and applies its call on the next. */
const stepped = async (pending) => {
  let done = false;
  pending.then(
    () => (done = true),
    () => (done = true),
  );
  for (let i = 0; i < 8 && !done; i++) {
    runtime.step(16);
    await new Promise((resolve) => setTimeout(resolve, 0));
  }
  return pending;
};

runtime.step(16);
close(read("actuator/level"), 0.25, "the output starts at the input's default");

runtime.setValue("sensor/level", 0.9);
runtime.setValue("scratch/note", 3);
runtime.step(16);
close(read("actuator/level"), 0.9, "the write moved the output");

await stepped(runtime.invoke("reset"));
close(read("sensor/level"), 0.25, "the input is back at its default");
close(read("actuator/level"), 0.25, "the graph carried the output back to rest");
close(read("scratch/note"), 3, "a key with no declared rest keeps its value");

// Under run() the reset lands on the device's own next step.
runtime.setValue("sensor/level", 0.6);
void runtime.run(5).catch(() => {});
await runtime.invoke("reset");
close(read("sensor/level"), 0.25, "reset under run()");
runtime.stop();

console.log("@vizij/runtime reset: ok");
