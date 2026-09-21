// `say` on a browser face: the run is spawned on the device's interpreter,
// the provider fetches the audio and the marks (from the harness's stand-in
// for the cloud function), hands them to the page's playback hook, and the
// face-standard viseme weights follow the marks as the playhead advances;
// the run ends once playback does, and a halted run stops being polled.
// Needs `VIZIJ_FIXTURES`; skips without.
import assert from "node:assert/strict";
import { FIXTURES, loadVizij, open } from "./common.mjs";

if (!FIXTURES) {
  console.log("VIZIJ_FIXTURES unset — skipping the browser speech test");
  process.exit(0);
}

const { page, logs, close } = await open(FIXTURES);
try {
  await loadVizij(page, "face", "Quori_Current_Extended.glb", { program: "none", audio: "scripted" });
  const result = await page.evaluate(async () => {
    const h = window.vizijHarness;
    const shape = (s) => h.path("face", `standard/vizij/viseme/${s}`);
    const paths = [shape("PP"), shape("aa"), h.path("face", "standard/vizij/viseme")];
    const sample = (status) => ({
      values: h.readValues("face", paths),
      status: h.runStatus(h.readValues("face", [status])[status]),
    });
    const handle = await h.spawnSkill("face", "say", { text: "hello", voice: "Ruth" });
    // Sampled until the run ends (bounded): the script's 260 ms take as
    // many polls as the page steps in them, a slow page many seconds.
    const samples = [];
    for (let i = 0; i < 200; i++) {
      await new Promise((r) => setTimeout(r, 50));
      samples.push(sample(handle.status));
      if (samples[samples.length - 1].status === "success" && i >= 5) break;
    }
    // A second run, halted while it plays: it is never polled again. The
    // run fetches before it plays, and a slow page takes its time to the
    // first poll, so the halt waits for one (bounded).
    const halted = await h.spawnSkill("face", "say", { text: "and again", voice: "Ruth" });
    const polls = () => window.vizijPlayback.calls[1]?.polls ?? 0;
    for (let i = 0; i < 100 && polls() === 0; i++) {
      await new Promise((r) => setTimeout(r, 50));
    }
    const pollsBeforeHalt = polls();
    await h.halt("face", halted);
    // The halt lands on the run's next step; the count is compared once it
    // has, over a window of steps.
    await new Promise((r) => setTimeout(r, 300));
    const pollsAtHalt = polls();
    await new Promise((r) => setTimeout(r, 300));
    const pollsAfterHalt = polls();
    return {
      samples,
      playback: window.vizijPlayback.calls,
      halted: sample(halted.status),
      pollsBeforeHalt,
      pollsAtHalt,
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
  // The run ran, then succeeded once the playhead reported the end.
  const statuses = result.samples.map((s) => s.status);
  assert.equal(statuses[0], "running", "running first");
  assert.equal(statuses[statuses.length - 1], "success", "then ended");
  // The halted run: polled while it played, not after the halt.
  const second = JSON.stringify({
    playback: result.playback,
    halted: result.halted,
    polls: [result.pollsBeforeHalt, result.pollsAtHalt, result.pollsAfterHalt],
    statuses,
  });
  assert.ok(result.pollsBeforeHalt > 0, `the second run was playing: ${second}`);
  assert.equal(result.pollsAfterHalt, result.pollsAtHalt, "no poll after the halt");
  const state = await page.evaluate(() => window.vizijHarness.state());
  assert.deepEqual(state.stepErrors, []);
  console.log(`@vizij/runtime browser speech: ok (shapes ${[...shapes].join(" ")})`);
} catch (e) {
  console.error(logs.slice(-30).join("\n"));
  throw e;
} finally {
  await close();
}
