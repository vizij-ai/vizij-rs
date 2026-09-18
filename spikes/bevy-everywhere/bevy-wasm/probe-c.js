// Probes the overlay page: is the page text visible through the canvas, and what does the canvas look like?
const { chromium } = require('playwright');
(async () => {
  const browser = await chromium.launch({ headless: true });
  const page = await browser.newPage({ viewport: { width: 800, height: 600 } });
  await page.goto('http://127.0.0.1:8765/multi-c.html');
  await page.waitForFunction(() => window.__spikeStarted === true, null, { timeout: 90000 });
  await page.waitForTimeout(4000);
  const info = await page.evaluate(() => {
    const c = document.querySelector('#face-overlay');
    const cs = getComputedStyle(c);
    const p = document.querySelector('p');
    const r = p.getBoundingClientRect();
    const el = document.elementFromPoint(r.left + 5, r.top + 5);
    return { canvasAttr: { w: c.width, h: c.height }, canvasStyle: { w: cs.width, h: cs.height, pos: cs.position, bg: cs.backgroundColor, opacity: cs.opacity }, canvasStyleAttr: c.getAttribute('style'), pRect: r, scrollY: window.scrollY, elementAtP: el && (el.id || el.tagName), bodyBg: getComputedStyle(document.body).backgroundImage.slice(0, 60) };
  });
  console.log(JSON.stringify(info, null, 1));
  // Hide the canvas and screenshot to see the page underneath.
  await page.evaluate(() => { document.querySelector('#overlay').style.visibility = 'hidden'; });
  await page.screenshot({ path: 'shots/multi-c-v2-canvas-hidden.png', timeout: 120000 });
  await browser.close();
})().catch((e) => { console.error('FAILED', e.message); process.exit(1); });
