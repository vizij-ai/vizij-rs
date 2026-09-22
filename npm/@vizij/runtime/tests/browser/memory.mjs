// Twenty-five load/unload cycles reach a plateau of linear memory and keep
// the WebGL context alive: what an authoring session mounting faces over
// and over needs. Linear memory never shrinks, and the allocator grows it
// in chunks until the freed chunks of one cycle serve the next; a cycle
// served from freed chunks leaves the reading unchanged. That is the
// plateau's signature and what holds across machines: how many cycles the
// growth takes, and the mark it stops at, depend on the frames each cycle
// gets and on the bundle (a software GPU climbs in small steps for many
// cycles; the mark has ranged from 345 to 482 MB). A leak is a face's worth
// (tens of MB) on every cycle, so the reading never repeats. Needs
// `VIZIJ_FIXTURES`; skips without.
import assert from "node:assert/strict";
import { FIXTURES, loadVizij, open } from "./common.mjs";

if (!FIXTURES) {
  console.log("VIZIJ_FIXTURES unset — skipping the browser memory test");
  process.exit(0);
}
const CYCLES = 25;
// Among the last cycles, this many consecutive cycles must leave the
// reading unchanged: cycles served from freed chunks.
const TAIL_CYCLES = 10;
const STILL_CYCLES = 3;
// A gross bound a leak of a face per cycle passes long before the last
// cycle, whatever the allocator does.
const CEILING_MB = 700;

const { page, logs, close } = await open(FIXTURES);
try {
  const memory = [];
  for (let i = 0; i < CYCLES; i++) {
    await loadVizij(page, "cycle", "Quori_Current_Extended.glb", { program: "none" });
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
  const last = memory[CYCLES - 1];
  assert.ok(
    last < CEILING_MB * 1048576,
    `linear memory is at ${mb(last)} MB after ${CYCLES} cycles, over the ${CEILING_MB} MB ceiling`,
  );
  const tail = memory.slice(CYCLES - TAIL_CYCLES);
  let still = 1;
  let longest = 1;
  for (let i = 1; i < tail.length; i++) {
    still = tail[i] === tail[i - 1] ? still + 1 : 1;
    longest = Math.max(longest, still);
  }
  assert.ok(
    longest >= STILL_CYCLES,
    `no ${STILL_CYCLES} consecutive cycles of the last ${TAIL_CYCLES} left memory unchanged: no plateau`,
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
