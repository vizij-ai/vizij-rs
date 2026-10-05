// Quori's animations on a page: its own, loaded with the face, listed from the
// module's player states; one played, paused and seeked, run to completion
// once at four times its speed, stopped back to its first frame; then one
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

  // The player states land with the device's first steps.
  const animations = () => page.evaluate(() => window.vizijHarness.device("face").animations());
  await page.waitForFunction(() => window.vizijHarness.device("face").animations().length > 0);
  assert.deepEqual(await animations(), [
    { id: "authoring.timeline.clip.1", duration: 5 },
    { id: "authoring.timeline.main", duration: 15 },
  ]);

  const stateOf = (id) =>
    page.evaluate((id) => window.vizijHarness.device("face").animationState(id), id);
  const state = () => stateOf(NONESENSE);
  const gaze = () =>
    page.evaluate((path) => {
      const device = window.vizijHarness.device("face");
      const value = device.readValues([device.path(path)])[device.path(path)];
      return value ? (value.f32 ?? value.f64) : null;
    }, GAZE);
  const until = (predicate, arg, id = NONESENSE) =>
    page.waitForFunction(
      ([source, id, arg]) =>
        new Function("s", "arg", `return ${source}`)(
          window.vizijHarness.device("face").animationState(id),
          arg,
        ),
      [predicate, id, arg],
      { timeout: 60_000, polling: 50 },
    );
  const call = (method, ...args) =>
    page.evaluate(
      ([method, args]) => window.vizijHarness.device("face")[method](...args),
      [method, args],
    );
  const transport = (method, ...args) => call(method, NONESENSE, ...args);

  let s = await state();
  assert.equal(s.playing, false, "a loaded animation is stopped");
  assert.equal(s.loop, true);
  assert.equal(s.duration, 5);
  const resting = await gaze();

  // Play from the start: the gaze follows the animation.
  await transport("playAnimation", { reset: true });
  await until("s.time > 0.5");
  s = await state();
  assert.equal(s.playing, true);
  assert.equal(s.completed, false);
  const playing = await gaze();
  assert.ok(playing < -0.15, `the animation drives the gaze: ${resting} → ${playing}`);

  // Pause, then seek: the playhead holds where it is put.
  await transport("pauseAnimation");
  await transport("seekAnimation", 2.5);
  await until("Math.abs(s.time - 2.5) < 1e-3");
  s = await state();
  assert.equal(s.playing, false);
  await page.waitForTimeout(300);
  assert.ok(Math.abs((await state()).time - 2.5) < 1e-3, "paused, the playhead holds");

  // Once, four times faster: it completes at its end and holds there.
  await transport("setAnimationLoop", false);
  await transport("setAnimationSpeed", 4);
  await transport("playAnimation");
  await until("s.completed");
  s = await state();
  assert.deepEqual(
    { time: s.time, playing: s.playing, loop: s.loop, speed: s.speed },
    { time: 5, playing: false, loop: false, speed: 4 },
  );

  // Stop: back to the first frame, then silent.
  await transport("stopAnimation", { clearOutputs: true });
  await until("s.time === 0 && !s.playing");
  const cleared = await gaze();
  assert.ok(Math.abs(cleared - -0.04) < 1e-3, `the first frame's gaze: ${cleared}`);

  // An animation loaded at run time: its channel resolves through Quori's
  // rig, it lists beside the face's own, plays, and unloads.
  const LOOK = "runtime.look-right";
  const loaded = await call("loadAnimation", {
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
  });
  assert.deepEqual(loaded, { id: LOOK, name: "Look right", duration: 1 });
  assert.deepEqual(
    (await animations()).map(({ id }) => id),
    ["authoring.timeline.clip.1", "authoring.timeline.main", LOOK],
  );
  assert.ok(Math.abs((await gaze()) - -0.04) < 1e-3, "a loaded animation is silent");
  await call("playAnimation", LOOK, { reset: true });
  await until("s.playing && s.time > 0.2", null, LOOK);
  const looked = await gaze();
  assert.ok(Math.abs(looked - 0.3) < 1e-3, `the run-time animation drives the gaze: ${looked}`);
  assert.equal(await call("unloadAnimation", LOOK), true);
  assert.equal(await stateOf(LOOK), null);
  assert.deepEqual(
    (await animations()).map(({ id }) => id),
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
