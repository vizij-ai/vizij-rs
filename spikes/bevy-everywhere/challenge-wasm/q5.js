// Q5: AppExit, then a second App::run (mount) in the same module instance.
// usage: node q5.js <url> <outPrefix>
const { open, loadFace, probe, finish, main } = require('./common');
main(async (logs) => {
  const [url, out] = process.argv.slice(2);
  const { browser, page } = await open(url, logs);
  const r = {};
  r.readyMs = await loadFace(page, 'a', 'Quori_Current_Extended.glb', 'slot-a');
  await page.waitForTimeout(1000);
  r.beforeStop = await probe(page);
  await page.screenshot({ path: out + '-1-running.png', timeout: 60000 });
  logs.push('--- stop() ---');
  const tStop = Date.now();
  await page.evaluate(() => window.__vz.mod.stop());
  await page.waitForFunction(() => window.__vz.mod.running() === false, null, { timeout: 15000 })
    .catch((e) => logs.push('[timeout] running() never went false: ' + e.message));
  r.stopMs = Date.now() - tStop;
  await page.waitForTimeout(1000);
  r.afterStop = await probe(page);
  await page.waitForTimeout(500);
  r.afterStopLater = await probe(page);
  await page.screenshot({ path: out + '-2-stopped.png', timeout: 60000 });
  // Does the exited App still draw? Move device a's eye: the canvas must keep its last frame.
  await page.evaluate(() => { const d = window.__vz.devices.get('a').dev; d.setValue(d.rigPrefix + 'propsrig/l_eye/translation/x', JSON.stringify({ f32: 6.35 })); });
  await page.waitForTimeout(800);
  await page.screenshot({ path: out + '-2b-stopped-after-write.png', timeout: 60000 });
  // The device outlives the App: it is JS-owned.
  r.deviceAfterStop = await page.evaluate(() => { const d = window.__vz.devices.get('a').dev; try { d.step(16); return { ok: true, steps: d.steps(), poseLen: d.poseLen() }; } catch (e) { return { ok: false, err: String(e) }; } });
  logs.push('--- mount() again ---');
  r.secondMount = await page.evaluate(() => { try { window.__vz.mod.mount('#view'); return 'ok'; } catch (e) { return 'threw: ' + (e && e.message ? e.message : String(e)); } });
  await page.waitForTimeout(1500);
  r.afterSecondMount = await page.evaluate(() => { try { const m = window.__vz.mod; return { running: m.running(), frames: m.frames(), faceCount: m.faceCount() }; } catch (e) { return { err: String(e) }; } });
  if (r.secondMount === 'ok') {
    r.secondLoad = await page.evaluate(async () => { try { await window.__vz.load('a2', 'Quori_Current_Extended.glb', 'slot-a'); return 'ok'; } catch (e) { return 'threw: ' + String(e); } });
    await page.waitForFunction(() => window.__vz.mod.faceReady('a2'), null, { timeout: 60000 }).catch((e) => logs.push('[timeout] second faceReady: ' + e.message));
    await page.waitForTimeout(1500);
    r.afterRestart = await probe(page).catch((e) => ({ err: String(e) }));
    await page.screenshot({ path: out + '-3-restarted.png', timeout: 60000 });
    // The second App draws: move a2's eye.
    await page.evaluate(() => { const d = window.__vz.devices.get('a2').dev; d.setValue(d.rigPrefix + 'propsrig/l_eye/translation/x', JSON.stringify({ f32: 6.35 })); });
    await page.waitForTimeout(800);
    r.afterRestartWrite = await probe(page).catch((e) => ({ err: String(e) }));
    await page.screenshot({ path: out + '-4-restarted-after-write.png', timeout: 60000 });
  }
  finish(out, logs, r);
  await browser.close();
});
