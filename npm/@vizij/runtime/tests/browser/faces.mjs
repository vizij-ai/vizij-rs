// Two faces on one canvas, each in its own rectangle, and a click on each
// reported as a pick naming the face and the element the GLB declares.
// Needs `VIZIJ_FIXTURES`; skips without.
import assert from "node:assert/strict";
import { FIXTURES, loadVizij, open } from "./common.mjs";

if (!FIXTURES) {
  console.log("VIZIJ_FIXTURES unset — skipping the browser faces test");
  process.exit(0);
}

const { page, logs, close } = await open(FIXTURES);
try {
  await loadVizij(page, "left", "Quori_Current_Extended.glb", { program: "none" });
  await loadVizij(page, "right", "Toasty_Current.glb", { program: "none" });
  await page.evaluate(() => {
    window.vizijHarness.place("left", { x: 0, y: 0, width: 381, height: 486 });
    window.vizijHarness.place("right", { x: 382, y: 0, width: 381, height: 486 });
  });
  await page.waitForTimeout(1000);

  // The elements each GLB declares, to check the picks against.
  const elements = await page.evaluate(async () => {
    const vizij = await import("../../dist/runtime/src/index.js");
    const ids = async (file) => {
      const bytes = new Uint8Array(await (await fetch(`/fixtures/${file}`)).arrayBuffer());
      return (await vizij.describe(bytes)).elements.map((e) => e.id);
    };
    return { left: await ids("Quori_Current_Extended.glb"), right: await ids("Toasty_Current.glb") };
  });
  assert.ok(elements.left.length > 0 && elements.right.length > 0);

  // A press in the middle of each rectangle lands on that face. The pointer
  // settles over the face before the press, and the press is held over a
  // frame: picking hovers from the pointer's last position, and a frame of
  // two faces under a software GPU can take longer than a click.
  await page.evaluate(() => window.vizijHarness.picks());
  const press = async (face, x, y) => {
    for (let attempt = 0; attempt < 5; attempt++) {
      await page.mouse.move(x, y);
      await page.waitForTimeout(500);
      await page.mouse.down();
      await page.waitForTimeout(500);
      await page.mouse.up();
      await page.waitForTimeout(300);
      const picks = await page.evaluate(() => window.vizijHarness.picks());
      if (picks.some((p) => p.vizijId === face)) return picks;
    }
    return [];
  };
  const picks = [...(await press("left", 190, 243)), ...(await press("right", 572, 243))];
  const byFace = (id) => picks.filter((p) => p.vizijId === id);
  assert.ok(byFace("left").length >= 1, `no pick on the left face: ${JSON.stringify(picks)}`);
  assert.ok(byFace("right").length >= 1, `no pick on the right face: ${JSON.stringify(picks)}`);
  for (const pick of byFace("left")) assert.ok(elements.left.includes(pick.elementId), pick.elementId);
  for (const pick of byFace("right")) assert.ok(elements.right.includes(pick.elementId), pick.elementId);

  const state = await page.evaluate(() => window.vizijHarness.state());
  assert.deepEqual(state.stepErrors, []);
  console.log(`@vizij/runtime browser faces: ok (${picks.length} picks)`);
} catch (e) {
  console.error(logs.slice(-30).join("\n"));
  throw e;
} finally {
  await close();
}
