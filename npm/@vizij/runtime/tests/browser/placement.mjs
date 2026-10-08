// A Vizij's rectangle reaching past the canvas's edge: the part on the
// canvas draws, cropped from the framing of the whole rectangle — half a slot
// past the left edge shows the right half of the face exactly as the slot
// fully on the canvas draws it, and past the right edge the left half. A
// rectangle wholly off the canvas, or an empty one, draws nothing; the safe
// area follows the whole rectangle. Needs `VIZIJ_FIXTURES`; skips without.
import assert from "node:assert/strict";
import { PNG } from "pngjs";
import { FIXTURES, loadVizij, open, settled } from "./common.mjs";

if (!FIXTURES) {
  console.log("VIZIJ_FIXTURES unset — skipping the browser placement test");
  process.exit(0);
}
// The harness's canvas, at device pixel ratio 1.
const CANVAS = { width: 763, height: 486 };
const SLOT = { width: 380, height: 486 };
const HALF = SLOT.width / 2;
// Inset from the edges of every compared strip: the canvas's focus ring and
// a cut's column touch them.
const INSET = 4;

const { page, logs, close } = await open(FIXTURES);
try {
  // A static face, so two frames of it compare pixel for pixel.
  await loadVizij(page, "a", "Quori_Current_Extended.glb", {
    program: "none",
    ros4hri: false,
    animations: false,
  });
  const authored = await page.evaluate(async () => {
    const vizij = await import("../../dist/runtime/src/index.js");
    const bytes = new Uint8Array(await (await fetch("/fixtures/Quori_Current_Extended.glb")).arrayBuffer());
    return (await vizij.describe(bytes)).rootBounds;
  });

  /** Place the face on `rect`, let it draw, and read the canvas and the
   * face's safe area. */
  const placed = async (rect) => {
    await page.evaluate((rect) => window.vizijHarness.place("a", rect), rect);
    await settled(page);
    const png = PNG.sync.read(
      await page.screenshot({ clip: { x: 0, y: 0, ...CANVAS } }),
    );
    const area = await page.evaluate(() => window.vizijHarness.safeArea("a"));
    return { png, area };
  };
  /** The pixels of columns [x0, x1) of `png`, inset vertically. */
  const strip = (png, x0, x1) => {
    const rows = [];
    for (let y = INSET; y < png.height - INSET; y++) {
      const row = [];
      for (let x = x0; x < x1; x++) {
        const i = (y * png.width + x) * 4;
        row.push([png.data[i], png.data[i + 1], png.data[i + 2]]);
      }
      rows.push(row);
    }
    return rows;
  };
  const drawn = ([r, g, b]) => r + g + b > 24;
  /** The share of a strip's pixels the face covers (the page's background is
   * black). */
  const coverage = (rows) => {
    const pixels = rows.flat();
    return pixels.filter(drawn).length / pixels.length;
  };
  /** Mean absolute per-channel difference (0..255) of two same-size strips
   * over the pixels where either draws the face. */
  const faceDiff = (a, b) => {
    const [pa, pb] = [a.flat(), b.flat()];
    let sum = 0;
    let samples = 0;
    pa.forEach((p, i) => {
      const q = pb[i];
      if (!drawn(p) && !drawn(q)) return;
      for (let c = 0; c < 3; c++) sum += Math.abs(p[c] - q[c]);
      samples += 3;
    });
    return samples ? sum / samples : 0;
  };
  /** The safe area `bounds` covers in `rect` under contain. */
  const containArea = (rect, bounds) => {
    const scale = Math.min(rect.width / bounds.x, rect.height / bounds.y);
    const width = bounds.x * scale;
    const height = bounds.y * scale;
    return { x: rect.x + (rect.width - width) / 2, y: rect.y + (rect.height - height) / 2, width, height };
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

  // The slot fully on the canvas: the reference, its left and right halves.
  const onX = 191;
  const on = await placed({ x: onX, y: 0, ...SLOT });
  const leftHalf = strip(on.png, onX + INSET, onX + HALF - INSET);
  const rightHalf = strip(on.png, onX + HALF + INSET, onX + SLOT.width - INSET);
  console.log(`on canvas: coverage left ${coverage(leftHalf).toFixed(3)}, right ${coverage(rightHalf).toFixed(3)}`);
  assert.ok(coverage(leftHalf) > 0.05 && coverage(rightHalf) > 0.05, "the face draws across the slot");

  // Half past the left edge: the canvas's first half-slot shows the slot's
  // right half, where and as large as the whole slot draws it; nothing else
  // draws.
  const pastLeft = { x: -HALF, y: 0, ...SLOT };
  const left = await placed(pastLeft);
  const shownLeft = strip(left.png, INSET, HALF - INSET);
  const leftDiff = faceDiff(shownLeft, rightHalf);
  console.log(`past the left edge: right half diff ${leftDiff.toFixed(2)}`);
  assert.ok(leftDiff < 2, `the right half draws as on the canvas (diff ${leftDiff.toFixed(2)})`);
  assert.ok(Math.abs(coverage(shownLeft) - coverage(rightHalf)) < 0.01, "the same share of it");
  assert.equal(coverage(strip(left.png, HALF + INSET, CANVAS.width - INSET)), 0, "nothing draws past the slot");
  assertArea(left.area, containArea(pastLeft, authored.size), "the safe area past the left edge");

  // Half past the right edge: the slot's left half, at the canvas's end.
  const pastRight = { x: CANVAS.width - HALF, y: 0, ...SLOT };
  const right = await placed(pastRight);
  const shownRight = strip(right.png, CANVAS.width - HALF + INSET, CANVAS.width - INSET);
  const rightDiff = faceDiff(shownRight, leftHalf);
  console.log(`past the right edge: left half diff ${rightDiff.toFixed(2)}`);
  assert.ok(rightDiff < 2, `the left half draws as on the canvas (diff ${rightDiff.toFixed(2)})`);
  assert.ok(Math.abs(coverage(shownRight) - coverage(leftHalf)) < 0.01, "the same share of it");
  assert.equal(coverage(strip(right.png, INSET, CANVAS.width - HALF - INSET)), 0, "nothing draws before the slot");
  assertArea(right.area, containArea(pastRight, authored.size), "the safe area past the right edge");

  // Wholly off the canvas, or empty: nothing draws.
  const everything = (png) => coverage(strip(png, INSET, CANVAS.width - INSET));
  const off = await placed({ x: -SLOT.width - 10, y: 0, ...SLOT });
  assert.equal(everything(off.png), 0, "a slot off the canvas draws nothing");
  const empty = await placed({ x: onX, y: 0, width: 0, height: SLOT.height });
  assert.equal(everything(empty.png), 0, "an empty slot draws nothing");
  assert.equal(empty.area, null, "an empty slot has no safe area");

  // Back on the canvas, it draws as before.
  const back = await placed({ x: onX, y: 0, ...SLOT });
  const backDiff = faceDiff(strip(back.png, onX + INSET, onX + SLOT.width - INSET), strip(on.png, onX + INSET, onX + SLOT.width - INSET));
  assert.ok(backDiff < 1, `placed back, it draws as before (diff ${backDiff.toFixed(2)})`);

  const state = await page.evaluate(() => window.vizijHarness.state());
  assert.deepEqual(state.stepErrors, []);
  console.log("@vizij/runtime browser placement: ok");
} catch (e) {
  console.error(logs.slice(-30).join("\n"));
  throw e;
} finally {
  await close();
}
