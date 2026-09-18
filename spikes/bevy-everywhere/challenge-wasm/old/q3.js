// Q3: exit the App (AppExit) and start a new one in the same module instance.
// Variant A: same canvas; variant B: the canvas element replaced first.
// usage: NODE_PATH=<semio_studio>/node_modules node q3.js <url> <outPrefix> [replace]
const { chromium } = require('playwright');
const fs = require('fs');
const logs = [];
(async () => {
  const [url, out, mode] = process.argv.slice(2);
  const browser = await chromium.launch({ headless: true });
  const page = await browser.newPage({ viewport: { width: 1100, height: 720 } });
  page.on('console', (m) => logs.push(`[${m.type()}] ${m.text()}`));
  page.on('pageerror', (e) => logs.push(`[pageerror] ${e.message}`));
  await page.goto(url);
  await page.waitForFunction(() => window.__spikeStarted === true, null, { timeout: 120000 });
  await page.waitForFunction(() => window.__mod.indexReady() === true, null, { timeout: 60000 });
  await page.waitForTimeout(1500);
  const probe = () => page.evaluate(() => {
    const c = document.querySelector('#face-a');
    const gl = c.getContext('webgl2');
    return { running: window.__mod.running(), frames: window.__mod.frames(), indexReady: window.__mod.indexReady(),
      hasContext: !!gl, contextLost: gl ? gl.isContextLost() : null, canvasW: c.width, canvasH: c.height,
      wasmMemoryBytes: window.__exports.memory.buffer.byteLength, listeners: typeof getEventListeners === 'function' ? Object.keys(getEventListeners(c)) : null };
  });
  const r = {};
  r.beforeStop = await probe();
  await page.screenshot({ path: out + '-1-first.png', timeout: 60000 });
  logs.push('--- stop() ---');
  const tStop = Date.now();
  await page.evaluate(() => window.__mod.stop());
  await page.waitForFunction(() => window.__mod.running() === false, null, { timeout: 15000 })
    .catch((e) => logs.push('[timeout] running() never went false: ' + e.message));
  r.stopMs = Date.now() - tStop;
  await page.waitForTimeout(1000);
  r.afterStop = await probe();
  // Does the exited App still draw? Blank the canvas region by watching frames.
  await page.waitForTimeout(500);
  r.afterStopLater = await probe();
  await page.screenshot({ path: out + '-2-stopped.png', timeout: 60000 });
  if (mode === 'replace') {
    logs.push('--- replacing the canvas element ---');
    await page.evaluate(() => { const old = document.querySelector('#face-a'); const c = document.createElement('canvas'); c.id = 'face-a'; old.replaceWith(c); });
  }
  logs.push('--- start() again ---');
  const tStart = Date.now();
  r.secondStart = await page.evaluate(() => { try { window.__mod.start('#face-a', window.__glb); return 'ok'; } catch (e) { return 'threw: ' + (e && e.message ? e.message : String(e)); } });
  await page.waitForFunction(() => window.__mod.indexReady() === true, null, { timeout: 60000 })
    .catch((e) => logs.push('[timeout] second indexReady: ' + e.message));
  r.secondIndexMs = Date.now() - tStart;
  await page.waitForTimeout(1500);
  r.afterRestart = await probe();
  await page.screenshot({ path: out + '-3-restarted.png', timeout: 60000 });
  // Move the mouth on the second device to prove it is live.
  await page.evaluate(() => { const p = window.__mod.rigPrefix(); window.__mod.setValue(p + 'standard/vizij/mouth/morph/jaw_open', JSON.stringify({ f32: 1.0 })); });
  await page.waitForTimeout(800);
  r.afterRestartWrite = await probe();
  await page.screenshot({ path: out + '-4-restarted-open.png', timeout: 60000 });
  fs.writeFileSync(out + '-console.log', logs.join('\n') + '\n');
  console.log(JSON.stringify({ mode: mode || 'same-canvas', ...r, errors: logs.filter((l) => /error|panic|failed|RecreationAttempt/i.test(l)).slice(0, 15) }, null, 1));
  await browser.close();
})().catch((e) => { console.error('FAILED', e.message); try { fs.writeFileSync(process.argv[3] + '-console.log', logs.join('\n') + '\n'); } catch (_) {} process.exit(1); });
