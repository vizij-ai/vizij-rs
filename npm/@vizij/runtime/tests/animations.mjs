// Animations on a device with no Vizij, driven the way any client of the
// device drives them: the animation module's declared functions invoked by
// name. One animation is loaded at run time, played, paused (its speed kept),
// seeked, sped up while paused, run to completion, reloaded in place while it
// plays, stopped back to its first frame, silenced by its weight, and
// unloaded. Keys are read back from the store, states from the player states
// the animation source writes.
import assert from "node:assert/strict";
import { headerJson } from "@vizij/animation-module";
import {
  composeVizij,
  startRuntime,
  ANIMATION_PLAYERS_PATH,
  decodePlayerStates,
} from "../dist/runtime/src/index.js";

// --- a device stepping the animation source ----------------------------------
// A bundle with no graph composes to the animation source alone.
const graph = await composeVizij(
  { extensions: { VIZIJ_bundle: { graphs: [] } } },
  { animations: true, ros4hri: false },
);
const runtime = await startRuntime(graph);
// Step the device, 10 ms at a time, until a promise resolves — an invoke
// reads the method's signature on one step and applies its call on the next
// — then once more: the player states the animation source writes are a step
// behind.
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
const invoke = (method, args) => settle(runtime.invoke(method, args));
const players = () =>
  decodePlayerStates(runtime.readValues([ANIMATION_PLAYERS_PATH])[ANIMATION_PLAYERS_PATH]);
const playerNamed = (name) => players().find((player) => player.name === name);
const stepFor = (ms) => {
  for (let t = 0; t < ms; t += 10) runtime.step(10);
};
const read = (path) => runtime.readValues([path])[path]?.f32;
const x = () => read("face/x");

// --- the module's declared functions are the device's described methods ------
const header = JSON.parse(headerJson);
const described = await settle(runtime.describeMethods());
for (const { name } of header.exports) {
  assert.ok(
    described.some((method) => method.path === name && method.module === header.id),
    `the device describes ${name}`,
  );
}

// --- loaded silent ------------------------------------------------------------
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
const converted = runtime.moduleAnimation(ramp);
assert.deepEqual(
  { id: converted.id, name: converted.name, duration: converted.duration },
  { id: "ramp", name: "Ramp", duration: 1 },
);
const anim = await invoke("load_animation", { clip: converted.moduleAnimation });
const player = await invoke("create_player", { name: "ramp" });
const instance = await invoke("add_instance_with_weight", { player, anim, weight: { f32: 0 } });
await invoke("stop", { player });
stepFor(50);
assert.equal(x(), undefined, "an instance at weight 0 writes nothing");
let state = playerNamed("ramp");
assert.equal(state.player, player.u32);
assert.equal(state.instances[0].weight, 0);
assert.equal(state.state, "stopped");

// --- played: the ramp writes its key, the playhead advances ------------------
await invoke("set_weight", { player, instance, weight: { f32: 1 } });
await invoke("play", { player });
stepFor(300);
state = playerNamed("ramp");
assert.equal(state.state, "playing");
assert.ok(state.time > 0.2 && state.time < 0.45, `playhead at ${state.time}`);
assert.ok(Math.abs(x() - state.time) < 0.05, `linear ramp: ${x()} at ${state.time}`);

// --- paused: the playhead holds, the speed is kept, a seek moves the pose ----
await invoke("pause", { player });
const held = playerNamed("ramp").time;
stepFor(200);
state = playerNamed("ramp");
assert.equal(state.state, "paused");
assert.ok(Math.abs(state.time - held) < 1e-6, "paused, the playhead holds");
assert.equal(state.speed, 1, "a pause keeps the speed");
await invoke("seek", { player, time_ns: { u64: 750_000_000 } });
state = playerNamed("ramp");
assert.ok(Math.abs(state.time - 0.75) < 1e-3, JSON.stringify(state));
assert.ok(Math.abs(x() - 0.75) < 1e-3, `seeked pose: ${x()}`);

// --- once, at double speed set while paused: completes at its end ------------
await invoke("set_loop", { player, mode: "once" });
await invoke("set_speed", { player, speed: { f32: 2 } });
state = playerNamed("ramp");
assert.equal(state.state, "paused", "a speed change does not resume");
assert.equal(state.speed, 2);
await invoke("play", { player });
assert.equal(playerNamed("ramp").speed, 2, "play runs at the speed set");
stepFor(200);
state = playerNamed("ramp");
assert.equal(state.loopMode, "once");
assert.ok(Math.abs(state.time - 1) < 1e-6, JSON.stringify(state));
assert.ok(Math.abs(x() - 1) < 1e-3, `the last pose holds: ${x()}`);

// --- reloaded in place while it plays: the playback stays, the tracks change -
await invoke("set_loop", { player, mode: "loop" });
await invoke("set_speed", { player, speed: { f32: 1 } });
await invoke("seek", { player, time_ns: { u64: 0 } });
stepFor(200);
const before = playerNamed("ramp").time;
const inverted = runtime.moduleAnimation({
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
});
assert.deepEqual(
  await invoke("reload_animation", { anim, clip: inverted.moduleAnimation }),
  { bool: true },
);
state = playerNamed("ramp");
assert.equal(state.state, "playing", "a reloaded animation plays on");
assert.ok(
  state.time > before && state.time < before + 0.06,
  `the playhead carries over: ${before} → ${state.time}`,
);
assert.equal(state.loopMode, "loop");
assert.equal(state.speed, 1);
assert.equal(state.instances[0].weight, 1, "the instance keeps its weight");
assert.equal(state.instances[0].anim, anim.u32, "under the same animation id");
stepFor(100);
assert.ok(x() < -0.1, `the new tracks play: ${x()}`);

// --- stopped: back to its first frame; weight 0 lets go of its key -----------
await invoke("stop", { player });
stepFor(30);
state = playerNamed("ramp");
assert.equal(state.state, "stopped");
assert.equal(state.time, 0);
assert.ok(Math.abs(x()) < 1e-3, `a stopped animation shows its first frame: ${x()}`);
await invoke("set_weight", { player, instance, weight: { f32: 0 } });
runtime.setValue("face/x", { float: 0.5 });
stepFor(30);
assert.equal(x(), 0.5, "at weight 0 it no longer writes");

// --- unloaded ----------------------------------------------------------------
assert.deepEqual(await invoke("remove_player", { player }), { bool: true });
assert.deepEqual(await invoke("unload_animation", { anim }), { bool: true });
assert.deepEqual(await invoke("unload_animation", { anim }), { bool: false });
assert.deepEqual(players(), []);

runtime.dispose();
console.log("@vizij/runtime animations: ok");
