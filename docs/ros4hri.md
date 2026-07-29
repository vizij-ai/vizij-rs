# ROS4HRI support

Vizij ships official support for [ROS4HRI](https://wiki.ros.org/hri), the ROS 2
human-robot-interaction standard, as a **built-in profile**: a graph that maps
the ROS4HRI face vocabulary onto the [Vizij face standard](face-standard.md), so
a ROS4HRI face command drives any compliant Vizij face.

This is **topic-level** support. Vizij consumes the ROS4HRI face-command
vocabulary (expressions, action units, gaze, and — as a Vizij extension —
visemes); it does not (yet) speak ROS4HRI's services or actions.

## How it fits together

```
ROS4HRI topics ──(a ROS bridge)──▶ standard/ros4hri/* store keys
                                          │
                                   ros4hri profile graph
                                          │
                                   standard/vizij/* controls
                                          │
                                   the face's own rig graph ──▶ morphs / bones
```

The profile is one layer in that chain: it reads the `standard/ros4hri/*` keys a
bridge writes and produces `standard/vizij/*` controls. It is
asset-independent — it only writes standard control paths; what an expression or
a viseme *looks like* stays with the face.

> **The ROS bridge that writes `standard/ros4hri/*` from live ROS 2 topics is
> the `arora-bridge-ros2` side, not this repo.** The ROS4HRI message vocabulary
> (`hri_msgs/Expression`, `FacialActionUnits`, `Gaze`, …) is available as typed
> ROS 2 messages in
> [`arora-msgs-ros2`](https://github.com/semio-ai/arora-sdk/tree/main/crates/arora-msgs-ros2);
> the typed-topic exposure that lands those onto these keys is a planned
> bridge feature. Until then, drive the keys directly (any behavior or test can
> write them) — the profile behaves identically regardless of who writes them.

## The `standard/ros4hri/*` key contract

What a bridge (or a test) writes:

| Key | Type | ROS4HRI source | Meaning |
|---|---|---|---|
| `standard/ros4hri/expression/name` | string | `hri_msgs/Expression.expression` | one-hots the named expression weight |
| `standard/ros4hri/expression/valence` | f32 `[-1,1]` | `hri_msgs/Expression.valence` | blends named weights by circumplex proximity when no name is set |
| `standard/ros4hri/expression/arousal` | f32 `[-1,1]` | `hri_msgs/Expression.arousal` | as above |
| `standard/ros4hri/gaze/target` | vec3 (m) | a look-at point (face frame: x forward, y left, z up) | per-eye gaze with vergence |
| `standard/ros4hri/au/<code>` | f32 `[0,1]` | `hri_msgs/FacialActionUnits` | FACS action-unit intensity → muscle controls |
| `standard/ros4hri/viseme/<shape>` | f32 `[0,1]` | *Vizij extension* (ROS4HRI has no viseme topic) | viseme weight, pass-through |

## Per-channel behaviour

- **Expression** — a non-empty `expression/name` one-hots the named weight.
  Otherwise `valence`/`arousal` blend the named weights by proximity to each
  expression's circumplex anchor. Weights are smoothed and written to
  `standard/vizij/expression/<name>`.
- **Gaze** — `gaze/target` maps to per-eye positions with vergence, the
  incumbent ±0.78 rad → ±1 normalization, and a center fallback for targets at
  or behind the face plane (x ≤ 0.1 m).
- **Action units** — `au/<code>` intensities route to the muscle-tier controls
  ([`FACE_CONTROLS`](face-standard.md#muscle-tier)); the eyes-closed unit also
  drives the eyelids, and jaw-open additionally drives the de-facto
  `mouth/morph/jaw_open` control.
- **Visemes** — `viseme/<shape>` weights pass through, smoothed, to
  `standard/vizij/viseme/<shape>`. ROS4HRI defines no viseme channel; this is a
  Vizij extension fed by a lipsync source.
- **Blink** — an idle generator (≈8 s cycle, deterministically jittered, 0.2 s
  parabolic pulse) drives the eyelids, inhibited while the eyes are commanded
  closed or the face is asleep.

All continuous channels pass through a ~200 ms exponential smoother — the
incumbent ROS4HRI face's dynamics.

## Enabling it

The profile composes between a face's own graphs and any playing program, so a
performance overrides it (last-writer-wins).

- **From the `vizij` binary** it is **on by default**; opt out with
  `--no-ros4hri`.
- **From a library** it is opt-in: `vizij_arora_host::ros4hri::ros4hri_source(rig_prefix)`
  returns the composable graph source, or embed it into a face GLB so it travels
  with the asset (see below).

## Embedding and editing the profile

The profile graph is a canonical JSON asset,
[`profiles/ros4hri.json`](../crates/interop/vizij-arora-host/profiles/ros4hri.json).
Rust reads it (and regenerates it from the node-graph builder; a test fails if
the committed file drifts), so it is programmatically available *and* separately
editable and exportable.

- **Bundle it into a face** with
  [`vizij-bundle`](../crates/tools/vizij-bundle/README.md):
  `vizij-bundle add-standard face.glb --standard ros4hri -o out.glb` grafts the
  profile under a stable id (`standard::ros4hri`), re-runnable to update in
  place.
- **From the web** the same asset is served by
  [`@vizij/runtime`](../npm/@vizij/runtime/README.md): `standardProfiles()`
  lists the shipped profiles (the introspectable menu of what a face may opt
  into), and `standardProfile("ros4hri", rigPrefix)` returns the profile graph
  as an object for an authoring app to embed.

## Progressive compliance

A face implements the standard tiers it covers, and the profile degrades to
them: gaze & lids (L0), expressions (L1), visemes (L2), muscle/AU (L3).
`vizij-bundle validate --min-level <n>` reports and gates a face's coverage.

## See also

- [The Vizij face standard](face-standard.md) — the target vocabulary.
- [`vizij-bundle`](../crates/tools/vizij-bundle/README.md) — bundle, validate,
  and export profiles.
- [`arora-msgs-ros2`](https://github.com/semio-ai/arora-sdk/tree/main/crates/arora-msgs-ros2) —
  the ROS4HRI (`hri_msgs`) message vocabulary as typed ROS 2 messages.
