// An exported face GLB deploys through composeFace: the bundle's embedded
// (here: modified) standard profile composes and wins over the built-in —
// the VIZ-92 precedence, proven on the wasm runtime anywhere Node runs.
import assert from "node:assert/strict";
import { composeFace, startRuntime } from "../dist/runtime/src/index.js";

// A modified ros4hri copy: valence rides verbatim onto the happy weight (no
// smoothing, no blending, name ignored) — a mapping the built-in never
// produces.
const modifiedProfile = {
  nodes: [
    {
      id: "v",
      type: "input",
      params: { path: "standard/ros4hri/expression/valence", value: 0.0 },
    },
    {
      id: "o",
      type: "output",
      params: { path: "standard/vizij/expression/happy" },
    },
  ],
  edges: [{ from: { node_id: "v" }, to: { node_id: "o", input: "in" } }],
};
const gltf = {
  nodes: [
    {
      extensions: {
        VIZIJ_bundle: {
          version: 1,
          graphs: [
            {
              id: "standard::ros4hri",
              kind: "standard-profile",
              spec: modifiedProfile,
            },
          ],
        },
      },
    },
  ],
};

const spec = await composeFace(gltf, { program: "none" });
const ids = spec.nodes.map((node) => node.id);
assert.ok(
  ids.some((id) => id.startsWith("standard::ros4hri::")),
  "the embedded profile composes under its stable id",
);
assert.ok(
  !ids.some((id) => id.startsWith("ros4hri::")),
  "the built-in of the same id is suppressed",
);

// Deploy the composed face and drive it over the ROS4HRI keys.
const runtime = await startRuntime();
await runtime.loadGraph(spec);
runtime.setValue("standard/ros4hri/expression/name", { text: "sad" });
runtime.setValue("standard/ros4hri/expression/valence", { f32: 0.8 });
for (let i = 0; i < 5; i += 1) {
  runtime.step(16);
}
const happy = runtime.readValues(["standard/vizij/expression/happy"])[
  "standard/vizij/expression/happy"
];
// The built-in would have one-hotted "sad" (happy ≈ 0, smoothed); the
// embedded verbatim mapping ignores the name and rides valence through.
assert.ok(
  happy && Math.abs(happy.f32 - 0.8) < 1e-6,
  `embedded mapping wins, got ${JSON.stringify(happy)}`,
);

console.log("compose-face: ok");
