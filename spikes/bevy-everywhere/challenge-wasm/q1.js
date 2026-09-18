// Q1: one module, JS-paced. Loads Quori into slot A, screenshots, writes two
// rig inputs through the JS-owned device, screenshots again; reports the
// step/frame interleaving and memory.
// usage: node q1.js <url> <outPrefix>
const { open, loadFace, probe, finish, main } = require('./common');
main(async (logs) => {
  const [url, out] = process.argv.slice(2);
  const { browser, page } = await open(url, logs);
  const r = { timing: await page.evaluate(() => window.__timing) };
  r.memoryBeforeLoad = await page.evaluate(() => window.__vz.memory());
  r.readyMs = await loadFace(page, 'a', 'Quori_Current_Extended.glb', 'slot-a');
  await page.waitForTimeout(1500);
  const paths = await page.evaluate(() => {
    const p = window.__vz.devices.get('a').dev.rigPrefix;
    return [p + 'standard/vizij/mouth/morph/jaw_open', p + 'propsrig/l_eye/translation/x', p + 'propsrig/mouth/jawud/value'];
  });
  r.paths = paths;
  r.before = { values: await page.evaluate((p) => window.__vz.devices.get('a').dev.readValues(p), paths), ...(await probe(page)) };
  await page.screenshot({ path: out + '-1.png', timeout: 60000 });
  await page.evaluate((p) => {
    const dev = window.__vz.devices.get('a').dev;
    dev.setValue(p[0], JSON.stringify({ f32: 1.0 }));
    dev.setValue(p[1], JSON.stringify({ float: 6.35 }));
  }, paths);
  await page.waitForTimeout(1000);
  r.after = { values: await page.evaluate((p) => window.__vz.devices.get('a').dev.readValues(p), paths), ...(await probe(page)) };
  await page.screenshot({ path: out + '-2.png', timeout: 60000 });
  r.orderTrace = await page.evaluate(() => window.__vz.mod.orderTrace());
  // How many frames does a write take to show? Sample the trace right after a write+step.
  r.rects = await page.evaluate(() => {
    const cr = window.__vz.canvas.getBoundingClientRect();
    const s = document.getElementById('slot-a').getBoundingClientRect();
    return { canvas: [cr.x, cr.y, cr.width, cr.height], slotA: [s.x, s.y, s.width, s.height] };
  });
  finish(out, logs, r);
  await browser.close();
});
