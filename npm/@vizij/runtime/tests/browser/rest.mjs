// A face written away from rest and reset() back: every key its store held
// at rest reads its rest value again, and the face draws as it did at rest.
// Rest is the face as loaded with its neutral pose staged. Needs
// `VIZIJ_FIXTURES`; skips without.
import assert from "node:assert/strict";
import { FIXTURES, loadVizij, meanDiff32, open } from "./common.mjs";

if (!FIXTURES) {
  console.log("VIZIJ_FIXTURES unset — skipping the browser rest test");
  process.exit(0);
}
const CLIP = { x: 0, y: 0, width: 763, height: 486 };
// The settling time a write needs to reach the scene: the device steps on
// the page's frames, the view draws the pose a frame later.
const SETTLE_MS = 1500;

/** The keys of a snapshot that are the face's, without the runtime's own
 * (`arora/` built-ins: the clock, the tick). */
function faceKeys(snapshot) {
  return Object.keys(snapshot).filter((path) => !path.startsWith("arora/"));
}

/** Whether two store values are the same, numbers within `1e-4`. */
function same(a, b) {
  if (typeof a === "number" && typeof b === "number") return Math.abs(a - b) < 1e-4;
  if (a === null || b === null || typeof a !== "object" || typeof b !== "object") return a === b;
  const keys = new Set([...Object.keys(a), ...Object.keys(b)]);
  return [...keys].every((key) => same(a[key], b[key]));
}

const { page, logs, close } = await open(FIXTURES);
try {
  await loadVizij(page, "quori", "Quori_Current_Extended.glb", { program: "none" });
  await page.waitForTimeout(SETTLE_MS);
  const rest = await page.evaluate(() => window.vizijHarness.snapshot("quori"));
  const restShot = await page.screenshot({ clip: CLIP });
  assert.ok(faceKeys(rest).length > 0, "the face holds keys at rest");

  // Away from rest: every number the face holds, moved by half a unit — its
  // inputs, which stay where they are written, and its graphs' outputs,
  // which the next tick recomputes from them.
  const writes = await page.evaluate((rest) => {
    const writes = {};
    for (const [path, value] of Object.entries(rest)) {
      const number = value && Object.values(value)[0];
      if (path.startsWith("arora/") || typeof number !== "number") continue;
      writes[path] = number + 0.5;
      window.vizijHarness.setValue("quori", path, number + 0.5);
    }
    return writes;
  }, rest);
  assert.ok(Object.keys(writes).length > 0, "the face holds numbers to write");
  await page.waitForTimeout(SETTLE_MS);
  const moved = await page.evaluate(() => window.vizijHarness.snapshot("quori"));
  const movedKeys = faceKeys(rest).filter((path) => !same(rest[path], moved[path]));
  const awayDiff = meanDiff32(await page.screenshot({ clip: CLIP }), restShot);
  console.log(
    `away from rest: ${Object.keys(writes).length} writes, ${movedKeys.length} keys moved, mean 32×32 diff ${awayDiff.toFixed(2)}`,
  );
  assert.ok(movedKeys.length > 0, "the writes moved keys the face holds at rest");
  assert.ok(awayDiff > 2, `the writes changed how the face draws (diff ${awayDiff.toFixed(2)})`);

  await page.evaluate(() => window.vizijHarness.reset("quori"));
  await page.waitForTimeout(SETTLE_MS);
  const back = await page.evaluate(() => window.vizijHarness.snapshot("quori"));
  const astray = faceKeys(rest)
    .filter((path) => !same(rest[path], back[path]))
    .map((path) => `${path}: ${JSON.stringify(rest[path])} → ${JSON.stringify(back[path])}`);
  assert.deepEqual(astray, [], "every key held at rest is back at its rest value");
  const backDiff = meanDiff32(await page.screenshot({ clip: CLIP }), restShot);
  console.log(`after reset: mean 32×32 diff ${backDiff.toFixed(2)}`);
  assert.ok(backDiff < 0.5, `the face draws as it did at rest (diff ${backDiff.toFixed(2)})`);

  const state = await page.evaluate(() => window.vizijHarness.state());
  assert.deepEqual(state.stepErrors, []);
  console.log("@vizij/runtime browser rest: ok");
} catch (e) {
  console.error(logs.slice(-30).join("\n"));
  throw e;
} finally {
  await close();
}
