// Quori's animations on a page, driven through the animation module's
// declared functions by name, as any client of the device drives them: its
// own, loaded with the face and listed from the module's player states; one
// given a weight, played, paused and seeked, run to its end once at four
// times its speed, stopped back to its first frame and silenced; then one
// loaded at run time, played on the gaze, and unloaded. The gaze key is read
// from the store and the states from the player states. Needs
// `VIZIJ_FIXTURES`; skips without.
import assert from "node:assert/strict";
import { FIXTURES, loadVizij, open } from "./common.mjs";

if (!FIXTURES) {
  console.log("VIZIJ_FIXTURES unset — skipping the browser animations test");
  process.exit(0);
}

const NONESENSE = "authoring.timeline.clip.1"; // "Nonesense": 5 s, gaze/left_right from -0.04 to -0.43 over 1.26 s
const GAZE = "gaze/left_right";

const { page, logs, close } = await open(FIXTURES);
try {
  // No program: nothing else drives the gaze.
  await loadVizij(page, "face", "Quori_Current_Extended.glb", { program: "none" });

  // The test owns the device clock from here: what it reads is where its
  // steps took the device, however fast the page runs.
  await page.evaluate(() => window.vizijHarness.manual());
  const advance = (steps) => page.evaluate((steps) => window.vizijHarness.advance(16, steps), steps);
  // The face's own animations, each on a player named after it, land with
  // the device's first steps.
  const players = () => page.evaluate(() => window.vizijHarness.players("face"));
  await advance(2);
  assert.deepEqual(
    (await players()).map(({ name, duration }) => [name, duration]),
    [
      ["authoring.timeline.clip.1", 5],
      ["authoring.timeline.main", 15],
    ],
  );
  const stateOf = async (name) => (await players()).find((player) => player.name === name);
  const state = () => stateOf(NONESENSE);
  const gaze = () =>
    page.evaluate((path) => {
      const device = window.vizijHarness.device("face");
      const value = device.readValues([device.path(path)])[device.path(path)];
      return value ? (value.f32 ?? value.f64) : null;
    }, GAZE);
  // Step the device until `predicate` holds of the player named `name`
  // (bounded: 30 s of device time).
  const until = (predicate, arg, name = NONESENSE) =>
    page.evaluate(
      ([source, name, arg]) => {
        const h = window.vizijHarness;
        const holds = () =>
          new Function("s", "arg", `return ${source}`)(
            h.players("face").find((player) => player.name === name),
            arg,
          );
        for (let i = 0; i < 1875 && !holds(); i++) h.advance(16);
        if (!holds()) throw new Error(`never: ${source}`);
      },
      [predicate, name, arg],
    );
  const invoke = (method, args) =>
    page.evaluate(
      ([method, args]) => {
        const h = window.vizijHarness;
        return h.settle(h.invoke("face", method, args));
      },
      [method, args],
    );
  const u32 = (n) => ({ u32: n });

  let s = await state();
  assert.equal(s.state, "stopped", "a face's animation loads stopped");
  assert.equal(s.loopMode, "loop");
  assert.equal(s.instances[0].weight, 0, "and silent");
  const player = u32(s.player);
  const instance = u32(s.instances[0].instance);
  const resting = await gaze();

  // Given a weight and played: the gaze follows the animation.
  await invoke("set_weight", { player, instance, weight: { f32: 1 } });
  await invoke("play", { player });
  await until("s.time > 0.5");
  assert.equal((await state()).state, "playing");
  const playing = await gaze();
  assert.ok(playing < -0.15, `the animation drives the gaze: ${resting} → ${playing}`);

  // Paused, then seeked: the playhead holds where it is put.
  await invoke("pause", { player });
  await invoke("seek", { player, time_ns: { u64: 2_500_000_000 } });
  await until("Math.abs(s.time - 2.5) < 1e-3");
  assert.equal((await state()).state, "paused");
  await advance(20);
  assert.ok(Math.abs((await state()).time - 2.5) < 1e-3, "paused, the playhead holds");

  // Once, four times faster: it runs to its end and holds there.
  await invoke("set_loop", { player, mode: "once" });
  await invoke("set_speed", { player, speed: { f32: 4 } });
  await invoke("play", { player });
  await until("s.time >= 5");
  s = await state();
  assert.deepEqual({ time: s.time, loopMode: s.loopMode, speed: s.speed }, { time: 5, loopMode: "once", speed: 4 });

  // Stopped: back to its first frame, which it shows until its weight goes.
  await invoke("stop", { player });
  await until("s.time === 0 && s.state === 'stopped'");
  const first = await gaze();
  assert.ok(Math.abs(first - -0.04) < 1e-3, `the first frame's gaze: ${first}`);
  await invoke("set_weight", { player, instance, weight: { f32: 0 } });

  // An animation loaded at run time: its channel resolves through Quori's
  // rig, its player lists beside the face's own, it plays, and unloads.
  const LOOK = "runtime.look-right";
  const { moduleAnimation } = await page.evaluate(
    (animation) => window.vizijHarness.moduleAnimation("face", animation),
    {
      id: LOOK,
      name: "Look right",
      duration: 1,
      tracks: [
        {
          channel: GAZE,
          interpolation: "linear",
          keyframes: [
            { time: 0, value: 0.3, interpolation: null },
            { time: 1, value: 0.3, interpolation: null },
          ],
        },
      ],
    },
  );
  const anim = await invoke("load_animation", { clip: moduleAnimation });
  const look = await invoke("create_player", { name: LOOK });
  await invoke("add_instance", { player: look, anim });
  assert.deepEqual(
    (await players()).map(({ name }) => name),
    ["authoring.timeline.clip.1", "authoring.timeline.main", LOOK],
  );
  await until("s && s.state === 'playing' && s.time > 0.2", null, LOOK);
  const looked = await gaze();
  assert.ok(Math.abs(looked - 0.3) < 1e-3, `the run-time animation drives the gaze: ${looked}`);
  assert.deepEqual(await invoke("remove_player", { player: look }), { bool: true });
  assert.deepEqual(await invoke("unload_animation", { anim }), { bool: true });
  await advance(2);
  assert.deepEqual(
    (await players()).map(({ name }) => name),
    ["authoring.timeline.clip.1", "authoring.timeline.main"],
  );

  const { stepErrors } = await page.evaluate(() => window.vizijHarness.state());
  assert.deepEqual(stepErrors, []);
  console.log("@vizij/runtime browser animations: ok");
} catch (e) {
  console.error(logs.slice(-30).join("\n"));
  throw e;
} finally {
  await close();
}
