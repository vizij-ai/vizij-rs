import fs from "fs";
import path from "path";

const moduleDir = path.dirname(new URL(import.meta.url).pathname);
const modulePath = path.join(moduleDir, "dist/src/index.js");
const { init, createGraph, normalizeGraphSpec } = await import(modulePath);

const specPath =
  process.argv[2] ?? "/home/chris/Code/Semio/vizij_ws/q-short_emotion_rig (2).json";
const spec = JSON.parse(fs.readFileSync(specPath, "utf8"));

await init();
const normalized = await normalizeGraphSpec(spec);
const graph = await createGraph(normalized);

const neutralResult = graph.evalAll();
const neutralBlend = neutralResult.nodes.pose_blend.out.value;
console.log(
  "Initial pose_blend output:",
  JSON.stringify(neutralBlend, null, 2),
);

const stagedWeights = [
  { path: "rig/quori-sample/poses/full.weight", value: { float: 0 } },
  { path: "rig/quori-sample/poses/empty.weight", value: { float: 0 } },
  { path: "rig/quori-sample/poses/cross.weight", value: { float: 1 } },
];
stagedWeights.forEach(({ path: posePath, value }) => {
  graph.stageInput(posePath, value);
});

const blendedResult = graph.evalAll();
const blendedBlend = blendedResult.nodes.pose_blend.out.value;
console.log(
  "Staged weights pose_blend output:",
  JSON.stringify(blendedBlend, null, 2),
);

function extractScalar(record, key) {
  const slot = record?.record?.values?.record?.[key];
  return typeof slot?.float === "number" ? slot.float : null;
}

const channels = [
  "left_eye_pos_x",
  "left_eye_pos_y",
  "right_eye_pos_x",
  "right_eye_pos_y",
];
channels.forEach((channel) => {
  const value = extractScalar(blendedBlend, channel);
  if (value === null) {
    throw new Error(`Channel ${channel} returned null after blending`);
  }
  console.log(`pose_blend ${channel}:`, value.toFixed(6));
});

const writes = blendedResult.writes ?? [];
console.log(
  "Output writes:",
  writes.map((entry) => ({
    path: entry.path,
    value: entry.value,
  })),
);
