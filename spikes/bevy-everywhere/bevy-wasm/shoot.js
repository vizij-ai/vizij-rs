// Loads a spike page in headless Chromium and takes two screenshots ~1 s apart.
// usage: NODE_PATH=<semio_studio>/node_modules node shoot.js <url> <outPrefix> [chromium args...]
const { chromium } = require('playwright');
const fs = require('fs');
const logs = [];
(async () => {
  const [url, out, ...args] = process.argv.slice(2);
  const browser = await chromium.launch({ headless: true, args });
  const vw = Number(process.env.VW || 1100), vh = Number(process.env.VH || 720);
  const shotTimeout = Number(process.env.SHOT_TIMEOUT || 30000);
  const page = await browser.newPage({ viewport: { width: vw, height: vh } });
  page.on('console', (m) => logs.push(`[${m.type()}] ${m.text()}`));
  page.on('pageerror', (e) => logs.push(`[pageerror] ${e.message}`));
  const t0 = Date.now();
  await page.goto(url);
  await page
    .waitForFunction(() => window.__spikeStarted === true, null, { timeout: 90000 })
    .catch((e) => logs.push('[timeout] ' + e.message));
  const started = Date.now() - t0;
  await page.waitForTimeout(5000);
  await page.screenshot({ path: out + '-1.png', timeout: shotTimeout });
  await page.waitForTimeout(900);
  await page.screenshot({ path: out + '-2.png', timeout: shotTimeout });
  const info = await page.evaluate(() => {
    const probe = document.createElement('canvas');
    const g = probe.getContext('webgl2');
    const d = g && g.getExtension('WEBGL_debug_renderer_info');
    const canvases = [...document.querySelectorAll('canvas')].map((c) => ({
      id: c.id, w: c.width, h: c.height, hasWebgl2: !!c.getContext('webgl2'),
    }));
    return {
      renderer: g ? (d ? g.getParameter(d.UNMASKED_RENDERER_WEBGL) : g.getParameter(g.RENDERER)) : null,
      status: document.getElementById('status')?.textContent,
      canvases,
    };
  });
  fs.writeFileSync(out + '-console.log', logs.join('\n') + '\n');
  console.log(JSON.stringify({ args, startedMs: started, info, consoleLines: logs.length,
    errors: logs.filter((l) => /error|panic|failed/i.test(l)).slice(0, 12) }, null, 1));
  await browser.close();
})().catch((e) => { console.error('FAILED', e.message); try { fs.writeFileSync(process.argv[3] + '-console.log', logs.join('\n') + '\n'); } catch (_) {} process.exit(1); });
