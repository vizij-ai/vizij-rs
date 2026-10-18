// `loadGraph` and `applyGraphEdits` in a page settle on a device nothing steps
// (`../load-graph-scenario.mjs`, as the Node suite runs it). One issued from
// inside a step — by the page's playback hook, which a `say` run polls during
// the step — is an ordinary call: a step at a zero dt applies it once that step
// has ended, and it settles before the page's next step, with the edited graph
// already run.
// That part needs `VIZIJ_FIXTURES`; it skips without.
import assert from "node:assert/strict";
import { FIXTURES, loadVizij, open } from "./common.mjs";

const { page, logs, close } = await open(FIXTURES);
try {
  await page.evaluate(async () => {
    const vizij = await import("../../dist/runtime/src/index.js");
    const { loadGraphSettles } = await import("../load-graph-scenario.mjs");
    await loadGraphSettles(vizij);
  });

  if (!FIXTURES) {
    console.log("VIZIJ_FIXTURES unset — skipping an edit issued from inside a step");
  } else {
    await loadVizij(page, "face", "Quori_Current_Extended.glb", { program: "none", audio: "scripted" });
    const result = await page.evaluate(async () => {
      const h = window.vizijHarness;
      h.manual();
      const step = async () => {
        h.advance(16);
        await new Promise((r) => setTimeout(r, 0));
      };
      const settle = async (pending) => {
        let done = false;
        pending.then(
          () => (done = true),
          () => (done = true),
        );
        for (let i = 0; i < 100 && !done; i++) await step();
        return pending;
      };
      const read = () => h.readValues("face", ["test/edited"])["test/edited"];
      const edits = {
        upsert_nodes: [
          { id: "inside_k", type: "constant", params: { value: { f32: 0.5 } } },
          { id: "inside_out", type: "output", params: { path: "test/edited" } },
        ],
        upsert_edges: [{ from: { node_id: "inside_k" }, to: { node_id: "inside_out", input: "in" } }],
      };
      const seen = {};
      window.vizijPlayback.onPoll = () => {
        window.vizijPlayback.onPoll = null;
        seen.issuedAt = h.now();
        seen.inStep = read() ?? null;
        h.applyGraphEdits("face", edits).then(
          () => {
            seen.resolvedAt = h.now();
            seen.written = read() ?? null;
          },
          (e) => (seen.error = String(e)),
        );
      };
      await settle(h.invoke("face", "say", { text: "hello", voice: "Ruth" }));
      for (let i = 0; i < 2000 && seen.issuedAt === undefined; i++) await step();
      return { ...seen };
    });
    assert.notEqual(result.issuedAt, undefined, "the playback hook was polled");
    assert.equal(result.error, undefined, result.error);
    assert.equal(result.inStep, null, "nothing nested in the step that issued it");
    assert.equal(
      result.resolvedAt,
      result.issuedAt,
      "an edit issued inside a step settles before the page's next step",
    );
    assert.deepEqual(result.written, { f32: 0.5 }, "the step that applied it ran the edited graph");
    const state = await page.evaluate(() => window.vizijHarness.state());
    assert.deepEqual(state.stepErrors, []);
  }
  console.log("@vizij/runtime browser loadGraph: ok");
} catch (e) {
  console.error(logs.slice(-30).join("\n"));
  throw e;
} finally {
  await close();
}
