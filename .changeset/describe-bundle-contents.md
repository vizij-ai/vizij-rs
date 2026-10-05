---
"@vizij/runtime": minor
---

`describe(glb)` returns what a face's bundle carries for an app to build its controls from, parsed once in the wasm: `poses` and `poseGroups` (each pose with its description, the ids of the groups it belongs to and its input values), `rigInputs` (each rig input's path relative to the rig prefix, label, group, default and range), `programs` (each program's id, label and graph, as authored), `animations` (ids, names, durations in seconds, tracks of time-ordered keyframes) and `metadata`, the bundle's open-ended metadata as authored (`speechConfig`, `activeMotionGraphId`, …). `Program`, `Pose`, `PoseGroup`, `RigInput`, `Animation`, `AnimationTrack` and `AnimationKeyframe` are exported types (`Animation` is `@vizij/runtime`'s, not the DOM's Web Animations `Animation`), and `VizijDescription` documents every field.
