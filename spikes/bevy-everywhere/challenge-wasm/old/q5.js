// Q5: SPAWN/HALT of the play_viseme task fragment on the browser interpreter.
// usage: NODE_PATH=<semio_studio>/node_modules node q5.js <url> <outPrefix>
const { chromium } = require('playwright');
const fs = require('fs');
const logs = [];
(async () => {
  const [url, out] = process.argv.slice(2);
  const browser = await chromium.launch({ headless: true });
  const page = await browser.newPage({ viewport: { width: 1100, height: 720 } });
  page.on('console', (m) => logs.push(`[${m.type()}] ${m.text()}`));
  page.on('pageerror', (e) => logs.push(`[pageerror] ${e.message}`));
  await page.goto(url);
  await page.waitForFunction(() => window.__spikeStarted === true, null, { timeout: 120000 });
  await page.waitForFunction(() => window.__mod.indexReady() === true, null, { timeout: 60000 });
  await page.waitForTimeout(1000);
  const r = await page.evaluate(async () => {
    const wait = (ms) => new Promise((res) => setTimeout(res, ms));
    const m = window.__mod; const p = m.rigPrefix();
    const keys = (h) => [h.status, p + 'standard/vizij/viseme/aa', p + 'standard/vizij/viseme', p + 'poses/pose_a.weight', p + 'standard/vizij/mouth/morph/jaw_open', ...h.feedback];
    const out = { prefix: p, samples: [] };
    let handle;
    try { handle = JSON.parse(m.spawnPlayViseme('aa', 1.0)); out.handle = handle; } catch (e) { out.spawnError = String(e); return out; }
    for (let i = 0; i < 8; i++) { await wait(60); out.samples.push({ t: i * 60, frames: m.frames(), values: m.readValues(keys(handle)) }); }
    await wait(400);
    out.samples.push({ t: 'after ~900ms', frames: m.frames(), values: m.readValues(keys(handle)) });
    // A second run, halted mid-envelope.
    try { const h2 = JSON.parse(m.spawnPlayViseme('oh', 1.0)); out.handle2 = h2; await wait(120);
      out.beforeHalt = { frames: m.frames(), values: m.readValues(keys(h2)) };
      out.haltResult = JSON.parse(m.halt(JSON.stringify(h2))); await wait(120);
      out.afterHalt = { frames: m.frames(), values: m.readValues(keys(h2)) };
    } catch (e) { out.haltError = String(e); }
    return out;
  });
  await page.screenshot({ path: out + '.png', timeout: 60000 });
  fs.writeFileSync(out + '-console.log', logs.join('\n') + '\n');
  console.log(JSON.stringify({ ...r, errors: logs.filter((l) => /error|panic|failed/i.test(l)).slice(0, 12) }, null, 1));
  await browser.close();
})().catch((e) => { console.error('FAILED', e.message); try { fs.writeFileSync(process.argv[3] + '-console.log', logs.join('\n') + '\n'); } catch (_) {} process.exit(1); });
