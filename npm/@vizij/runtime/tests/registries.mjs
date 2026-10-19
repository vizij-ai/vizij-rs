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

// The shipped profiles, summarized: the face standard (face-scoped — 91
// controls, the face's three settings, the caption and its speaker, and the
// two speech state keys it reports) and
// the ROS4HRI interface (device-scoped — 10 named command keys, the streamed
// viseme among them as one code rather than a weight per shape, with its time
// and duration, and the closed caption's text and speaker; and the speech
// state it reports, the utterance and its current viseme).
const listed = await profiles();
assert.deepEqual(
  listed.map((p) => [p.id, p.scope, p.keys]),
  [
    ["vizij-face", "face", 98],
    ["ros4hri", "device", 12],
  ],
);

// A face-scoped profile is addressed to the face: every path takes the prefix.
const face = await profile("vizij-face", "rig/quori/");
assert.equal(face.keys.length, 98);
assert.ok(face.keys.every((k) => k.path.startsWith("rig/quori/standard/vizij/")));
// What the face reports rather than takes: the current viseme and the
// utterance being spoken, output keys outside the control tiers.
assert.deepEqual(
  face.keys.filter((k) => k.kind === "output").map((k) => k.path),
  ["rig/quori/standard/vizij/viseme", "rig/quori/standard/vizij/speech"],
);
// The face's settings, outside the tiers: the lipsync gain its adaptation
// scales the viseme weights by, resting at 1, and how far apart its eyes are
// and how far they turn, which the ROS4HRI mapping aims them with, resting at
// the incumbent's 6 cm and 0.78 rad, the range within [0.01, 0.9].
for (const [suffix, rest, min, max] of [
  ["lipsync/gain", 1, 0, undefined],
  ["gaze/eye_spacing", 0.06, 0, undefined],
  ["gaze/range", 0.78, 0.01, 0.9],
]) {
  const setting = face.keys.find((k) => k.path === `rig/quori/standard/vizij/${suffix}`);
  assert.ok(setting, `vizij-face declares ${suffix}`);
  assert.deepEqual(
    [setting.kind, setting.value_type, setting.min, setting.max, setting.meta],
    ["input", "f32", min, max, undefined],
  );
  assert.ok(Math.abs(setting.default_value.f32 - rest) < 1e-6, `${suffix} rests at ${rest}`);
}
const jaw = face.keys.find((k) => k.path.endsWith("/face/jaw_open"));
assert.deepEqual(jaw.meta, { au: 26, arkit: "jawOpen", tier: "muscle" });
assert.equal(jaw.value_type, "f32");
assert.deepEqual(jaw.default_value, { f32: 0 });

// The blink, the gaze target's weight, the expressions beyond ROS4HRI's and
// the conversation state are weights resting at zero, in their tiers.
for (const [suffix, tier] of [
  ["/blink", "gaze"],
  ["/gaze/target/weight", "gaze"],
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

// The gaze target is a point on the face's frame, resting at its centre.
for (const axis of ["x", "y"]) {
  const key = face.keys.find((k) => k.path === `rig/quori/standard/vizij/gaze/target/${axis}`);
  assert.ok(key, `vizij-face declares the gaze target's ${axis}`);
  assert.deepEqual(
    [key.kind, key.value_type, key.min, key.max, key.default_value, key.meta],
    ["input", "f32", 0, 1, { f32: 0.5 }, { tier: "gaze" }],
  );
}

// A device-scoped profile ignores the prefix: its paths are absolute.
const ros = await profile("ros4hri", "rig/quori/");
assert.ok(ros.keys.every((k) => k.path.startsWith("standard/ros4hri/")));
const target = ros.keys.find((k) => k.path === "standard/ros4hri/gaze/target");
assert.equal(target.value_type, "struct");
assert.deepEqual(
  ros.keys.filter((k) => k.kind === "output").map((k) => k.path),
  ["standard/ros4hri/speech/text", "standard/ros4hri/speech/viseme"],
);

// The portable form, and an unknown id.
const portable = await profile("vizij-face");
assert.equal(portable.keys[0].path, "standard/vizij/left_eye/pos/x");
assert.equal(await profile("nope"), null);

// The shipped mappings, and the ROS4HRI graph with the face's paths prefixed:
// it reads the device-scoped ROS4HRI commands and writes the face's controls,
// the speech channel runs the other way — the face's speech state in, the
// ROS4HRI speech state out — and the gaze aims the eyes with the face's eye
// settings, the prefix landing on the face's side only.
assert.deepEqual((await mappings()).map((m) => m.id), ["ros4hri"]);
const graph = await mapping("ros4hri", "rig/quori/");
const outputs = graph.nodes.filter((n) => n.type === "output").map((n) => n.params.path);
assert.ok(outputs.length > 0);
assert.ok(
  outputs.every(
    (p) => p.startsWith("rig/quori/standard/vizij/") || p.startsWith("standard/ros4hri/speech/"),
  ),
);
assert.ok(outputs.includes("standard/ros4hri/speech/text"));
assert.ok(outputs.includes("standard/ros4hri/speech/viseme"));
const inputs = graph.nodes.filter((n) => n.type === "input").map((n) => n.params.path);
assert.ok(
  inputs.every(
    (p) =>
      p.startsWith("standard/ros4hri/") ||
      p === "rig/quori/standard/vizij/speech" ||
      p === "rig/quori/standard/vizij/viseme" ||
      p === "rig/quori/standard/vizij/gaze/eye_spacing" ||
      p === "rig/quori/standard/vizij/gaze/range",
  ),
);
for (const path of ["speech", "viseme", "gaze/eye_spacing", "gaze/range"]) {
  assert.ok(inputs.includes(`rig/quori/standard/vizij/${path}`), `the mapping reads ${path}`);
}
assert.equal(await mapping("nope"), null);

// The deprecated names are the same functions.
assert.equal(standardProfiles, mappings);
assert.equal(standardProfile, mapping);

console.log("@vizij/runtime registries: ok");
