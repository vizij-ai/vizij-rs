// The clip transport on a device with no Vizij: a clip set live, played,
// paused, seeked, sped up, run to completion, edited, stopped and removed,
// its key read back from the store and its state from the player states.
// The ids the transport calls are the animation module's declared ones; the
// state read back checks PlayerState's field ids.
import assert from "node:assert/strict";
import { headerJson } from "@vizij/animation-module";
import { composeVizij, startRuntime } from "../dist/runtime/src/index.js";
import { ANIMATION_IDS } from "../dist/runtime/src/clips.js";

// --- the transport's ids are the module's ------------------------------------
const header = JSON.parse(headerJson);
for (const [name, ids] of Object.entries(ANIMATION_IDS)) {
  const declared = header.exports.find((e) => e.name === name);
  assert.ok(declared, `the module exports ${name}`);
  assert.equal(ids.function, declared.id, `${name}'s id`);
  for (const [parameter, id] of Object.entries(ids)) {
    if (parameter === "function") continue;
    const p = declared.parameters.find((q) => q.name === parameter);
    assert.equal(p?.id, id, `${name}(${parameter})'s id`);
  }
}

// --- a device stepping the animation source ----------------------------------
// A bundle with no graph composes to the animation source alone.
const graph = await composeVizij(
  { extensions: { VIZIJ_bundle: { graphs: [] } } },
  { animations: true, ros4hri: false },
);
const runtime = await startRuntime(graph);
// Step the device until a transport call's promise resolves — each call
// applies at a step, and a stop's silence one step after it — then once more:
// the player states the animation source writes are a step behind.
const settle = async (promise) => {
  let done = false;
  promise.then(() => (done = true));
  for (let i = 0; !done; i++) {
    assert.ok(i < 10, "the call resolves within a few steps");
    runtime.step(0);
    await new Promise((resolve) => setImmediate(resolve));
  }
  await promise;
  runtime.step(0);
};
const stepFor = (ms) => {
  for (let t = 0; t < ms; t += 10) runtime.step(10);
};
const x = () => runtime.readValues(["face/x"])["face/x"]?.f32;

assert.deepEqual(runtime.clips(), []);
const ramp = {
  id: "ramp",
  name: "Ramp",
  duration: 1,
  tracks: [
    {
      channel: "face/x",
      interpolation: "linear",
      keyframes: [
        { time: 0, value: 0, interpolation: null },
        { time: 1, value: 1, interpolation: null },
      ],
    },
  ],
};
assert.deepEqual(runtime.setClip(ramp), { id: "ramp", name: "Ramp", duration: 1 });
assert.deepEqual(runtime.clips(), [{ id: "ramp", name: "Ramp", duration: 1 }]);
assert.equal(runtime.clipState("nope"), null);
stepFor(50);
assert.equal(x(), undefined, "a loaded clip is silent");
let state = runtime.clipState("ramp");
assert.equal(state.playing, false);
assert.equal(state.loop, true);
assert.equal(state.duration, 1);

// Play: the ramp writes its key, the playhead advances.
await settle(runtime.playClip("ramp", { reset: true }));
stepFor(300);
state = runtime.clipState("ramp");
assert.equal(state.playing, true);
assert.ok(state.time > 0.2 && state.time < 0.45, `playhead at ${state.time}`);
assert.ok(Math.abs(x() - state.time) < 0.05, `linear ramp: ${x()} at ${state.time}`);

// Pause holds; seek moves the held pose.
await settle(runtime.pauseClip("ramp"));
const held = runtime.clipState("ramp").time;
stepFor(200);
state = runtime.clipState("ramp");
assert.equal(state.playing, false);
assert.ok(Math.abs(state.time - held) < 1e-6, "paused, the playhead holds");
assert.equal(state.speed, 1, "a pause keeps the speed");
await settle(runtime.seekClip("ramp", 0.75));
assert.ok(Math.abs(runtime.clipState("ramp").time - 0.75) < 1e-3, JSON.stringify(runtime.clipState("ramp")));
assert.ok(Math.abs(x() - 0.75) < 1e-3, `seeked pose: ${x()}`);

// Once, at double speed, from 0.75 s: completes at the end and holds there.
await settle(runtime.setClipLoop("ramp", false));
await settle(runtime.setClipSpeed("ramp", 2));
await settle(runtime.playClip("ramp"));
assert.equal(runtime.clipState("ramp").speed, 2);
stepFor(200);
state = runtime.clipState("ramp");
assert.equal(state.completed, true, JSON.stringify(state));
assert.equal(state.playing, false);
assert.equal(state.loop, false);
assert.equal(state.time, 1);
assert.ok(Math.abs(x() - 1) < 1e-3, `the last pose holds: ${x()}`);

// A live edit: same player, new tracks, playback kept.
await settle(runtime.playClip("ramp", { reset: true, speed: 1 }));
stepFor(100);
runtime.setClip({ ...ramp, tracks: [{ ...ramp.tracks[0], keyframes: [
  { time: 0, value: 0, interpolation: null },
  { time: 1, value: -1, interpolation: null },
] }] });
stepFor(100);
state = runtime.clipState("ramp");
assert.equal(state.playing, true, "an edited clip plays on");
assert.ok(x() < -0.1, `the edited tracks play: ${x()}`);

// Stop: back to the first frame, then silent.
await settle(runtime.stopClip("ramp"));
stepFor(30);
state = runtime.clipState("ramp");
assert.equal(state.playing, false);
assert.equal(state.time, 0);
assert.ok(Math.abs(x()) < 1e-3, `cleared to the first frame: ${x()}`);
runtime.setValue("face/x", { float: 0.5 });
stepFor(30);
assert.equal(x(), 0.5, "a stopped clip no longer writes");

assert.equal(runtime.removeClip("ramp"), true);
assert.equal(runtime.removeClip("ramp"), false);
assert.deepEqual(runtime.clips(), []);
assert.throws(() => runtime.playClip("ramp"), /no clip "ramp"/);

runtime.dispose();
console.log("@vizij/runtime clips: ok");
