// The face skills run in the browser runtime as on the native device: the
// device registers the setters' and players' fragments, its interpreter
// describes them, and with the ROS4HRI mapping composed the expression keys
// run set_expression — the face boots at neutral from a setter run, and a
// key write, a set_expression call and a set_facial_action_units call take
// the face over from each other.
import assert from "node:assert/strict";
import { composeVizij, startRuntime } from "../dist/runtime/src/index.js";

const runtime = await startRuntime();
const settle = async (promise) => {
  let done = false;
  promise.then(
    () => (done = true),
    () => (done = true),
  );
  for (let i = 0; !done; i++) {
    assert.ok(i < 10, "the promise resolves within a few steps");
    runtime.step(16);
    await new Promise((resolve) => setImmediate(resolve));
  }
  return promise;
};
const stepFor = (ms) => {
  for (let t = 0; t < ms; t += 16) runtime.step(16);
};
const read = (path) => runtime.readValues([path])[path]?.f32 ?? 0;
const expression = (name) => read(`standard/vizij/expression/${name}`);

const plain = { extensions: { VIZIJ_bundle: { graphs: [] } } };
await settle(runtime.loadGraph(await composeVizij(plain, { program: "none", ros4hri: true })));

const described = (await settle(runtime.describeMethods())).map((method) => method.path);
for (const skill of ["set_expression", "set_facial_action_units", "set_viseme", "play_visemes"]) {
  assert.ok(described.includes(skill), `the device describes ${skill}`);
}

stepFor(800);
assert.ok(expression("neutral") > 0.95, "the face boots at neutral");

runtime.setValue("standard/ros4hri/expression/name", { text: "happy" });
runtime.setValue("standard/ros4hri/expression/arousal", { f32: 1 });
stepFor(800);
assert.ok(expression("happy") > 0.95, `the keys show happy (${expression("happy")})`);

await settle(
  runtime.invoke("set_facial_action_units", {
    codes: { u8s: [12] },
    intensities: { f32s: [0.8] },
  }),
);
stepFor(800);
assert.ok(expression("happy") < 0.01, "the action units take the face over");
assert.ok(expression("neutral") > 0.95, "over the neutral expression");
assert.ok(Math.abs(read("standard/vizij/face/mouth_smile_left") - 0.8) < 0.02);

await settle(
  runtime.invoke("set_expression", { expression: "sad", valence: { f32: 0 }, arousal: { f32: 1 } }),
);
stepFor(800);
assert.ok(expression("sad") > 0.95, "set_expression takes it back");
assert.ok(read("standard/vizij/face/mouth_smile_left") < 0.01);

console.log("face-skills: ok");
