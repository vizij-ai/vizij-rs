// A Vizij is ready once the GPU has drawn it: from `ready` on, the page
// draws on, the face in its frames. Under the software GPU a face's first
// draw compiles its pipelines in the GPU process for seconds, during which
// the page draws no frame; a Vizij reported ready before that draw finished
// leaves the page frozen within a few frames of `ready`. Measured in the
// page's own frames: the longest wait in the first frames after `ready` is
// held against the frame time of the frames after those, so a slower
// machine is slower on both sides.
// Needs `VIZIJ_FIXTURES`; skips without.
import assert from "node:assert/strict";
import { PNG } from "pngjs";
import { FIXTURES, open } from "./common.mjs";

if (!FIXTURES) {
  console.log("VIZIJ_FIXTURES unset — skipping the browser ready test");
  process.exit(0);
}

const { page, logs, close } = await open(FIXTURES);
try {
  const gaps = await page.evaluate(async () => {
    const h = window.vizijHarness;
    const frame = () => new Promise((resolve) => requestAnimationFrame(resolve));
    await h.load("quori", "/fixtures/Quori_Current_Extended.glb", { program: "none" });
    // Twice the frames after which a face is ready whatever holds it back.
    for (let frames = 0; !h.ready("quori"); frames++) {
      if (frames > 1200) throw new Error("quori not ready after 1200 frames");
      await frame();
    }
    let last = performance.now();
    const gaps = [];
    for (let i = 0; i < 8; i++) {
      const now = await frame();
      gaps.push(now - last);
      last = now;
    }
    return gaps;
  });
  // Ready because it was drawn, not because something held it back.
  const held = logs.filter((l) => l.includes("not drawn after") || l.includes("without being drawn"));
  assert.deepEqual(held, [], "the face was drawn, nothing held it back");
  const median = (xs) => xs.slice().sort((a, b) => a - b)[Math.floor(xs.length / 2)];
  const stall = Math.max(...gaps.slice(0, 4));
  const frameTime = median(gaps.slice(4));
  console.log(
    `ready: the longest wait for a frame after it ${stall.toFixed(0)} ms, then a frame every ${frameTime.toFixed(0)} ms`,
  );
  assert.ok(
    stall <= 4 * frameTime + 250,
    `the page stalled ${stall.toFixed(0)} ms after ready, against ${frameTime.toFixed(0)} ms a frame`,
  );

  // The frame shows the face: the canvas is not the background alone.
  const { clip, background } = await page.evaluate(() => {
    const { x, y, width, height } = document.getElementById("faces").getBoundingClientRect();
    const hex = new URLSearchParams(location.search).get("background") ?? "000000";
    return {
      clip: { x, y, width, height },
      background: [0, 2, 4].map((i) => parseInt(hex.slice(i, i + 2), 16)),
    };
  });
  const shot = PNG.sync.read(await page.screenshot({ clip }));
  let face = 0;
  for (let i = 0; i < shot.data.length; i += 4) {
    const off = [0, 1, 2].reduce((sum, c) => sum + Math.abs(shot.data[i + c] - background[c]), 0);
    if (off > 24) face += 1;
  }
  const share = face / (shot.width * shot.height);
  console.log(`ready: the face covers ${(share * 100).toFixed(1)} % of the canvas`);
  assert.ok(share > 0.05, `the canvas shows ${(share * 100).toFixed(1)} % of a face`);

  const state = await page.evaluate(() => window.vizijHarness.state());
  assert.deepEqual(state.stepErrors, []);
  console.log("@vizij/runtime browser ready: ok");
} catch (e) {
  console.error(logs.slice(-30).join("\n"));
  throw e;
} finally {
  await close();
}
