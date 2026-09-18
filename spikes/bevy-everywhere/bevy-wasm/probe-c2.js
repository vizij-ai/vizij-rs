// Reads the overlay canvas's pixels (via drawImage into a 2D canvas inside a rAF) at a few points.
const { chromium } = require('playwright');
(async () => {
  const browser = await chromium.launch({ headless: true });
  const page = await browser.newPage({ viewport: { width: 800, height: 600 } });
  await page.goto('http://127.0.0.1:8765/multi-c.html');
  await page.waitForFunction(() => window.__spikeStarted === true, null, { timeout: 90000 });
  await page.waitForTimeout(4000);
  const px = await page.evaluate(() => new Promise((resolve) => {
    requestAnimationFrame(() => {
      const src = document.querySelector('#face-overlay');
      const c = document.createElement('canvas'); c.width = src.width; c.height = src.height;
      const ctx = c.getContext('2d', { willReadFrequently: true });
      ctx.drawImage(src, 0, 0);
      const at = (x, y) => Array.from(ctx.getImageData(x, y, 1, 1).data);
      const gl = src.getContext('webgl2');
      resolve({ contextAttrs: gl && gl.getContextAttributes(), outsideViewports: at(400, 20), insideSlotA: at(225, 200), insideSlotAbg: at(40, 300), insideSlotB: at(360, 480) });
    });
  }));
  console.log(JSON.stringify(px));
  await browser.close();
})().catch((e) => { console.error('FAILED', e.message); process.exit(1); });
