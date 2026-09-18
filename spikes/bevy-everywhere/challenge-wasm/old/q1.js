// Q1: one module = device + view. Loads the page, waits for the scene index,
// screenshots, writes two rig inputs through the device, screenshots again.
// usage: NODE_PATH=<semio_studio>/node_modules node q1.js <url> <outPrefix>
const { chromium } = require('playwright');
const fs = require('fs');
const logs = [];
(async () => {
  const [url, out] = process.argv.slice(2);
  const browser = await chromium.launch({ headless: true });
  const page = await browser.newPage({ viewport: { width: 1100, height: 720 } });
  page.on('console', (m) => logs.push(`[${m.type()}] ${m.text()}`));
  page.on('pageerror', (e) => logs.push(`[pageerror] ${e.message}`));
  const t0 = Date.now();
  await page.goto(url);
  await page.waitForFunction(() => window.__spikeStarted === true, null, { timeout: 120000 });
  const startedMs = Date.now() - t0;
  await page.waitForFunction(() => window.__mod.indexReady() === true, null, { timeout: 60000 })
    .catch((e) => logs.push('[timeout] indexReady: ' + e.message));
  const indexMs = Date.now() - t0;
  await page.waitForTimeout(1500);
  const prefix = await page.evaluate(() => window.__mod.rigPrefix());
  const paths = [prefix + 'standard/vizij/mouth/morph/jaw_open', prefix + 'propsrig/l_eye/translation/x', prefix + 'propsrig/mouth/jawud/value'];
  const before = await page.evaluate((p) => ({ values: window.__mod.readValues(p), frames: window.__mod.frames(), pose: window.__mod.poseLen(), timing: window.__timing }), paths);
  await page.screenshot({ path: out + '-1.png', timeout: 60000 });
  await page.evaluate((p) => {
    window.__mod.setValue(p[0], JSON.stringify({ f32: 1.0 }));
    window.__mod.setValue(p[1], JSON.stringify({ float: 6.35 }));
  }, paths);
  await page.waitForTimeout(1000);
  const after = await page.evaluate((p) => ({ values: window.__mod.readValues(p), frames: window.__mod.frames(), pose: window.__mod.poseLen() }), paths);
  await page.screenshot({ path: out + '-2.png', timeout: 60000 });
  const canvas = await page.evaluate(() => { const r = document.querySelector('#face-a').getBoundingClientRect(); return { x: r.x, y: r.y, w: r.width, h: r.height, cw: document.querySelector('#face-a').width, ch: document.querySelector('#face-a').height }; });
  const mem = await page.evaluate(() => window.__exports.memory.buffer.byteLength);
  fs.writeFileSync(out + '-console.log', logs.join('\n') + '\n');
  console.log(JSON.stringify({ startedMs, indexMs, before, after, canvas, wasmMemoryBytes: mem, consoleLines: logs.length,
    errors: logs.filter((l) => /error|panic|failed/i.test(l)).slice(0, 12) }, null, 1));
  await browser.close();
})().catch((e) => { console.error('FAILED', e.message); try { fs.writeFileSync(process.argv[3] + '-console.log', logs.join('\n') + '\n'); } catch (_) {} process.exit(1); });
