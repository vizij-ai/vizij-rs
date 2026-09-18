// Shared Playwright plumbing for the drivers: headless Chromium, console
// capture, the page's __vz helpers, a probe of the App/canvas state.
// NODE_PATH=<semio_studio>/node_modules
const { chromium } = require('playwright');
const fs = require('fs');

async function open(url, logs, viewport = { width: 1100, height: 620 }) {
  const browser = await chromium.launch({ headless: true });
  const page = await browser.newPage({ viewport });
  page.on('console', (m) => logs.push(`[${m.type()}] ${m.text()}`));
  page.on('pageerror', (e) => logs.push(`[pageerror] ${e.message}`));
  await page.goto(url);
  await page.waitForFunction(() => window.__spikeStarted === true, null, { timeout: 120000 });
  return { browser, page };
}

// Loads a face into a slot and waits until its scene is indexed.
async function loadFace(page, id, glbName, slotId, timeout = 240000) {
  const t = Date.now();
  await page.evaluate(([id, glbName, slotId]) => window.__vz.load(id, glbName, slotId), [id, glbName, slotId]);
  await page.waitForFunction((id) => window.__vz.mod.faceReady(id), id, { timeout });
  return Date.now() - t;
}

const probe = (page) => page.evaluate(() => {
  const v = window.__vz;
  const gl = v.canvas.getContext('webgl2');
  const devices = {};
  for (const [id, e] of v.devices) devices[id] = { steps: e.dev.steps(), alive: e.dev.alive(), poseLen: e.dev.poseLen(), loadMs: Math.round(e.loadMs) };
  return {
    running: v.mod.running(), frames: v.mod.frames(), faceCount: v.mod.faceCount(), rafTicks: v.rafTicks(),
    devices, memoryBytes: v.memory(), hasContext: !!gl, contextLost: gl ? gl.isContextLost() : null,
    contextEvents: v.contextEvents.slice(), stepErrors: v.stepErrors.slice(0, 5),
    canvas: { w: v.canvas.width, h: v.canvas.height },
  };
});

function finish(out, logs, result) {
  fs.writeFileSync(out + '-console.log', logs.join('\n') + '\n');
  result.errors = logs.filter((l) => /error|panic|failed|RecreationAttempt/i.test(l)).slice(0, 15);
  result.consoleLines = logs.length;
  console.log(JSON.stringify(result, null, 1));
}

function main(fn) {
  const logs = [];
  fn(logs).catch((e) => {
    console.error('FAILED', e.stack || e.message);
    try { fs.writeFileSync(process.argv[3] + '-console.log', logs.join('\n') + '\n'); } catch (_) {}
    process.exit(1);
  });
}

module.exports = { open, loadFace, probe, finish, main };
