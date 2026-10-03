// Programs played, paused, stopped and replaced on a running device, through
// the published wrapper surface (dist/), with the store's other values kept
// through every change.
import assert from "node:assert/strict";
import { startRuntime } from "../dist/runtime/src/index.js";

// The base graph: input(sensor/x) -> output(actuator/y).
const runtime = await startRuntime({
  nodes: [
    { id: "in", type: "input", params: { path: "sensor/x", value: { float: 0 } } },
    { id: "out", type: "output", params: { path: "actuator/y" } },
  ],
  edges: [{ from: { node_id: "in" }, to: { node_id: "out", input: "in" } }],
});

/** A program writing `value` to `path`. */
const writes = (path, value) => ({
  nodes: [
    { id: "k", type: "constant", params: { value: { float: value } } },
    { id: "out", type: "output", params: { path } },
  ],
  edges: [{ from: { node_id: "k" }, to: { node_id: "out", input: "in" } }],
});

const read = (path) => runtime.readValues([path])[path];
/** The store's values a program must leave alone, checked after each step. */
const unrelated = () => {
  assert.deepEqual(read("unrelated/kept"), { f32: 0.375 }, "an unrelated key keeps its value");
  assert.deepEqual(read("actuator/y"), { f32: 0.25 }, "the base graph keeps running");
};

runtime.setValue("unrelated/kept", 0.375);
runtime.setValue("sensor/x", 0.25);
runtime.step(16);
unrelated();

// A device knows only the programs it is given.
assert.equal(runtime.programState("a"), undefined);
await assert.rejects(runtime.startProgram("a"), /no program "a"/);

// Defined, a program waits; started, it writes beside the base graph.
await runtime.setProgram("a", writes("program/a", 1));
await runtime.setProgram("b", writes("program/b", 2));
assert.equal(runtime.programState("a"), "stopped");
assert.equal(read("program/a"), null, "a program that does not play writes nothing");
await runtime.startProgram("a");
await runtime.startProgram("b");
assert.equal(runtime.programState("a"), "playing");
assert.equal(runtime.programState("b"), "playing");
runtime.step(16);
assert.deepEqual(read("program/a"), { f32: 1 });
assert.deepEqual(read("program/b"), { f32: 2 });
unrelated();

// Stopped with its outputs reset: the key it wrote is cleared, the other
// program plays on.
await runtime.stopProgram("a", { resetOutputs: true });
assert.equal(runtime.programState("a"), "stopped");
runtime.step(16);
assert.equal(read("program/a"), null, "the stopped program's output is back at rest");
assert.deepEqual(read("program/b"), { f32: 2 });
unrelated();

// Replaced live: the playing program writes its new graph, nothing else moves.
await runtime.setProgram("b", writes("program/b", 3));
assert.equal(runtime.programState("b"), "playing");
runtime.step(16);
assert.deepEqual(read("program/b"), { f32: 3 });
unrelated();

// Paused: out of the graph, its output holding.
await runtime.pauseProgram("b");
assert.equal(runtime.programState("b"), "paused");
runtime.setValue("program/b", 7);
runtime.step(16);
assert.deepEqual(read("program/b"), { f32: 7 }, "a paused program writes nothing");
unrelated();

// Stopped without a reset: its output holds too.
await runtime.stopProgram("b");
runtime.step(16);
assert.deepEqual(read("program/b"), { f32: 7 });

// A stopped program's graph changes without playing it; a graph that does not
// parse changes nothing.
await runtime.setProgram("a", writes("program/a", 5));
assert.equal(runtime.programState("a"), "stopped");
await runtime.startProgram("a");
runtime.step(16);
assert.deepEqual(read("program/a"), { f32: 5 });
await assert.rejects(runtime.setProgram("c", "{ not json"), /not JSON/);
assert.equal(runtime.programState("c"), undefined);

// A whole-graph load leaves out the programs the loaded graph does not hold.
await runtime.loadGraph({
  nodes: [
    { id: "in", type: "input", params: { path: "sensor/x", value: { float: 0 } } },
    { id: "out", type: "output", params: { path: "actuator/y" } },
  ],
  edges: [{ from: { node_id: "in" }, to: { node_id: "out", input: "in" } }],
});
assert.equal(runtime.programState("a"), "stopped");
runtime.setValue("program/a", 9);
runtime.step(16);
assert.deepEqual(read("program/a"), { f32: 9 });
unrelated();

runtime.dispose();
console.log("@vizij/runtime programs: ok");
process.exit(0);
