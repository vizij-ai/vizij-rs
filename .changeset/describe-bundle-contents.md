---
"@vizij/runtime": minor
---

`describe(glb)` returns what a face's bundle carries for an app to build its controls from, parsed once in the wasm: `poses` and `poseGroups` (each pose with its description, the ids of the groups it belongs to and its input values), `rigInputs` (each rig input's path relative to the rig prefix, label, group, default and range), `programLabels` (program id → label), `clips` (ids, names, durations in seconds, tracks of time-ordered keyframes) and `metadata`, the bundle's open-ended metadata as authored (`speechConfig`, `activeMotionGraphId`, …). `Pose`, `PoseGroup`, `RigInput`, `Clip`, `ClipTrack` and `ClipKeyframe` are exported types, and `VizijDescription` documents every field.
