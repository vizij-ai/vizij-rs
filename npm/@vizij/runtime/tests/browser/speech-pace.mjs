// `say` into the package's own player on a page stepping its device about
// once a second, as a page under a software GPU does: the page's first
// playback plays its visemes through, and a playback whose device stops
// stepping stops within the halt bound — `HALT_POLLS` (4) of the page's
// poll interval. The page's wall clock drives both, so this test waits on it.
// Needs `VIZIJ_FIXTURES`; skips without.
import assert from "node:assert/strict";
import { FIXTURES, loadVizij, open } from "./common.mjs";

if (!FIXTURES) {
  console.log("VIZIJ_FIXTURES unset — skipping the browser speech pace test");
  process.exit(0);
}

const HALT_POLLS = 4;
// The halt bound is checked every `IDLE_STOP_MS / 2`; a little over that.
const CHECK_SLACK_MS = 250;

const { page, logs, close } = await open(FIXTURES);
try {
  await loadVizij(page, "face", "Quori_Current_Extended.glb", { program: "none", audio: "page" });
  // The autoplay policy keeps the page's AudioContext suspended until a
  // user gesture resumes it.
  await page.evaluate(() => {
    window.audioUnlocked = window.vizijHarness.unlockAudioOnPress();
  });
  await page.mouse.click(20, 20);
  await page.evaluate(() => window.audioUnlocked);

  const result = await page.evaluate(
    async ([HALT_POLLS, CHECK_SLACK_MS]) => {
      const h = window.vizijHarness;
      const sleep = (ms) => new Promise((r) => setTimeout(r, ms));
      // One step a second, of a second of device time.
      h.manual();
      const beat = async () => {
        h.advance(1000);
        await sleep(1000);
      };
      const settle = async (pending) => {
        let done = false;
        pending.then(
          () => (done = true),
          () => (done = true),
        );
        for (let i = 0; i < 20 && !done; i++) await beat();
        return pending;
      };
      const current = h.path("face", "standard/vizij/viseme");
      const status = (handle) => h.runStatus(h.readValues("face", [handle.status])[handle.status]);
      const calls = window.vizijPlayback.calls;

      // The page's first playback: six seconds of audio, marks two seconds
      // apart. A step a second polls it within each mark's span and after
      // its last mark, at 4 s, whatever the phase of the steps against the
      // audio.
      const first = await settle(h.invoke("face", "say", { text: "one two three", voice: "Ruth" }));
      const samples = [];
      for (let i = 0; i < 20; i++) {
        await beat();
        samples.push({ viseme: h.readValues("face", [current])[current]?.str, status: status(first) });
        if (samples[samples.length - 1].status !== "running") break;
      }

      // A second playback of twelve seconds, its device stopped once it plays.
      const second = await settle(h.invoke("face", "say", { text: "a b c d e f", voice: "Ruth" }));
      const playing = () => {
        const at = calls[1]?.at ?? [];
        return at.length > 0 && at[at.length - 1][1] > 0;
      };
      for (let i = 0; i < 20 && !playing(); i++) await beat();
      const lastPoll = calls[1]?.at.at(-1)?.[0] ?? performance.now();
      // The poll interval the player measured is at most the widest gap
      // between two polls of one playback.
      const gaps = calls.flatMap(({ at }) => at.slice(1).map(([t], i) => t - at[i][0]));
      const widest = Math.max(...gaps);
      const bound = Math.max(250, HALT_POLLS * widest);
      await sleep(lastPoll + bound + CHECK_SLACK_MS - performance.now());
      const waited = performance.now() - lastPoll;
      const afterStop = window.vizijPlayback.playheads[1]?.();
      return {
        samples,
        firstAt: calls[0]?.at ?? [],
        secondAt: calls[1]?.at ?? [],
        secondStatus: status(second),
        widest,
        bound,
        waited,
        afterStop,
      };
    },
    [HALT_POLLS, CHECK_SLACK_MS],
  );

  const trace = JSON.stringify(result);
  // The first playback was polled from its start to its end, never stopped
  // unpolled: its first poll found it playing, and its playhead passed the
  // last mark before the audio ended.
  assert.ok(result.firstAt.length >= 3, `the first playback was polled through: ${trace}`);
  assert.notEqual(result.firstAt[0][1], null, `the first playback was stopped before its first poll: ${trace}`);
  const reached = Math.max(...result.firstAt.map(([, t]) => t ?? 0));
  assert.ok(reached >= 4000, `the first playback reached ${reached} ms of 6000, short of its last mark: ${trace}`);
  const shapes = new Set(result.samples.map((s) => s.viseme));
  assert.ok(shapes.has("PP") && shapes.has("aa"), `the visemes played: ${[...shapes].join(",")}\n${trace}`);
  assert.equal(result.samples.at(-1).status, "success", `the first run ended with its audio: ${trace}`);
  // The second playback, unpolled since its device stopped, stopped within
  // the bound, long before its twelve seconds of audio ran out.
  assert.ok(result.secondAt.length > 0 && result.secondAt.at(-1)[1] > 0, `the second playback played: ${trace}`);
  assert.ok(Number.isFinite(result.widest), `the polls measured an interval: ${trace}`);
  assert.equal(result.afterStop, null, `the second playback still played after ${Math.round(result.waited)} ms unpolled: ${trace}`);
  assert.ok(result.secondAt.at(-1)[1] + result.waited < 12_000, `the audio ran out before the bound: ${trace}`);
  const state = await page.evaluate(() => window.vizijHarness.state());
  assert.deepEqual(state.stepErrors, []);
  console.log(
    `@vizij/runtime browser speech pace: ok (polls ${Math.round(result.widest)} ms apart at most, ` +
      `stopped within ${Math.round(result.waited)} ms unpolled; shapes ${[...shapes].join(" ")})`,
  );
} catch (e) {
  console.error(logs.slice(-30).join("\n"));
  throw e;
} finally {
  await close();
}
