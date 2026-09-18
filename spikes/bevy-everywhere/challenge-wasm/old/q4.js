// Q4: mesh picking under WebGL2/SwiftShader. Clicks the page at the given
// canvas-relative points and reports what the observer recorded.
// usage: NODE_PATH=<semio_studio>/node_modules node q4.js <url> <outPrefix> x1,y1[,label] x2,y2[,label] ...
const { chromium } = require('playwright');
const fs = require('fs');
const logs = [];
(async () => {
  const [url, out, ...points] = process.argv.slice(2);
  const browser = await chromium.launch({ headless: true });
  const page = await browser.newPage({ viewport: { width: 1100, height: 720 } });
  page.on('console', (m) => logs.push(`[${m.type()}] ${m.text()}`));
  page.on('pageerror', (e) => logs.push(`[pageerror] ${e.message}`));
  await page.goto(url);
  await page.waitForFunction(() => window.__spikeStarted === true, null, { timeout: 120000 });
  await page.waitForFunction(() => window.__mod.indexReady() === true, null, { timeout: 60000 });
  await page.waitForTimeout(1500);
  await page.evaluate(() => { window.__cb = []; window.__mod.setClickCallback((s) => window.__cb.push(s)); });
  const rect = await page.evaluate(() => { const r = document.querySelector('#face-a').getBoundingClientRect(); return { x: r.x, y: r.y, w: r.width, h: r.height }; });
  const results = [];
  for (const p of points) {
    const [x, y, label] = p.split(',');
    const px = rect.x + Number(x), py = rect.y + Number(y);
    await page.mouse.move(px, py);
    await page.waitForTimeout(150);
    await page.mouse.click(px, py);
    await page.waitForTimeout(400);
    const clicks = await page.evaluate(() => window.__mod.drainClicks());
    const cb = await page.evaluate(() => { const c = window.__cb; window.__cb = []; return c; });
    results.push({ label: label || null, canvasXY: [Number(x), Number(y)], pageXY: [px, py], clicks: clicks.map((c) => JSON.parse(c)), callback: cb.length });
  }
  await page.screenshot({ path: out + '.png', timeout: 60000 });
  fs.writeFileSync(out + '-console.log', logs.join('\n') + '\n');
  console.log(JSON.stringify({ rect, results, errors: logs.filter((l) => /error|panic|failed/i.test(l)).slice(0, 12) }, null, 1));
  await browser.close();
})().catch((e) => { console.error('FAILED', e.message); try { fs.writeFileSync(process.argv[3] + '-console.log', logs.join('\n') + '\n'); } catch (_) {} process.exit(1); });
