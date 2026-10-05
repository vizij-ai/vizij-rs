// Animations on a device with no Vizij, through the animation module's
// declared functions: one loaded at run time, played, paused, seeked, sped
// up, run to completion, replaced while playing and while another client has
// paused it, stopped and unloaded; one loaded by another client's raw call
// sequence, listed and driven by the same methods; and one that is not there
// until its load lands. Keys are read back from the store, states from the
// player states.
// The ids the runtime calls are the module's declared ones; the states read
// back check PlayerState's and InstanceState's field ids.
import assert from "node:assert/strict";
import { headerJson } from "@vizij/animation-module";
import { composeVizij, startRuntime } from "../dist/runtime/src/index.js";
import {
  ANIMATION_IDS,
  ANIMATION_PLAYERS_PATH,
  decodePlayerStates,
} from "../dist/runtime/src/animations.js";

// --- the runtime's ids are the module's ---------------------------------------
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
// Step the device, 10 ms at a time, until a promise resolves — a call applies
// at a step, a load takes two, a stop's silence one more — then once more:
// the player states the animation source writes are a step behind.
const settle = async (promise) => {
  let done = false;
  promise.then(
    () => (done = true),
    () => (done = true),
  );
  for (let i = 0; !done; i++) {
    assert.ok(i < 10, "the promise resolves within a few steps");
    runtime.step(10);
    await new Promise((resolve) => setImmediate(resolve));
  }
  const value = await promise;
  runtime.step(10);
  return value;
};
// A client that knows only the module's declared functions: a call by
// function and parameter names, and the players it reads from the store.
const call = async (name, args) => {
  const ids = ANIMATION_IDS[name];
  const result = await settle(
    runtime.call({
      id: ids.function,
      args: Object.entries(args).map(([parameter, value]) => ({ id: ids[parameter], value })),
    }),
  );
  return result.ret;
};
const playerNamed = (name) =>
  decodePlayerStates(runtime.readValues([ANIMATION_PLAYERS_PATH])[ANIMATION_PLAYERS_PATH]).find(
    (player) => player.name === name,
  );
const stepFor = (ms) => {
  for (let t = 0; t < ms; t += 10) runtime.step(10);
};
const read = (path) => runtime.readValues([path])[path]?.f32;
const x = () => read("face/x");

assert.deepEqual(runtime.animations(), []);
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
assert.deepEqual(await settle(runtime.loadAnimation(ramp)), {
  id: "ramp",
  name: "Ramp",
  duration: 1,
});
assert.deepEqual(runtime.animations(), [{ id: "ramp", duration: 1 }]);
assert.equal(runtime.animationState("nope"), null);
stepFor(50);
assert.equal(x(), undefined, "a loaded animation is silent");
let state = runtime.animationState("ramp");
assert.equal(state.playing, false);
assert.equal(state.loop, true);
assert.equal(state.duration, 1);

// Play: the ramp writes its key, the playhead advances.
await settle(runtime.playAnimation("ramp", { reset: true }));
stepFor(300);
state = runtime.animationState("ramp");
assert.equal(state.playing, true);
assert.ok(state.time > 0.2 && state.time < 0.45, `playhead at ${state.time}`);
assert.ok(Math.abs(x() - state.time) < 0.05, `linear ramp: ${x()} at ${state.time}`);

// Pause holds; seek moves the held pose.
await settle(runtime.pauseAnimation("ramp"));
const held = runtime.animationState("ramp").time;
stepFor(200);
state = runtime.animationState("ramp");
assert.equal(state.playing, false);
assert.ok(Math.abs(state.time - held) < 1e-6, "paused, the playhead holds");
assert.equal(state.speed, 1, "a pause keeps the speed");
await settle(runtime.seekAnimation("ramp", 0.75));
state = runtime.animationState("ramp");
assert.ok(Math.abs(state.time - 0.75) < 1e-3, JSON.stringify(state));
assert.ok(Math.abs(x() - 0.75) < 1e-3, `seeked pose: ${x()}`);

// Once, at double speed, from 0.75 s: completes at the end and holds there.
await settle(runtime.setAnimationLoop("ramp", false));
await settle(runtime.setAnimationSpeed("ramp", 2));
await settle(runtime.playAnimation("ramp"));
assert.equal(runtime.animationState("ramp").speed, 2);
stepFor(200);
state = runtime.animationState("ramp");
assert.equal(state.completed, true, JSON.stringify(state));
assert.equal(state.playing, false);
assert.equal(state.loop, false);
assert.equal(state.time, 1);
assert.ok(Math.abs(x() - 1) < 1e-3, `the last pose holds: ${x()}`);

// Loaded again under its id while it plays: the new tracks play on from the
// playhead, at the speed and loop mode it had; the old animation is gone.
await settle(runtime.setAnimationLoop("ramp", true));
await settle(runtime.playAnimation("ramp", { reset: true, speed: 1 }));
stepFor(200);
const before = runtime.animationState("ramp").time;
const inverted = {
  ...ramp,
  tracks: [
    {
      ...ramp.tracks[0],
      keyframes: [
        { time: 0, value: 0, interpolation: null },
        { time: 1, value: -1, interpolation: null },
      ],
    },
  ],
};
await settle(runtime.loadAnimation(inverted));
state = runtime.animationState("ramp");
assert.equal(state.playing, true, "a replaced animation plays on");
assert.ok(
  state.time > before && state.time < before + 0.06,
  `the playhead carries over: ${before} → ${state.time}`,
);
assert.equal(state.loop, true);
assert.equal(state.speed, 1);
stepFor(100);
assert.ok(x() < -0.1, `the new tracks play: ${x()}`);
assert.deepEqual(
  runtime.animations().map(({ id }) => id),
  ["ramp"],
  "one animation under the id",
);

// Paused by another client, then set to once at double speed: loaded again,
// it stays paused where it was, once, at double speed.
await call("pause", { player: { u32: playerNamed("ramp").player } });
await settle(runtime.setAnimationLoop("ramp", false));
await settle(runtime.setAnimationSpeed("ramp", 2));
const pausedAt = runtime.animationState("ramp").time;
await settle(runtime.loadAnimation(ramp));
state = runtime.animationState("ramp");
assert.deepEqual(
  { playing: state.playing, loop: state.loop, speed: state.speed },
  { playing: false, loop: false, speed: 2 },
  JSON.stringify(state),
);
assert.ok(Math.abs(state.time - pausedAt) < 1e-6, `held at ${pausedAt}: ${state.time}`);
assert.ok(Math.abs(x() - pausedAt) < 1e-3, `the new tracks hold the pose: ${x()}`);
await settle(runtime.playAnimation("ramp"));
const resumed = runtime.animationState("ramp").time;
stepFor(100);
state = runtime.animationState("ramp");
assert.ok(
  Math.abs(state.time - resumed - 0.2) < 0.03,
  `double speed: ${resumed} → ${state.time}`,
);

// Stop: back to the first frame, then silent.
await settle(runtime.stopAnimation("ramp"));
stepFor(30);
state = runtime.animationState("ramp");
assert.equal(state.playing, false);
assert.equal(state.time, 0);
assert.ok(Math.abs(x()) < 1e-3, `cleared to the first frame: ${x()}`);
runtime.setValue("face/x", { float: 0.5 });
stepFor(30);
assert.equal(x(), 0.5, "a stopped animation no longer writes");

// Loaded again while stopped: still silent and stopped.
await settle(runtime.loadAnimation(ramp));
stepFor(30);
assert.equal(x(), 0.5, "a stopped animation loads silent");
assert.equal(runtime.animationState("ramp").time, 0);

assert.equal(await settle(runtime.unloadAnimation("ramp")), true);
assert.equal(await settle(runtime.unloadAnimation("ramp")), false);
assert.deepEqual(runtime.animations(), []);
await assert.rejects(runtime.playAnimation("ramp"), /no animation "ramp"/);

// --- another client's animation ------------------------------------------------
// A client that knows only the module's declared functions loads one: its
// `AnimationClip` built by hand, a player named after it, an instance at the
// default weight. The runtime lists it and drives it like its own.
const field = (id, value) => ({ id, value });
const handles = { structs: { id: "76697a69-6a00-0000-0000-000000000103", elements: [] } };
const keypoint = {
  fields: [
    field("76697a69-6a00-0000-0102-000000000001", { str: "k0" }),
    field("76697a69-6a00-0000-0102-000000000002", { f32: 0 }),
    field("76697a69-6a00-0000-0102-000000000003", { f32: 0.25 }),
    field("76697a69-6a00-0000-0102-000000000004", handles),
    field("76697a69-6a00-0000-0102-000000000005", handles),
  ],
};
const track = {
  fields: [
    field("76697a69-6a00-0000-0101-000000000001", { str: "wave:0" }),
    field("76697a69-6a00-0000-0101-000000000002", { str: "face/y" }),
    field("76697a69-6a00-0000-0101-000000000003", { str: "face/y" }),
    field("76697a69-6a00-0000-0101-000000000004", {
      structs: { id: "76697a69-6a00-0000-0000-000000000102", elements: [keypoint] },
    }),
  ],
};
const clip = {
  struct: {
    id: "76697a69-6a00-0000-0000-000000000100",
    fields: [
      field("76697a69-6a00-0000-0100-000000000001", { str: "Wave" }),
      field("76697a69-6a00-0000-0100-000000000002", { u32: 2000 }),
      field("76697a69-6a00-0000-0100-000000000003", {
        structs: { id: "76697a69-6a00-0000-0000-000000000101", elements: [track] },
      }),
    ],
  },
};
const anim = (await call("load_animation", { clip })).u32;
const player = (await call("create_player", { name: { str: "wave" } })).u32;
await call("add_instance", { player: { u32: player }, anim: { u32: anim } });
assert.deepEqual(runtime.animations(), [{ id: "wave", duration: 2 }]);
assert.equal(runtime.animationState("wave").playing, true, "a new player plays");
assert.equal(read("face/y"), 0.25, "its instance writes at the default weight");

await settle(runtime.stopAnimation("wave", { clearOutputs: false }));
stepFor(30);
assert.equal(runtime.animationState("wave").playing, false);
runtime.setValue("face/y", { float: 0.75 });
stepFor(30);
assert.equal(read("face/y"), 0.75, "silenced by the runtime");
assert.equal(await settle(runtime.unloadAnimation("wave")), true);
assert.deepEqual(runtime.animations(), []);

// --- an animation is there once its load lands ---------------------------------
// After the load's first step its player exists, without an instance yet:
// nothing reports the animation until its second step.
const loading = runtime.loadAnimation({ ...ramp, id: "late" });
await new Promise((resolve) => setImmediate(resolve)); // its first calls enqueue
runtime.step(10);
assert.equal(playerNamed("late")?.instances.length, 0, "the player came first");
assert.equal(runtime.animationState("late"), null);
assert.deepEqual(runtime.animations(), []);
await settle(loading);
state = runtime.animationState("late");
assert.equal(state.playing, false, "loaded stopped");
assert.deepEqual(runtime.animations(), [{ id: "late", duration: 1 }]);

runtime.dispose();
console.log("@vizij/runtime animations: ok");
