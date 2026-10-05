// Programs are runs of the device's interpreter: spawned beside its graph,
// found in its store by name, edited in place, halted, and returned to rest
// only when the page says so — through the published wrapper surface
// (dist/), on a device with no Vizij.
import assert from "node:assert/strict";
import { startRuntime } from "../dist/runtime/src/index.js";

const number = (value) => (value ? Object.values(value)[0] : value);
const close = (actual, expected, message) =>
  assert.ok(Math.abs(number(actual) - expected) < 1e-6, `${message}: ${JSON.stringify(actual)}`);

// The device's graph: input(sensor/x, rests at 0) -> output(actuator/y).
const runtime = await startRuntime({
  nodes: [
    { id: "in", type: "input", params: { path: "sensor/x", value: { float: 0 } } },
    { id: "out", type: "output", params: { path: "actuator/y" } },
  ],
  edges: [{ from: { node_id: "in" }, to: { node_id: "out", input: "in" } }],
});
const read = (path) => runtime.readValues([path])[path];
/** Step the device once, then settle what was waiting on that step. */
const stepped = async (pending) => {
  runtime.step(100);
  return pending;
};
const status = (name) => runtime.programRuns().find((run) => run.name === name)?.status;

/** A program writing `value` to `path`. */
const writes = (path, value) => ({
  nodes: [
    { id: "k", type: "constant", params: { value: { float: value } } },
    { id: "out", type: "output", params: { path } },
  ],
  edges: [{ from: { node_id: "k" }, to: { node_id: "out", input: "in" } }],
});
/** A program damping `in/target` (from 0) onto each of `outputs`. */
const damped = (...outputs) => ({
  nodes: [
    { id: "target", type: "input", params: { path: "in/target", value: { float: 0 } } },
    { id: "damp", type: "damp", params: { half_life: 0.5 } },
    ...outputs.map((path, i) => ({ id: `out${i}`, type: "output", params: { path } })),
  ],
  edges: [
    { from: { node_id: "target" }, to: { node_id: "damp", input: "in" } },
    ...outputs.map((_, i) => ({ from: { node_id: "damp" }, to: { node_id: `out${i}`, input: "in" } })),
  ],
});

runtime.setValue("unrelated/kept", 0.375);
runtime.step(16);

// A page's program needs a name; a bundle id must name a program.
await assert.rejects(runtime.spawnProgram(writes("program/a", 1)), /needs a name/);
await assert.rejects(runtime.spawnProgram("nope"), /no program "nope"/);

// Two programs run at once, beside the graph; one drives the graph's input.
const drive = writes("sensor/x", 1);
const driver = await stepped(runtime.spawnProgram(drive, "driver"));
const mover = await stepped(runtime.spawnProgram(damped("program/moving"), "mover"));
runtime.step(100);
close(read("actuator/y"), 1, "the driver writes the graph's input");
assert.deepEqual(
  runtime.programRuns().map(({ name, status }) => [name, status]),
  [
    ["driver", "running"],
    ["mover", "running"],
  ],
  "any client reads the runs off the store",
);
assert.equal(runtime.programRuns()[0].handle.id, driver.id);

// The mover moves: its damp heads for the target.
runtime.setValue("in/target", 1);
for (let i = 0; i < 3; i++) runtime.step(100);
const moving = number(read("program/moving"));
assert.ok(moving > 0.1 && moving < 0.9, `on its way: ${moving}`);

// Edited live: the damp it keeps keeps its state, the output it adds runs.
await stepped(runtime.editProgram(mover, damped("program/moving"), damped("program/moving", "program/copy")));
const next = number(read("program/moving"));
assert.ok(next > moving && next < 0.9, `the kept damp carried on: ${moving} -> ${next}`);
close(read("program/copy"), next, "the added output runs");

// Halted, the driver ends and its output holds; the mover runs on.
await stepped(runtime.halt(driver));
runtime.step(100);
assert.equal(status("driver"), "failure");
assert.equal(status("mover"), "running");
close(read("sensor/x"), 1, "a halted program's output holds");
close(read("actuator/y"), 1, "and the graph reads it");

// Returned to rest only when asked: its outputs go back to where they rest.
assert.deepEqual(runtime.programOutputs(drive), ["sensor/x"]);
await runtime.reset(runtime.programOutputs(drive));
close(read("sensor/x"), 0, "the driven input is back at rest");
runtime.step(100);
close(read("actuator/y"), 0, "the graph carried it");
close(read("unrelated/kept"), 0.375, "an unrelated key keeps its value");
assert.ok(number(read("program/moving")) > next, "the other program runs on");

// Spawning again is a new run, from fresh node state.
const again = await stepped(runtime.spawnProgram(drive, "driver"));
assert.notEqual(again.id, driver.id);
runtime.step(100);
close(read("sensor/x"), 1, "the new run writes again");
assert.deepEqual(
  runtime.programRuns().filter((run) => run.name === "driver").map((run) => run.status).sort(),
  ["failure", "running"],
  "the ended run and the new one",
);

runtime.dispose();
console.log("@vizij/runtime programs: ok");
