// A Vizij's device keeps no time until the Vizij is drawn. Every step before
// `ready` is a step of 0 whatever dt the page passes; the first step after
// advances the time since the frame that drew the Vizij, not the page's dt;
// the steps after that advance the page's dt. A `run()` called before
// `ready` starts its loop at that frame, and a `stop()` before then gives it
// up. Read on the device's own clock, `arora/time` and `arora/dt`
// (nanoseconds), with the page's steps deliberately long (500 ms) so a step
// that kept the page's dt shows.
// Needs `VIZIJ_FIXTURES`; skips without.
import assert from "node:assert/strict";
import { FIXTURES, open } from "./common.mjs";

if (!FIXTURES) {
  console.log("VIZIJ_FIXTURES unset — skipping the browser time test");
  process.exit(0);
}

const FACE = "/fixtures/Quori_Current_Extended.glb";
const PAGE_DT = 500;

const { page, logs, close } = await open(FIXTURES);
try {
  // Stepped by the page: no time before `ready`, the time since the drawn
  // frame at the first step after it, then the page's dt.
  const stepped = await page.evaluate(
    async ([face, pageDt]) => {
      const h = window.vizijHarness;
      h.manual();
      const frame = () => new Promise((resolve) => requestAnimationFrame(resolve));
      const clock = (id) => {
        const v = h.readValues(id, ["arora/time", "arora/dt"]);
        return { time: (v["arora/time"]?.u64 ?? 0) / 1e6, dt: (v["arora/dt"]?.u64 ?? 0) / 1e6 };
      };
      await h.load("stepped", face, { audio: false });
      const before = [];
      // When the page last saw the Vizij not drawn: the frame that draws it
      // comes after.
      let notDrawnAt = performance.now();
      let first = null;
      for (let frames = 0; first === null; frames++) {
        if (frames > 1200) throw new Error("not ready after 1200 frames");
        await frame();
        const drawn = h.ready("stepped");
        const seenAt = performance.now();
        h.step("stepped", pageDt);
        const read = clock("stepped");
        if (drawn) {
          first = { ...read, bound: performance.now() - notDrawnAt };
        } else {
          before.push(read);
          notDrawnAt = seenAt;
        }
      }
      const after = [];
      for (let i = 0; i < 3; i++) {
        await frame();
        h.step("stepped", pageDt);
        after.push(clock("stepped"));
      }
      return { before, first, after };
    },
    [FACE, PAGE_DT],
  );
  console.log(
    `time: ${stepped.before.length} steps before ready, then a first step of ${stepped.first.dt.toFixed(1)} ms`,
  );
  assert.ok(stepped.before.length > 0, "the page stepped the device before the Vizij was drawn");
  for (const [i, read] of stepped.before.entries()) {
    assert.deepEqual(read, { time: 0, dt: 0 }, `step ${i} before ready advances no time`);
  }
  assert.ok(
    stepped.first.dt > 0 && stepped.first.dt < PAGE_DT,
    `the first step after ready advances the time since the drawn frame, not the page's ${PAGE_DT} ms: ${stepped.first.dt} ms`,
  );
  assert.ok(
    stepped.first.dt <= stepped.first.bound,
    `at most the time since the page last saw the Vizij not drawn (${stepped.first.bound.toFixed(1)} ms): ${stepped.first.dt} ms`,
  );
  assert.equal(stepped.first.time, stepped.first.dt, "the device's time starts at that step");
  for (const [i, read] of stepped.after.entries()) {
    assert.equal(read.dt, PAGE_DT, `step ${i + 1} after ready advances the page's dt`);
  }

  // Under `run()` called before ready: no time until the drawn frame, then
  // at most the time since the page last saw the Vizij not drawn.
  const ran = await page.evaluate(async (face) => {
    const h = window.vizijHarness;
    const frame = () => new Promise((resolve) => requestAnimationFrame(resolve));
    const time = (id) => (h.readValues(id, ["arora/time"])["arora/time"]?.u64 ?? 0) / 1e6;
    await h.load("ran", face, { audio: false });
    const device = h.device("ran");
    const running = device.run(10);
    const before = [];
    let notDrawnAt = performance.now();
    for (let frames = 0; !h.ready("ran"); frames++) {
      if (frames > 1200) throw new Error("not ready after 1200 frames");
      before.push(time("ran"));
      notDrawnAt = performance.now();
      await frame();
    }
    for (let i = 0; i < 10; i++) await frame();
    const after = { time: time("ran"), bound: performance.now() - notDrawnAt, running: device.running };
    device.stop();
    await running;
    return { before, after };
  }, FACE);
  console.log(
    `time: under run(), ${ran.before.length} readings before ready, then ${ran.after.time.toFixed(1)} ms of device time in ${ran.after.bound.toFixed(1)} ms`,
  );
  assert.ok(ran.before.length > 0, "the run was called before the Vizij was drawn");
  for (const [i, t] of ran.before.entries()) {
    assert.equal(t, 0, `reading ${i} before ready: the run keeps no time`);
  }
  assert.ok(ran.after.running, "the run owns the device once the Vizij is drawn");
  assert.ok(
    ran.after.time > 0 && ran.after.time <= ran.after.bound,
    `the run's time starts at the drawn frame: ${ran.after.time} ms in ${ran.after.bound.toFixed(1)} ms`,
  );

  // `stop()` before ready gives the waiting run up: the drawn Vizij's
  // device is the page's, and nothing steps it.
  const given = await page.evaluate(async (face) => {
    const h = window.vizijHarness;
    const frame = () => new Promise((resolve) => requestAnimationFrame(resolve));
    await h.load("given", face, { audio: false });
    const device = h.device("given");
    const running = device.run(10);
    device.stop();
    await running;
    for (let frames = 0; !h.ready("given"); frames++) {
      if (frames > 1200) throw new Error("not ready after 1200 frames");
      await frame();
    }
    for (let i = 0; i < 10; i++) await frame();
    return {
      running: device.running,
      time: h.readValues("given", ["arora/time"])["arora/time"]?.u64 ?? 0,
    };
  }, FACE);
  assert.deepEqual(given, { running: false, time: 0 }, "a run stopped before ready never starts");

  const state = await page.evaluate(() => window.vizijHarness.state());
  assert.deepEqual(state.stepErrors, []);
  console.log("@vizij/runtime browser time: ok");
} catch (e) {
  console.error(logs.slice(-30).join("\n"));
  throw e;
} finally {
  await close();
}
