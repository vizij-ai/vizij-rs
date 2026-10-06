// Programs are runs of the device's interpreter, driven the way any client of
// the device drives them: `run_behavior` invoked by name, the runs listed off
// the store, edited in place with the interpreter's EDIT, halted, and
// returned to rest by the rest module's `reset_keys` only when the page says
// so — through the published wrapper surface (dist/), on a device with no
// Vizij.
import assert from "node:assert/strict";
import {
  startRuntime,
  behaviorValue,
  runEdits,
  runStatus,
  BEHAVIOR_RUNS,
} from "../dist/runtime/src/index.js";

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
/** Step the device until `pending` settles: an operation applies at the
 * next step, and an invoke reads the method's signature on one step and
 * applies its call on the next. */
const stepped = async (pending) => {
  let done = false;
  pending.then(
    () => (done = true),
    () => (done = true),
  );
  for (let i = 0; i < 8 && !done; i++) {
    runtime.step(100);
    await new Promise((resolve) => setTimeout(resolve, 0));
  }
  return pending;
};
/** Run `graph` under `name`, as any client does. */
const spawn = async (graph, name) =>
  stepped(runtime.invoke("run_behavior", { name, behavior: await behaviorValue(graph) }));
/** The runs the store holds, as any client reads them: `[name, status, id]`. */
const runs = async () => {
  const names = (await stepped(runtime.listKeys(BEHAVIOR_RUNS)))
    .map((key) => key.path)
    .filter((path) => path.endsWith("/name"));
  const values = runtime.readValues(names.flatMap((path) => [path, path.replace(/name$/, "status")]));
  return names
    .map((path) => [
      number(values[path]),
      runStatus(values[path.replace(/name$/, "status")]),
      path.slice(BEHAVIOR_RUNS.length, -"/name".length),
    ])
    .sort();
};
/** The keys `graph`'s output nodes write: what a page returns to rest. */
const outputs = (graph) =>
  graph.nodes.filter((node) => node.type === "output").map((node) => node.params.path);

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

// The interpreter describes run_behavior: any client finds it by name.
const methods = await stepped(runtime.describeMethods("run_behavior"));
assert.equal(methods.length, 1, JSON.stringify(methods));

// Two programs run at once, beside the graph; one drives the graph's input.
const drive = writes("sensor/x", 1);
const driver = await spawn(drive, "driver");
const mover = await spawn(damped("program/moving"), "mover");
runtime.step(100);
close(read("actuator/y"), 1, "the driver writes the graph's input");
assert.deepEqual(
  (await runs()).map(([name, status]) => [name, status]),
  [
    ["driver", "running"],
    ["mover", "running"],
  ],
  "any client reads the runs off the store",
);
assert.equal((await runs())[0][2], driver.run);

// The mover moves: its damp heads for the target.
runtime.setValue("in/target", 1);
for (let i = 0; i < 3; i++) runtime.step(100);
const moving = number(read("program/moving"));
assert.ok(moving > 0.1 && moving < 0.9, `on its way: ${moving}`);

// Edited live: the damp it keeps keeps its state, the output it adds runs.
const edits = await runEdits(mover.run, damped("program/moving"), damped("program/moving", "program/copy"));
await stepped(runtime.applyGraphEdits(edits));
const next = number(read("program/moving"));
assert.ok(next > moving && next < 0.9, `the kept damp carried on: ${moving} -> ${next}`);
close(read("program/copy"), next, "the added output runs");

// Halted, the driver ends and its output holds; the mover runs on.
await stepped(runtime.halt(driver.run));
runtime.step(100);
const status = async (name) => (await runs()).find(([n]) => n === name)?.[1];
assert.equal(await status("driver"), "failure");
assert.equal(await status("mover"), "running");
close(read("sensor/x"), 1, "a halted program's output holds");
close(read("actuator/y"), 1, "and the graph reads it");

// Returned to rest only when asked: its outputs go back to where they rest.
assert.deepEqual(outputs(drive), ["sensor/x"]);
await stepped(runtime.invoke("reset_keys", { keys: { strs: outputs(drive) } }));
close(read("sensor/x"), 0, "the driven input is back at rest");
runtime.step(100);
close(read("actuator/y"), 0, "the graph carried it");
close(read("unrelated/kept"), 0.375, "an unrelated key keeps its value");
assert.ok(number(read("program/moving")) > next, "the other program runs on");

// Spawning again is a new run, from fresh node state.
const again = await spawn(drive, "driver");
assert.notEqual(again.run, driver.run);
runtime.step(100);
close(read("sensor/x"), 1, "the new run writes again");
assert.deepEqual(
  (await runs())
    .filter(([name]) => name === "driver")
    .map(([, status]) => status)
    .sort(),
  ["failure", "running"],
  "the ended run and the new one",
);

runtime.dispose();
console.log("@vizij/runtime programs: ok");
