// An `invoke` answer costs about what `call`'s does for the same value.
//
// `call` hands its answer to JavaScript as one JSON string; `invoke` builds
// the answer as JavaScript values, crossing into the module for every object,
// array, string and number it holds. A crossing costs little only while no
// export runs the module's constructors: wasm-ld wraps every export in a call
// to them when the module never runs them itself, and Bevy's reflection
// registers each reflected type through such a constructor, hundreds of
// them. The module runs them once, when it is instantiated.
import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { dirname, resolve } from "node:path";
import { fileURLToPath } from "node:url";
import { composeVizij, startRuntime } from "../dist/runtime/src/index.js";
import { animationModule } from "./animation-guest.mjs";

// No export is the linker's command-style wrapper: wasm-bindgen's own
// exports (allocation, the externref table), which every JavaScript value an
// answer is built of goes through, are named for it when they are wrapped.
const wasm = resolve(dirname(fileURLToPath(import.meta.url)), "../dist/pkg/vizij_bg.wasm");
const wrapped = WebAssembly.Module.exports(new WebAssembly.Module(readFileSync(wasm)))
  .map((e) => e.name)
  .filter((name) => name.endsWith("_command_export"));
assert.deepEqual(wrapped, [], "no export runs the module's constructors on every call");

const header = JSON.parse(animationModule({ guest: false }).headerJson);
const stepValues = header.exports.find((e) => e.name === "step_values");
const dtNs = stepValues.parameters.find((p) => p.name === "dt_ns").id;

const graph = await composeVizij(
  { extensions: { VIZIJ_bundle: { graphs: [] } } },
  { animations: true }
);
const runtime = await startRuntime(graph);
const settle = async (pending) => {
  let done = false;
  Promise.all([pending].flat()).then(
    () => (done = true),
    () => (done = true)
  );
  for (let i = 0; !done; i++) {
    assert.ok(i < 10, "the promise resolves within a few steps");
    runtime.step(0);
    await new Promise((resolve) => setImmediate(resolve));
  }
  return Array.isArray(pending) ? Promise.all(pending) : pending;
};
await settle(runtime.describeMethods());

// An animation of 200 tracks: each `step_values` answers 200 values, a
// struct of a revision and a `values` list of `{ f64 }`.
const tracks = 200;
const { moduleAnimation } = runtime.moduleAnimation({
  id: "wide",
  name: "Wide",
  duration: 10,
  tracks: Array.from({ length: tracks }, (_, t) => ({
    channel: `face/c${t}`,
    interpolation: "linear",
    keyframes: Array.from({ length: 11 }, (_, i) => ({
      time: i,
      value: Math.sin(i + t),
      interpolation: null,
    })),
  })),
});
const anim = await settle(runtime.invoke("load_clip", { clip: moduleAnimation }));
const player = await settle(runtime.invoke("create_player", { name: { str: "p" } }));
await settle(runtime.invoke("add_instance", { player, anim }));

const dt = { u64: 0 };
const byName = () => runtime.invoke("step_values", { dt_ns: dt });
const byId = () =>
  runtime
    .call({ module_id: header.id, id: stepValues.id, args: [{ id: dtNs, value: dt }] })
    .then((result) => result.ret);

// The same answer, in the same form.
const [named, identified] = await settle([byName(), byId()]);
assert.equal(named.struct.fields[1].value.values.length, tracks);
assert.deepEqual(named, identified, "invoke answers the value call answers");

// A batch of operations shares one step, so a batch's time is its answers'.
const batch = 50;
const time = async (operation) => {
  const start = performance.now();
  await settle(Array.from({ length: batch }, operation));
  return performance.now() - start;
};
const ratios = [];
for (let round = 0; round < 9; round++) {
  ratios.push((await time(byName)) / (await time(byId)));
}
ratios.sort((a, b) => a - b);
const ratio = ratios[Math.floor(ratios.length / 2)];
// With the constructors run on every crossing, it takes about 17 times call's.
const MOST = 2.5;
assert.ok(
  ratio <= MOST,
  `an invoke answer of ${tracks} values took ${ratio.toFixed(1)}x call's (more than ${MOST}x)`
);
console.log(`invoke-answer-cost: invoke takes ${ratio.toFixed(2)}x call's time`);
