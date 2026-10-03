// The registries the wrapper serves: profiles (interfaces — paths and their
// types) and mappings (graphs implementing one profile in terms of another),
// through the published surface (dist/), exactly as an authoring app uses it.
import assert from "node:assert/strict";
import {
  mapping,
  mappings,
  profile,
  profiles,
  standardProfile,
  standardProfiles,
} from "../dist/runtime/src/index.js";

// The shipped profiles, summarized: the face standard (face-scoped) and the
// ROS4HRI command interface (device-scoped — 5 named keys and one per action
// unit; the lipsync surface is the viseme players', not ROS4HRI's).
const listed = await profiles();
assert.deepEqual(
  listed.map((p) => [p.id, p.scope, p.keys]),
  [
    ["vizij-face", "face", 88],
    ["ros4hri", "device", 25],
  ],
);

// A face-scoped profile is addressed to the face: every path takes the prefix.
const face = await profile("vizij-face", "rig/quori/");
assert.equal(face.keys.length, 88);
assert.ok(face.keys.every((k) => k.path.startsWith("rig/quori/standard/vizij/")));
const jaw = face.keys.find((k) => k.path.endsWith("/face/jaw_open"));
assert.deepEqual(jaw.meta, { au: 26, arkit: "jawOpen", tier: "muscle" });
assert.equal(jaw.value_type, "f32");
assert.deepEqual(jaw.default_value, { f32: 0 });

// The blink, the expressions beyond ROS4HRI's and the conversation state are
// weights resting at zero, in their tiers.
for (const [suffix, tier] of [
  ["/blink", "gaze"],
  ["/expression/concerned", "expression"],
  ["/expression/sleepy", "expression"],
  ["/conversation/speaking", "conversation"],
  ["/conversation/user_speaking", "conversation"],
  ["/conversation/thinking", "conversation"],
]) {
  const key = face.keys.find((k) => k.path === `rig/quori/standard/vizij${suffix}`);
  assert.ok(key, `vizij-face declares ${suffix}`);
  assert.deepEqual(
    [key.kind, key.value_type, key.min, key.max, key.default_value, key.meta],
    ["input", "f32", 0, 1, { f32: 0 }, { tier }],
  );
}

// A device-scoped profile ignores the prefix: its paths are absolute.
const ros = await profile("ros4hri", "rig/quori/");
assert.ok(ros.keys.every((k) => k.path.startsWith("standard/ros4hri/")));
const target = ros.keys.find((k) => k.path === "standard/ros4hri/gaze/target");
assert.equal(target.value_type, "struct");

// The portable form, and an unknown id.
const portable = await profile("vizij-face");
assert.equal(portable.keys[0].path, "standard/vizij/left_eye/pos/x");
assert.equal(await profile("nope"), null);

// The shipped mappings, and the ROS4HRI graph with its outputs prefixed.
assert.deepEqual((await mappings()).map((m) => m.id), ["ros4hri"]);
const graph = await mapping("ros4hri", "rig/quori/");
const outputs = graph.nodes.filter((n) => n.type === "output");
assert.ok(outputs.length > 0);
assert.ok(outputs.every((n) => n.params.path.startsWith("rig/quori/standard/vizij/")));
const inputs = graph.nodes.filter((n) => n.type === "input");
assert.ok(inputs.every((n) => n.params.path.startsWith("standard/ros4hri/")));
assert.equal(await mapping("nope"), null);

// The deprecated names are the same functions.
assert.equal(standardProfiles, mappings);
assert.equal(standardProfile, mapping);

console.log("@vizij/runtime registries: ok");
