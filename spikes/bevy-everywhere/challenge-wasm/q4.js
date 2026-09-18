// Q4: mesh picking under WebGL2/SwiftShader with two faces in two viewports.
// Clicks the page at the given slot-relative points and reports what the
// observer recorded.
// usage: node q4.js <url> <outPrefix> slot:x,y[,label] ...   e.g. a:300,300,mouth b:100,80,eye
const { open, loadFace, probe, finish, main } = require('./common');
main(async (logs) => {
  const [url, out, ...points] = process.argv.slice(2);
  const { browser, page } = await open(url, logs);
  const r = {};
  r.readyMsA = await loadFace(page, 'a', 'Quori_Current_Extended.glb', 'slot-a');
  r.readyMsB = await loadFace(page, 'b', 'Toasty_Current.glb', 'slot-b');
  await page.waitForTimeout(1500);
  await page.evaluate(() => { window.__cb = []; window.__vz.mod.setClickCallback((s) => window.__cb.push(s)); });
  const rects = await page.evaluate(() => {
    const f = (id) => { const s = document.getElementById(id).getBoundingClientRect(); return { x: s.x, y: s.y, w: s.width, h: s.height }; };
    return { a: f('slot-a'), b: f('slot-b') };
  });
  r.rects = rects;
  r.results = [];
  for (const p of points) {
    const [slot, rest] = p.split(':');
    const [x, y, label] = rest.split(',');
    const rect = rects[slot];
    const px = rect.x + Number(x), py = rect.y + Number(y);
    await page.mouse.move(px, py);
    await page.waitForTimeout(150);
    await page.mouse.click(px, py);
    await page.waitForTimeout(400);
    const clicks = await page.evaluate(() => window.__vz.mod.drainClicks());
    const cb = await page.evaluate(() => { const c = window.__cb; window.__cb = []; return c; });
    r.results.push({ slot, label: label || null, slotXY: [Number(x), Number(y)], pageXY: [px, py], clicks: clicks.map((c) => JSON.parse(c)), callback: cb.length });
  }
  r.state = await probe(page);
  await page.screenshot({ path: out + '.png', timeout: 60000 });
  finish(out, logs, r);
  await browser.close();
});
