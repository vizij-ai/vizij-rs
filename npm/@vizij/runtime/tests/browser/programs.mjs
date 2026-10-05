// Two programs of one face running at once as runs of its interpreter: the
// one the face loads with halted, its outputs holding until the page returns
// them to rest, the other edited live — while a key neither program writes
// keeps its value throughout. Quori's bundle carries two motiongraphs:
// "Speaks", its active program, and "Live". The ROS4HRI mapping is left out
// so the programs alone write their keys (the mapping writes some of
// Speaks'; which of two writers of one key wins is VIZ-173's). Needs
// `VIZIJ_FIXTURES`; skips without.
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
  [...new Set(spec.nodes.filter((node) => node.type === "output").map((node) => node.params.path))].sort();
const speaksKeys = outputs(speaks.spec);
const liveKeys = outputs(live.spec);

// Where Speaks' keys rest: the rig input each drives rests at the neutral
// pose the face stages for it, else at its authored default.
const neutral = bundle.poses?.config?.neutralInputs ?? {};
const rig = bundle.graphs.find((graph) => graph.kind === "rig").spec;
const rest = {};
for (const node of rig.nodes) {
  if (node.type !== "input" || !speaksKeys.includes(node.params.path)) continue;
  const name = node.id.startsWith("input_") ? node.id.slice(6) : null;
  rest[node.params.path] =
    name !== null && name in neutral ? neutral[name] : (node.params.value?.f32 ?? node.params.value);
}
assert.equal(Object.keys(rest).length, speaksKeys.length, "Speaks writes rig inputs only");

// Live, edited: the pose weight it writes lifted by 5 rather than 0.5.
const weightKey = liveKeys.find((key) => key.endsWith("pose_d_neutral_d.weight"));
const operand = live.spec.nodes.find((node) => node.id.endsWith("_operand_1"));
assert.ok(weightKey && operand?.params.value.f32 === 0.5, "Live lifts the pose weight by 0.5");
const edited = {
  ...live.spec,
  nodes: live.spec.nodes.map((node) =>
    node === operand ? { ...node, params: { value: { f32: 5 } } } : node,
  ),
};

const { page, logs, close } = await open(FIXTURES);
try {
  await loadVizij(page, "face", "Quori_Current_Extended.glb", { ros4hri: false, audio: false });
  const result = await page.evaluate(
    async ({ speaksId, liveId, speaksKeys, liveKeys, edited }) => {
      const h = window.vizijHarness;
      const float = (value) => (value ? (value.f32 ?? value.float ?? null) : null);
      const read = (keys) =>
        Object.fromEntries(Object.entries(h.readValues("face", keys)).map(([k, v]) => [k, float(v)]));
      const unrelated = h.path("face", "standard/vizij/expression/happy");
      h.setValue("face", unrelated, 0.5);
      // Speaks moves on its inputs: its idle motion, and the speech state.
      for (const input of ["mouth/twitch", "mouth/breathing", "eyes/explore", "eyes/jitter_amplitude", "eyes/jitter_speed", "brows/twitch"]) {
        h.setValue("face", h.path("face", `standard/vmotion/idle/${input}`), 1);
      }
      h.setValue("face", h.path("face", "speech/speaking"), 1);
      // Sampled over a second of device time, stepped here rather than left
      // to the page's frames (a software-rendered frame of a face can take
      // longer than that).
      const watch = () => {
        const samples = [];
        for (let i = 0; i < 10; i++) {
          h.step("face", 100);
          samples.push({ speaks: read(speaksKeys), live: read(liveKeys), unrelated: read([unrelated])[unrelated] });
        }
        return samples;
      };
      const runs = () => Object.fromEntries(h.programRuns("face").map((run) => [run.name, run.status]));
      const settle = async (pending) => {
        h.step("face", 16);
        return pending;
      };

      h.step("face", 16);
      const atLoad = runs();
      const speaksHandle = h.programRuns("face").find((run) => run.name === speaksId).handle;
      const liveHandle = await settle(h.spawnProgram("face", liveId));
      const together = { runs: runs(), samples: watch() };

      await settle(h.halt("face", speaksHandle));
      const halted = { runs: runs(), samples: watch() };
      const outputs = h.programOutputs("face", speaksId);
      await h.reset("face", outputs);
      const rested = { outputs, samples: watch() };

      await settle(h.editProgram("face", liveHandle, liveId, edited));
      const afterEdit = { runs: runs(), samples: watch() };
      return { atLoad, together, halted, rested, afterEdit };
    },
    { speaksId: speaks.id, liveId: live.id, speaksKeys, liveKeys, edited },
  );

  const varies = (samples, program, key) => new Set(samples.map((s) => s[program][key])).size > 1;
  const varying = (samples, program, keys) => keys.filter((key) => varies(samples, program, key));
  const unrelatedKept = (samples, phase) => {
    for (const sample of samples) assert.equal(sample.unrelated, 0.5, `${phase}: the unrelated key keeps its value`);
  };

  // At load the active program runs, found by its id in the store.
  assert.deepEqual(result.atLoad, { [speaks.id]: "running" });

  // Both run together: every key of each is written, and keys of each move.
  const { together } = result;
  assert.deepEqual(together.runs, { [live.id]: "running", [speaks.id]: "running" });
  for (const key of speaksKeys) assert.notEqual(together.samples.at(-1).speaks[key], null, key);
  for (const key of liveKeys) assert.notEqual(together.samples.at(-1).live[key], null, key);
  const speaksMoving = varying(together.samples, "speaks", speaksKeys);
  assert.ok(speaksMoving.length > 0, "Speaks moves");
  assert.ok(varying(together.samples, "live", liveKeys).length > 0, "Live moves");
  unrelatedKept(together.samples, "together");

  // Speaks halted: its run reads failure, its outputs hold their last
  // values; Live runs on.
  const { halted } = result;
  assert.deepEqual(halted.runs, { [live.id]: "running", [speaks.id]: "failure" });
  const held = halted.samples[0].speaks;
  for (const sample of halted.samples) assert.deepEqual(sample.speaks, held, "a halted program's outputs hold");
  assert.ok(
    speaksKeys.some((key) => Math.abs(held[key] - rest[key]) > 1e-3),
    "Speaks left some of its keys away from rest",
  );
  assert.ok(varying(halted.samples, "live", liveKeys).length > 0, "Live runs on");
  unrelatedKept(halted.samples, "after the halt");

  // Returned to rest on the page's word: each of Speaks' outputs at its rest.
  const { rested } = result;
  assert.deepEqual(rested.outputs, speaksKeys);
  for (const sample of rested.samples) {
    for (const key of speaksKeys) {
      assert.ok(Math.abs(sample.speaks[key] - rest[key]) < 1e-6, `${key} rests at ${rest[key]}, reads ${sample.speaks[key]}`);
    }
  }
  unrelatedKept(rested.samples, "after the rest");

  // Live edited in place: the lifted weight shows, the rest of it moves on.
  const { afterEdit } = result;
  assert.deepEqual(afterEdit.runs, { [live.id]: "running", [speaks.id]: "failure" });
  for (const { live: values } of afterEdit.samples) {
    assert.ok(values[weightKey] > 4, `the edited graph writes ${weightKey}: ${values[weightKey]}`);
  }
  assert.ok(
    varying(afterEdit.samples, "live", liveKeys.filter((key) => key !== weightKey)).length > 0,
    "the rest of the edited program moves",
  );
  unrelatedKept(afterEdit.samples, "after the edit");

  const state = await page.evaluate(() => window.vizijHarness.state());
  assert.deepEqual(state.stepErrors, []);
  console.log(
    `@vizij/runtime browser programs: ok (${speaksMoving.length}/${speaksKeys.length} Speaks keys moving, ` +
      `${speaksKeys.length} rested, ${liveKeys.length} Live keys edited live)`,
  );
} catch (e) {
  console.error(logs.slice(-30).join("\n"));
  throw e;
} finally {
  await close();
}
