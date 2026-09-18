// Q2: two faces, one App. Quori in slot A, Toasty in slot B; then N cycles of
// unloadFace('a') + loadFace('a') without exiting the App; records wasm memory
// per cycle and whether the WebGL context survives.
// usage: node q2.js <url> <outPrefix> [cycles=25]
const { open, loadFace, probe, finish, main } = require('./common');
main(async (logs) => {
  const [url, out, cyclesArg] = process.argv.slice(2);
  const cycles = Number(cyclesArg || 25);
  const { browser, page } = await open(url, logs);
  const r = { timing: await page.evaluate(() => window.__timing), cycles: [] };
  r.memoryBeforeLoad = await page.evaluate(() => window.__vz.memory());
  r.readyMsA = await loadFace(page, 'a', 'Quori_Current_Extended.glb', 'slot-a');
  r.readyMsB = await loadFace(page, 'b', 'Toasty_Current.glb', 'slot-b');
  await page.waitForTimeout(1500);
  // Open Toasty's jaw so B is visibly its own device.
  await page.evaluate(() => { const d = window.__vz.devices.get('b').dev; d.setValue(d.rigPrefix + 'propsrig/mouth/jawud/value', JSON.stringify({ f32: 1.0 })); });
  await page.waitForTimeout(500);
  r.twoFaces = await probe(page);
  await page.screenshot({ path: out + '-1-two-faces.png', timeout: 60000 });
  const t0 = Date.now();
  for (let i = 1; i <= cycles; i++) {
    const t = Date.now();
    await page.evaluate(() => window.__vz.unload('a'));
    await page.waitForFunction(() => window.__vz.mod.faceCount() === 1, null, { timeout: 30000 });
    const memAfterUnload = await page.evaluate(() => window.__vz.memory());
    const readyMs = await loadFace(page, 'a', 'Quori_Current_Extended.glb', 'slot-a');
    const p = await probe(page);
    r.cycles.push({ i, cycleMs: Date.now() - t, readyMs, memAfterUnload, memAfterLoad: p.memoryBytes, faceCount: p.faceCount, contextLost: p.contextLost, frames: p.frames });
    if (i === 1 || i === cycles) await page.screenshot({ path: `${out}-cycle-${i}.png`, timeout: 60000 });
  }
  r.cyclesTotalMs = Date.now() - t0;
  await page.waitForTimeout(1500);
  r.final = await probe(page);
  // Both devices still live: open Quori's jaw (a direct rig input) after the last reload.
  await page.evaluate(() => { const d = window.__vz.devices.get('a').dev; d.setValue(d.rigPrefix + 'propsrig/mouth/jawud/value', JSON.stringify({ f32: 1.0 })); });
  await page.waitForTimeout(800);
  await page.screenshot({ path: out + '-2-final-open.png', timeout: 60000 });
  // A standard key on Quori: is it a free input or a graph output? Read it back after one step and after 500 ms.
  r.standardKey = await page.evaluate(async () => {
    const d = window.__vz.devices.get('a').dev; const k = d.rigPrefix + 'standard/vizij/mouth/morph/jaw_open';
    const before = d.readValues([k])[k];
    d.setValue(k, JSON.stringify({ f32: 1.0 }));
    const afterWrite = d.readValues([k])[k];
    await new Promise((res) => requestAnimationFrame(() => requestAnimationFrame(res)));
    const afterStep = d.readValues([k])[k];
    await new Promise((res) => setTimeout(res, 500));
    return { key: k, before, afterWrite, afterStep, after500ms: d.readValues([k])[k], steps: d.steps() };
  });
  r.orderTrace = await page.evaluate(() => window.__vz.mod.orderTrace());
  finish(out, logs, r);
  await browser.close();
});
