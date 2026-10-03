// Two copies of one face side by side on one canvas, each framed and
// tone-mapped its own way through setView: framing one on bounds twice its
// authored size draws it at a quarter of its twin's area, a zoom of ½ on the
// twin draws it the same, the safe areas follow the framing, and tone
// mapping on one face changes its pixels and leaves its twin's alone. Needs
// `VIZIJ_FIXTURES`; skips without.
import assert from "node:assert/strict";
import { PNG } from "pngjs";
import { FIXTURES, loadVizij, open } from "./common.mjs";

if (!FIXTURES) {
  console.log("VIZIJ_FIXTURES unset — skipping the browser view test");
  process.exit(0);
}
const FACE = "Quori_Current_Extended.glb";
const RECTS = {
  a: { x: 0, y: 0, width: 381, height: 486 },
  b: { x: 382, y: 0, width: 381, height: 486 },
};
const SETTLE_MS = 1000;
const INSET = 4;

const { page, logs, close } = await open(FIXTURES);
try {
  // Static faces: no program, no ROS4HRI mapping (its idle blink runs on each
  // device's own clock) and no animations, so the twins' frames compare pixel
  // for pixel.
  for (const id of ["a", "b"]) {
    await loadVizij(page, id, FACE, { program: "none", ros4hri: false, animations: false });
  }
  await page.evaluate((rects) => {
    for (const [id, rect] of Object.entries(rects)) window.vizijHarness.place(id, rect);
  }, RECTS);
  const authored = await page.evaluate(async (file) => {
    const vizij = await import("../../dist/runtime/src/index.js");
    const bytes = new Uint8Array(await (await fetch(`/fixtures/${file}`)).arrayBuffer());
    return (await vizij.describe(bytes)).rootBounds;
  }, FACE);
  assert.ok(authored, "the face declares its bounds");

  const setView = async (id, view) => {
    await page.evaluate(([id, view]) => window.vizijHarness.setView(id, view), [id, view]);
  };
  /** Each face's rectangle as drawn, and its safe area, once the view settled. */
  const look = async () => {
    await page.waitForTimeout(SETTLE_MS);
    // Inset from the rectangle's edges, which the canvas's focus ring and
    // the gap between the rectangles touch.
    const shot = async ({ x, y, width, height }) =>
      PNG.sync.read(
        await page.screenshot({
          clip: { x: x + INSET, y: y + INSET, width: width - 2 * INSET, height: height - 2 * INSET },
        }),
      );
    return {
      a: await shot(RECTS.a),
      b: await shot(RECTS.b),
      areas: await page.evaluate(() => ({
        a: window.vizijHarness.safeArea("a"),
        b: window.vizijHarness.safeArea("b"),
      })),
    };
  };
  /** The share of a rectangle's pixels the face covers (the page's
   * background is black). */
  const coverage = (png) => {
    let covered = 0;
    for (let i = 0; i < png.data.length; i += 4) {
      if (png.data[i] + png.data[i + 1] + png.data[i + 2] > 24) covered += 1;
    }
    return covered / (png.width * png.height);
  };
  /** Mean absolute per-channel difference (0..255) of two same-size images
   * over the pixels where either one draws the face. */
  const faceDiff = (x, y) => {
    let sum = 0;
    let samples = 0;
    for (let i = 0; i < x.data.length; i += 4) {
      const drawn = (png) => png.data[i] + png.data[i + 1] + png.data[i + 2] > 24;
      if (!drawn(x) && !drawn(y)) continue;
      for (let c = 0; c < 3; c++) sum += Math.abs(x.data[i + c] - y.data[i + c]);
      samples += 3;
    }
    return samples ? sum / samples : 0;
  };
  /** The safe area `bounds` covers in `rect` under contain at `zoom`. */
  const containArea = (rect, bounds, zoom = 1) => {
    const scale = Math.min(rect.width / bounds.x, rect.height / bounds.y) * zoom;
    const width = bounds.x * scale;
    const height = bounds.y * scale;
    return {
      x: rect.x + (rect.width - width) / 2,
      y: rect.y + (rect.height - height) / 2,
      width,
      height,
    };
  };
  const assertArea = (actual, expected, what) => {
    assert.ok(actual, `${what}: no safe area`);
    for (const field of ["x", "y", "width", "height"]) {
      assert.ok(
        Math.abs(actual[field] - expected[field]) < 1,
        `${what}: ${JSON.stringify(actual)}, expected ${JSON.stringify(expected)}`,
      );
    }
  };

  // The page's framing: twins draw alike, their safe areas the authored
  // bounds fitted to their rectangles.
  const twins = await look();
  const twinDiff = faceDiff(twins.a, twins.b);
  console.log(`twins: coverage ${coverage(twins.a).toFixed(3)}, diff ${twinDiff.toFixed(2)}`);
  assert.ok(coverage(twins.a) > 0.05, "the face draws");
  assert.ok(twinDiff < 1, `the twins draw alike (diff ${twinDiff.toFixed(2)})`);
  assertArea(twins.areas.a, containArea(RECTS.a, authored.size), "a's safe area");
  assertArea(twins.areas.b, containArea(RECTS.b, authored.size), "b's safe area");

  // b framed on bounds twice the authored size: drawn at a quarter of the
  // area; its safe area is the new bounds, fitted the same way.
  const doubled = { x: authored.size.x * 2, y: authored.size.y * 2 };
  await setView("b", { bounds: { center: authored.center, size: doubled } });
  const framed = await look();
  const ratio = coverage(framed.b) / coverage(framed.a);
  console.log(`b on doubled bounds: coverage ratio ${ratio.toFixed(3)}`);
  assert.ok(ratio > 0.18 && ratio < 0.32, `b draws at a quarter of a's area (${ratio.toFixed(3)})`);
  assert.ok(faceDiff(framed.a, twins.a) < 1, "a's framing is a's own");
  assertArea(framed.areas.b, containArea(RECTS.b, doubled), "b's safe area on its bounds");

  // a at a zoom of ½ on its authored bounds frames what b frames: they draw
  // alike again, and a's safe area is half its fitted bounds.
  await setView("a", { zoom: 0.5 });
  const zoomed = await look();
  const zoomDiff = faceDiff(zoomed.a, zoomed.b);
  console.log(`a at zoom ½ against b on doubled bounds: diff ${zoomDiff.toFixed(2)}`);
  assert.ok(zoomDiff < 1, `zoom ½ frames as doubled bounds do (diff ${zoomDiff.toFixed(2)})`);
  assertArea(zoomed.areas.a, containArea(RECTS.a, authored.size, 0.5), "a's safe area at zoom ½");

  // Back to the page's framing, then tone mapping on b alone: b's pixels
  // move, a's stay; and b without it draws as before.
  await setView("a", {});
  await setView("b", {});
  const reset = await look();
  assert.ok(faceDiff(reset.a, reset.b) < 1, "{} hands both faces back to the page");
  await setView("b", { toneMapping: "aces" });
  const toned = await look();
  const tonedDiff = faceDiff(toned.a, toned.b);
  const aStill = faceDiff(toned.a, reset.a);
  console.log(`b under ACES: diff to a ${tonedDiff.toFixed(2)}, a moved ${aStill.toFixed(2)}`);
  assert.ok(tonedDiff > 5, `tone mapping changes b's pixels (diff ${tonedDiff.toFixed(2)})`);
  assert.ok(aStill < 1, `a's pixels stay (moved ${aStill.toFixed(2)})`);
  await setView("b", { toneMapping: "none" });
  const untoned = await look();
  const untonedDiff = faceDiff(untoned.a, untoned.b);
  assert.ok(untonedDiff < 1, `b without tone mapping draws as a (diff ${untonedDiff.toFixed(2)})`);

  // A view naming no curve the authoring app knows is refused.
  const refused = await page.evaluate(() => {
    try {
      window.vizijHarness.setView("b", { toneMapping: "filmic" });
      return null;
    } catch (e) {
      return String(e);
    }
  });
  assert.match(String(refused), /toneMapping must be/);

  const state = await page.evaluate(() => window.vizijHarness.state());
  assert.deepEqual(state.stepErrors, []);
  console.log("@vizij/runtime browser view: ok");
} catch (e) {
  console.error(logs.slice(-30).join("\n"));
  throw e;
} finally {
  await close();
}
