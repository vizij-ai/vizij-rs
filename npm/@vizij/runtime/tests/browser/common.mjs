// Shared Playwright plumbing for the browser tests: headless Chromium with a
// software WebGL2 (SwiftShader through ANGLE), console capture, the harness
// page, and the 32×32 mean-difference metric the native snapshot regression
// uses.
import { chromium } from "playwright";
import { PNG } from "pngjs";
import { serve } from "./serve.mjs";

export const FIXTURES = process.env.VIZIJ_FIXTURES;

export async function open(fixtures, query = "") {
  const server = await serve(fixtures);
  const browser = await chromium.launch({
    headless: true,
    args: ["--use-gl=angle", "--use-angle=swiftshader", "--enable-unsafe-swiftshader"],
  });
  const page = await browser.newPage({ viewport: { width: 800, height: 520 }, deviceScaleFactor: 1 });
  const logs = [];
  page.on("console", (m) => logs.push(`[${m.type()}] ${m.text()}`));
  page.on("pageerror", (e) => logs.push(`[pageerror] ${e.message}`));
  await page.goto(`${server.base}/tests/browser/index.html${query}`);
  try {
    await page.waitForFunction(() => window.vizijHarnessReady === true, null, { timeout: 120_000 });
  } catch (e) {
    console.error(logs.slice(-30).join("\n"));
    throw e;
  }
  return {
    page,
    logs,
    async close() {
      await browser.close();
      server.close();
    },
  };
}

/** Load a Vizij and wait until its scene is indexed. */
export async function loadVizij(page, id, file, options) {
  const device = await page.evaluate(
    ([id, file, options]) => window.vizijHarness.load(id, `/fixtures/${file}`, options),
    [id, file, options ?? null],
  );
  await page.waitForFunction((id) => window.vizijHarness.ready(id), id, { timeout: 240_000 });
  return device;
}

/** Mean absolute per-channel difference (0..255) of two PNG buffers, both
 * downscaled to 32×32 by box filtering. */
export function meanDiff32(a, b) {
  const pa = PNG.sync.read(a);
  const pb = PNG.sync.read(b);
  const box = (png) => {
    const out = new Float64Array(32 * 32 * 4);
    const counts = new Float64Array(32 * 32);
    for (let y = 0; y < png.height; y++) {
      const by = Math.min(31, Math.floor((y * 32) / png.height));
      for (let x = 0; x < png.width; x++) {
        const bx = Math.min(31, Math.floor((x * 32) / png.width));
        const i = (y * png.width + x) * 4;
        const o = (by * 32 + bx) * 4;
        for (let c = 0; c < 4; c++) out[o + c] += png.data[i + c];
        counts[by * 32 + bx] += 1;
      }
    }
    for (let p = 0; p < 32 * 32; p++) {
      for (let c = 0; c < 4; c++) out[p * 4 + c] /= counts[p] || 1;
    }
    return out;
  };
  const da = box(pa);
  const db = box(pb);
  let sum = 0;
  for (let i = 0; i < da.length; i++) sum += Math.abs(da[i] - db[i]);
  return sum / da.length;
}
