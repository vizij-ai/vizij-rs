// `say` on a browser face: the run is spawned on the device's interpreter,
// the provider fetches the audio and the marks (from the harness's stand-in
// for the cloud function), hands them to the page's playback hook, and the
// face-standard viseme weights follow the marks as the playhead advances;
// the run ends once playback does, and a halted run stops being polled.
// Needs `VIZIJ_FIXTURES`; skips without.
import assert from "node:assert/strict";
import { FIXTURES, loadFace, open } from "./common.mjs";

if (!FIXTURES) {
  console.log("VIZIJ_FIXTURES unset — skipping the browser speech test");
  process.exit(0);
}

const { page, logs, close } = await open(FIXTURES);
try {
  await loadFace(page, "face", "Quori_Current_Extended.glb", { program: "none", audio: "scripted" });
  const result = await page.evaluate(async () => {
    const h = window.vizijHarness;
    const shape = (s) => h.path("face", `standard/vizij/viseme/${s}`);
    const paths = [shape("PP"), shape("aa"), h.path("face", "standard/vizij/viseme")];
    const sample = (status) => ({
      values: h.readValues("face", paths),
      status: h.readValues("face", [status])[status],
    });
    const handle = await h.spawnSkill("face", "say", { text: "hello", voice: "Ruth" });
    const samples = [];
    for (let i = 0; i < 30; i++) {
      await new Promise((r) => setTimeout(r, 50));
      samples.push(sample(handle.status));
    }
    // A second run, halted while it plays: it is never polled again.
    const halted = await h.spawnSkill("face", "say", { text: "and again", voice: "Ruth" });
    await new Promise((r) => setTimeout(r, 300));
    const pollsBeforeHalt = window.vizijPlayback.calls[1]?.polls ?? null;
    await h.halt("face", halted);
    await new Promise((r) => setTimeout(r, 300));
    const pollsAfterHalt = window.vizijPlayback.calls[1]?.polls ?? null;
    return {
      samples,
      playback: window.vizijPlayback.calls,
      halted: sample(halted.status),
      pollsBeforeHalt,
      pollsAfterHalt,
      shapePaths: paths,
    };
  });

  const [pp, aa, current] = result.shapePaths;
  const values = (s, p) => s.values[p];
  const num = (v) => (v && typeof v === "object" ? (v.f32 ?? v.f64 ?? 0) : 0);
  // The hook received the bytes and the marks the stand-in served.
  assert.equal(result.playback[0].bytes, 4, "the audio bytes reached the page's hook");
  assert.equal(result.playback[0].marks, 3, "the speech marks reached the page's hook");
  // The viseme weights changed over time: PP rose, then aa, then rest.
  const maxPP = Math.max(...result.samples.map((s) => num(values(s, pp))));
  const maxAA = Math.max(...result.samples.map((s) => num(values(s, aa))));
  assert.ok(maxPP > 0.3, `PP never rose (max ${maxPP})`);
  assert.ok(maxAA > 0.3, `aa never rose (max ${maxAA})`);
  const shapes = new Set(result.samples.map((s) => values(s, current)?.str));
  assert.ok(shapes.has("PP") && shapes.has("aa") && shapes.has("sil"), [...shapes].join(","));
  // The run ran, then ended once the playhead reported the end: its status
  // key moved from one value to another and rests on the last one.
  const statuses = result.samples.map((s) => JSON.stringify(s.status));
  const distinct = [...new Set(statuses)];
  assert.ok(distinct.length >= 2, `the run's status never changed: ${distinct}`);
  assert.equal(statuses[0], distinct[0], "running first");
  assert.equal(statuses[statuses.length - 1], distinct[distinct.length - 1], "then ended");
  console.log(`statuses: ${distinct.join(" -> ")}`);
  // The halted run: polled while it played, not after the halt.
  assert.ok(result.pollsBeforeHalt > 0, "the second run was playing");
  assert.equal(result.pollsAfterHalt, result.pollsBeforeHalt, "no poll after the halt");
  const state = await page.evaluate(() => window.vizijHarness.state());
  assert.deepEqual(state.stepErrors, []);
  console.log(`@vizij/runtime browser speech: ok (shapes ${[...shapes].join(" ")})`);
} catch (e) {
  console.error(logs.slice(-30).join("\n"));
  throw e;
} finally {
  await close();
}
