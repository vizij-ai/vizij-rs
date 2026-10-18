// Store keys held on Quori against each of its writers — its program, an
// animation playing, its pose driver — keep the values the page writes;
// released, each key takes its writer's value again at the next step.
// Holding a key nothing writes changes nothing. Needs `VIZIJ_FIXTURES`;
// skips without.
import assert from "node:assert/strict";
import { FIXTURES, loadVizij, open } from "./common.mjs";

if (!FIXTURES) {
  console.log("VIZIJ_FIXTURES unset — skipping the browser holds test");
  process.exit(0);
}

const NONESENSE = "authoring.timeline.clip.1"; // plays gaze/left_right from -0.04 to -0.43
const PROGRAM_KEY = "standard/vizij/left_eye/pos/x"; // the "Speaks" program moves it at idle
const GAZE = "gaze/left_right";
const POSE = "poses/pose_d_happy_d.weight"; // a rig input the pose driver reads
const NOBODY = "nobody/writes";

const { page, logs, close } = await open(FIXTURES);
try {
  await loadVizij(page, "face", "Quori_Current_Extended.glb");
  await page.evaluate(() => window.vizijHarness.manual());
  const advance = (steps, dtMs = 16) =>
    page.evaluate(([steps, dtMs]) => window.vizijHarness.advance(dtMs, steps), [steps, dtMs]);
  const values = (relative) =>
    page.evaluate((relative) => {
      const device = window.vizijHarness.device("face");
      const paths = relative.map((path) => device.path(path));
      const read = device.readValues(paths);
      return relative.map((_, i) => {
        const value = read[paths[i]];
        return value ? (value.f32 ?? value.f64 ?? null) : null;
      });
    }, relative);
  const write = (entries) =>
    page.evaluate((entries) => {
      const device = window.vizijHarness.device("face");
      for (const [path, value] of entries) device.setValue(device.path(path), value);
    }, entries);
  const near = (actual, expected, what) =>
    assert.ok(
      actual !== null && Math.abs(actual - expected) < 1e-6,
      `${what}: ${actual} where ${expected} was written`,
    );

  // Nonesense plays on the gaze, the program runs.
  await advance(2);
  await page.evaluate(async (name) => {
    const h = window.vizijHarness;
    const state = h.players("face").find((player) => player.name === name);
    await h.settle(
      h.invoke("face", "set_weight", {
        player: { u32: state.player },
        instance: { u32: state.instances[0].instance },
        weight: { f32: 1 },
      }),
    );
    await h.settle(h.invoke("face", "play", { player: { u32: state.player } }));
  }, NONESENSE);
  const [programBefore, gazeBefore] = await values([PROGRAM_KEY, GAZE]);
  await advance(20);
  const [programMoving, gazeMoving] = await values([PROGRAM_KEY, GAZE]);
  assert.notEqual(programMoving, programBefore, "the program moves its key");
  assert.notEqual(gazeMoving, gazeBefore, "the animation moves the gaze");

  // A key the pose driver writes: one that moves with the pose's weight, at
  // a moment no time passes for the program to move its own keys.
  await write([[POSE, 0]]);
  await advance(4, 0);
  const atRest = await page.evaluate(() => window.vizijHarness.snapshot("face"));
  await write([[POSE, 1]]);
  await advance(4, 0);
  const posed = await page.evaluate(() => window.vizijHarness.snapshot("face"));
  const prefix = await page.evaluate(() => window.vizijHarness.device("face").rigPrefix);
  const driven = Object.keys(posed).filter(
    (path) =>
      path.startsWith(prefix) &&
      !path.startsWith(`${prefix}standard/`) &&
      path !== `${prefix}${POSE}` &&
      typeof posed[path]?.f32 === "number" &&
      Math.abs(posed[path].f32 - (atRest[path]?.f32 ?? 0)) > 1e-3,
  );
  assert.ok(driven.length > 0, "the pose's weight drives the pose driver's outputs");
  const POSED = driven[0].slice(prefix.length);

  // Held: each keeps what the page writes, while its writer writes on.
  await page.evaluate(
    (keys) => {
      const device = window.vizijHarness.device("face");
      device.holdKeys(keys.map((key) => device.path(key)));
    },
    [PROGRAM_KEY, GAZE, POSED, NOBODY],
  );
  await write([
    [PROGRAM_KEY, 0.11],
    [GAZE, -0.2],
    [POSED, 0.33],
  ]);
  await write([[POSE, 0]]);
  await advance(30);
  let [program, gaze, posedValue, nobody] = await values([PROGRAM_KEY, GAZE, POSED, NOBODY]);
  near(program, 0.11, "the program's key, held");
  near(gaze, -0.2, "the gaze, held against the animation");
  near(posedValue, 0.33, "the pose driver's output, held");
  assert.equal(nobody, null, "a held key nothing writes stays unwritten");

  // Released, each takes its writer's value again at the next step.
  await page.evaluate((key) => {
    const device = window.vizijHarness.device("face");
    device.releaseKeys([device.path(key)]);
  }, GAZE);
  await advance(1);
  [program, gaze, posedValue] = await values([PROGRAM_KEY, GAZE, POSED]);
  assert.ok(Math.abs(gaze - -0.2) > 1e-3, `released, the animation writes the gaze (${gaze})`);
  near(program, 0.11, "the program's key, still held");
  near(posedValue, 0.33, "the pose driver's output, still held");

  await page.evaluate(() => window.vizijHarness.device("face").releaseKeys());
  await advance(1);
  [program, gaze, posedValue] = await values([PROGRAM_KEY, GAZE, POSED]);
  assert.ok(Math.abs(program - 0.11) > 1e-3, `released, the program writes its key (${program})`);
  assert.ok(
    Math.abs(posedValue - 0.33) > 1e-3,
    `released, the pose driver writes its output (${posedValue})`,
  );

  const state = await page.evaluate(() => window.vizijHarness.state());
  assert.deepEqual(state.stepErrors, []);
  console.log("@vizij/runtime browser holds: ok");
} catch (e) {
  console.error(logs.slice(-30).join("\n"));
  throw e;
} finally {
  await close();
}
