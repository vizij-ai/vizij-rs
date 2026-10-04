// Quori's animations on a page: loaded with the face, one played, paused and
// seeked, run to completion once at four times its speed, stopped back to its
// first frame; its gaze key read from the store and its state from the
// player states. Needs `VIZIJ_FIXTURES`; skips without.
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

  const animations = await page.evaluate(() => window.vizijHarness.device("face").animations());
  assert.deepEqual(animations, [
    { id: "authoring.timeline.clip.1", name: "Nonesense", duration: 5 },
    { id: "authoring.timeline.main", name: "Stages", duration: 15 },
  ]);

  const state = () => page.evaluate((id) => window.vizijHarness.device("face").animationState(id), NONESENSE);
  const gaze = () =>
    page.evaluate((path) => {
      const device = window.vizijHarness.device("face");
      const value = device.readValues([device.path(path)])[device.path(path)];
      return value ? (value.f32 ?? value.f64) : null;
    }, GAZE);
  const until = (predicate, arg) =>
    page.waitForFunction(
      ([source, id, arg]) => new Function("s", "arg", `return ${source}`)(
        window.vizijHarness.device("face").animationState(id),
        arg,
      ),
      [predicate, NONESENSE, arg],
      { timeout: 60_000, polling: 50 },
    );
  const transport = (method, ...args) =>
    page.evaluate(
      ([method, id, args]) => window.vizijHarness.device("face")[method](id, ...args),
      [method, NONESENSE, args],
    );

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

  const { stepErrors } = await page.evaluate(() => window.vizijHarness.state());
  assert.deepEqual(stepErrors, []);
  console.log("@vizij/runtime browser animations: ok");
} catch (e) {
  console.error(logs.slice(-30).join("\n"));
  throw e;
} finally {
  await close();
}
