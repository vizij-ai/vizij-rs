// Two programs of one face played together, one stopped with its outputs
// reset to rest, the other replaced live — while the store keeps its other
// values through every step. Quori's bundle carries two motiongraphs:
// "Speaks", its active program, and "Live". The ROS4HRI mapping is left out
// so the programs alone write their keys (the mapping writes some of
// Speaks'). Needs `VIZIJ_FIXTURES`; skips without.
import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { join } from "node:path";
import { FIXTURES, loadVizij, open } from "./common.mjs";

if (!FIXTURES) {
  console.log("VIZIJ_FIXTURES unset — skipping the browser programs test");
  process.exit(0);
}

// The bundle, read from the GLB's JSON chunk.
const glb = readFileSync(join(FIXTURES, "Quori_Current_Extended.glb"));
const gltf = JSON.parse(glb.subarray(20, 20 + glb.readUInt32LE(12)).toString("utf8"));
const bundle =
  gltf.nodes?.map((node) => node.extensions?.VIZIJ_bundle).find(Boolean) ??
  gltf.extensions.VIZIJ_bundle;
const programs = bundle.graphs.filter((graph) => graph.kind === "motiongraph");
const speaks = programs.find((graph) => graph.id === bundle.metadata.activeMotionGraphId);
const live = programs.find((graph) => graph.id !== speaks.id);
assert.ok(speaks && live, "the face carries two programs");
const outputs = (spec) =>
  spec.nodes.filter((node) => node.type === "output").map((node) => node.params.path);
const speaksKeys = outputs(speaks.spec);
const liveKeys = outputs(live.spec);

// Where Live's keys rest: the neutral pose the face stages, on the keys its
// rig inputs read; unset elsewhere.
const neutral = bundle.poses?.config?.neutralInputs ?? {};
const rig = bundle.graphs.find((graph) => graph.kind === "rig").spec;
const neutralByPath = {};
for (const node of rig.nodes) {
  const name = node.type === "input" && node.id.startsWith("input_") ? node.id.slice(6) : null;
  if (name !== null && name in neutral) neutralByPath[node.params.path] = neutral[name];
}
const liveRest = Object.fromEntries(liveKeys.map((key) => [key, neutralByPath[key] ?? null]));
assert.ok(
  Object.values(liveRest).some((rest) => rest !== null) && Object.values(liveRest).includes(null),
  "Live writes keys that rest at a neutral value and keys that rest unset",
);

// Speaks replaced by itself with one output fed a constant instead.
const probe = speaksKeys[0];
const probeNode = speaks.spec.nodes.find((node) => node.type === "output" && node.params.path === probe);
const replacement = {
  ...speaks.spec,
  nodes: [...speaks.spec.nodes, { id: "probe", type: "constant", params: { value: { f32: 0.125 } } }],
  edges: [
    ...speaks.spec.edges.filter((edge) => edge.to.node_id !== probeNode.id),
    { from: { node_id: "probe" }, to: { node_id: probeNode.id, input: "in" } },
  ],
};

const { page, logs, close } = await open(FIXTURES);
try {
  await loadVizij(page, "face", "Quori_Current_Extended.glb", { ros4hri: false });
  const result = await page.evaluate(
    async ({ speaksId, liveId, speaksKeys, liveKeys, replacement }) => {
      const h = window.vizijHarness;
      const float = (value) => (value ? (value.f32 ?? value.float ?? null) : null);
      const read = (keys) =>
        Object.fromEntries(Object.entries(h.readValues("face", keys)).map(([k, v]) => [k, float(v)]));
      // Keys no program writes, set once and read after every change.
      const unrelatedKeys = ["vizij/test/unrelated", h.path("face", "standard/vizij/expression/happy")];
      h.setValue("face", unrelatedKeys[0], 0.375);
      h.setValue("face", unrelatedKeys[1], 0.5);
      // Speaks moves on its inputs: its idle motion, and the speech state.
      for (const input of ["mouth/twitch", "mouth/breathing", "eyes/explore", "eyes/jitter_amplitude", "eyes/jitter_speed", "brows/twitch"]) {
        h.setValue("face", h.path("face", `standard/vmotion/idle/${input}`), 1);
      }
      h.setValue("face", h.path("face", "speech/speaking"), 1);
      // Sampled over a second of device time, stepped here rather than left
      // to the page's frames (a software-rendered frame of a face can take
      // longer than that): the keys each program writes, and the unrelated
      // ones.
      const watch = () => {
        const samples = [];
        for (let i = 0; i < 10; i++) {
          h.step("face", 100);
          samples.push({ speaks: read(speaksKeys), live: read(liveKeys), unrelated: read(unrelatedKeys) });
        }
        return samples;
      };
      const states = () => ({ speaks: h.programState("face", speaksId), live: h.programState("face", liveId) });

      const atLoad = states();
      await h.startProgram("face", liveId);
      const together = { states: states(), samples: watch() };

      await h.stopProgram("face", liveId, { resetOutputs: true });
      const reset = read(liveKeys);
      const afterStop = { states: states(), reset, samples: watch() };

      await h.setProgram("face", speaksId, replacement);
      const afterReplace = { states: states(), samples: watch() };

      const unrelated = { [unrelatedKeys[0]]: 0.375, [unrelatedKeys[1]]: 0.5 };
      return { unrelated, atLoad, together, afterStop, afterReplace, unknown: h.programState("face", "nope") ?? null };
    },
    { speaksId: speaks.id, liveId: live.id, speaksKeys, liveKeys, replacement },
  );

  const varies = (samples, program, key) => new Set(samples.map((s) => s[program][key])).size > 1;
  const varying = (samples, program, keys) => keys.filter((key) => varies(samples, program, key));
  const unrelatedKept = (samples, phase) => {
    for (const { unrelated } of samples) {
      assert.deepEqual(unrelated, result.unrelated, `${phase}: the unrelated keys keep their values`);
    }
  };

  // At load the active program plays; the other waits.
  assert.deepEqual(result.atLoad, { speaks: "playing", live: "stopped" });
  assert.equal(result.unknown, null);

  // Both play together: every key of each is written, and keys of each move.
  const { together } = result;
  assert.deepEqual(together.states, { speaks: "playing", live: "playing" });
  for (const key of [...speaksKeys]) assert.notEqual(together.samples.at(-1).speaks[key], null, key);
  for (const key of [...liveKeys]) assert.notEqual(together.samples.at(-1).live[key], null, key);
  const speaksMoving = varying(together.samples, "speaks", speaksKeys);
  assert.ok(speaksMoving.length > 0, "Speaks moves");
  assert.ok(varying(together.samples, "live", liveKeys).length > 0, "Live moves");
  unrelatedKept(together.samples, "together");

  // Live stopped with a reset: its keys are at rest at once and stay there;
  // Speaks plays on.
  const { afterStop } = result;
  assert.deepEqual(afterStop.states, { speaks: "playing", live: "stopped" });
  for (const key of liveKeys) {
    const rest = liveRest[key];
    for (const value of [afterStop.reset[key], ...afterStop.samples.map((s) => s.live[key])]) {
      if (rest === null) assert.equal(value, null, `${key} rests unset`);
      else assert.ok(Math.abs(value - rest) < 1e-6, `${key} rests at ${rest}, reads ${value}`);
    }
  }
  assert.ok(varying(afterStop.samples, "speaks", speaksKeys).length > 0, "Speaks plays on");
  unrelatedKept(afterStop.samples, "after the stop");

  // Speaks replaced live: the probe key holds the new constant, its other
  // keys still move, Live stays at rest.
  const { afterReplace } = result;
  assert.deepEqual(afterReplace.states, { speaks: "playing", live: "stopped" });
  for (const { speaks: values } of afterReplace.samples) {
    assert.ok(Math.abs(values[probe] - 0.125) < 1e-6, `the replaced graph writes ${probe}: ${values[probe]}`);
  }
  const othersMoving = varying(
    afterReplace.samples,
    "speaks",
    speaksKeys.filter((key) => key !== probe),
  );
  assert.ok(othersMoving.length > 0, "the rest of the replaced program moves");
  for (const key of liveKeys) {
    assert.deepEqual(
      new Set(afterReplace.samples.map((s) => s.live[key])),
      new Set([afterStop.reset[key]]),
      `${key} stays at rest`,
    );
  }
  unrelatedKept(afterReplace.samples, "after the replacement");

  const state = await page.evaluate(() => window.vizijHarness.state());
  assert.deepEqual(state.stepErrors, []);
  console.log(
    `@vizij/runtime browser programs: ok (${speaksMoving.length}/${speaksKeys.length} Speaks keys moving, ` +
      `${liveKeys.length} Live keys reset)`,
  );
} catch (e) {
  console.error(logs.slice(-30).join("\n"));
  throw e;
} finally {
  await close();
}
