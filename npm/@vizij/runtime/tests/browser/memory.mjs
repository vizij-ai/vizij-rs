// Twenty-five load/unload cycles reach a plateau of linear memory and keep
// the WebGL context alive: what an authoring session mounting faces over
// and over needs. Linear memory never shrinks, and the allocator grows it
// for the first cycles until the freed chunks of one cycle serve the next
// (the plateau); a leak would keep it growing by a face's worth (tens of
// MB) every cycle, linearly. How fast the plateau comes depends on how
// many frames each cycle gets — a software GPU ramps for longer, in small
// steps — so the criterion is the shape, not flatness: the last ten cycles
// grow less than the first five did (a leak's last ten grow at least twice
// the first five). Needs `VIZIJ_FIXTURES`; skips without.
import assert from "node:assert/strict";
import { FIXTURES, loadFace, open } from "./common.mjs";

if (!FIXTURES) {
  console.log("VIZIJ_FIXTURES unset — skipping the browser memory test");
  process.exit(0);
}
const CYCLES = 25;
// The ramp: the growth over these first cycles is what the tail is held under.
const RAMP_CYCLES = 5;
// The tail: the last cycles, whose growth must stay under the ramp's.
const TAIL_CYCLES = 10;

const { page, logs, close } = await open(FIXTURES);
try {
  const memory = [];
  for (let i = 0; i < CYCLES; i++) {
    await loadFace(page, "cycle", "Quori_Current_Extended.glb", { program: "none" });
    await page.waitForTimeout(100);
    await page.evaluate(() => window.vizijHarness.unload("cycle"));
    await page.waitForTimeout(100);
    memory.push(await page.evaluate(() => window.vizijHarness.memory()));
  }
  const state = await page.evaluate(() => window.vizijHarness.state());
  console.log(`memory after each cycle (MB): ${memory.map((b) => (b / 1048576).toFixed(0)).join(" ")}`);
  assert.deepEqual(state.contextEvents, [], "the WebGL context was never lost");
  assert.equal(state.contextLost, false);
  assert.deepEqual(state.stepErrors, []);
  const mb = (bytes) => (bytes / 1048576).toFixed(1);
  const ramp = memory[RAMP_CYCLES - 1] - memory[0];
  const tail = memory[CYCLES - 1] - memory[CYCLES - 1 - TAIL_CYCLES];
  assert.ok(
    tail < ramp,
    `linear memory grew by ${mb(tail)} MB over the last ${TAIL_CYCLES} cycles, ` +
      `against ${mb(ramp)} MB over the first ${RAMP_CYCLES}: no plateau`,
  );
  console.log(
    `@vizij/runtime browser memory: ok (plateau ${(memory[CYCLES - 1] / 1048576).toFixed(0)} MB)`,
  );
} catch (e) {
  console.error(logs.slice(-30).join("\n"));
  throw e;
} finally {
  await close();
}
