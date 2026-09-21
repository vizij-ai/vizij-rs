// Visual regression of the browser bundle: Quori and Toasty on a plain page
// stay close to the native references (`crates/vizij/tests/references`),
// under the metric the native regression uses — a 32×32 mean difference of
// at most 10. Needs `VIZIJ_FIXTURES` (the face GLBs) and a Chromium
// (`npx playwright install chromium`); skips without the fixtures.
import assert from "node:assert/strict";
import { promises as fs } from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";
import { FIXTURES, loadVizij, meanDiff32, open } from "./common.mjs";

if (!FIXTURES) {
  console.log("VIZIJ_FIXTURES unset — skipping the browser snapshot regression");
  process.exit(0);
}
const references = path.resolve(
  path.dirname(fileURLToPath(import.meta.url)),
  "../../../../../crates/vizij/tests/references",
);
const MAX_MEAN_DIFF = 10;
const cases = [
  ["quori", "Quori_Current_Extended.glb"],
  ["toasty", "Toasty_Current.glb"],
];

const { page, logs, close } = await open(FIXTURES);
try {
  for (const [name, file] of cases) {
    // The neutral face: no program, as the references were rendered.
    await loadVizij(page, name, file, { program: "none" });
    // Let the pose flow through the device onto the scene.
    await page.waitForTimeout(1500);
    const shot = await page.screenshot({ clip: { x: 0, y: 0, width: 763, height: 486 } });
    const outDir = process.env.VIZIJ_SNAPSHOT_OUT;
    if (outDir) {
      await fs.mkdir(outDir, { recursive: true });
      await fs.writeFile(path.join(outDir, `${name}-web.png`), shot);
    }
    const reference = await fs.readFile(path.join(references, `${name}.png`));
    const diff = meanDiff32(shot, reference);
    console.log(`${name}: mean 32×32 diff ${diff.toFixed(2)} (max ${MAX_MEAN_DIFF})`);
    assert.ok(
      diff < MAX_MEAN_DIFF,
      `${name}: the browser render drifted from the reference (mean diff ${diff.toFixed(2)})`,
    );
    await page.evaluate((id) => window.vizijHarness.unload(id), name);
  }
  const state = await page.evaluate(() => window.vizijHarness.state());
  assert.deepEqual(state.stepErrors, [], "no device failed to step");
  console.log("@vizij/runtime browser snapshot: ok");
} catch (e) {
  console.error(logs.slice(-30).join("\n"));
  throw e;
} finally {
  await close();
}
