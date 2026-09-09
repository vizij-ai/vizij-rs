# ROS4HRI support

Vizij ships official support for [ROS4HRI](https://wiki.ros.org/hri), the ROS 2
human-robot-interaction standard, as a **profile and a mapping** (see
[profiles and mappings](profiles-and-mappings.md)): the `ros4hri` profile
declares the face-command interface — the `standard/ros4hri/*` keys and their
types — and the ROS4HRI mapping is the graph that implements it in terms of
the [Vizij face standard](face-standard.md), so a ROS4HRI face command drives
any compliant Vizij face.

Support spans both ROS4HRI planes:

- **Topics** — the face-command vocabulary (expressions, action units, gaze)
  lands on the `ros4hri` profile's `standard/ros4hri/*` keys through typed
  topic endpoints.
- **The skill plane** — the device serves ROS4HRI's gaze skill as a native
  ROS 2 action server, [`/skill/look_at`](#the-look_at-skill)
  (`interaction_skills/LookAt` — the standard's only action; `set_expression`
  is, per the standard, a topic).

## How it fits together

```
ROS4HRI topics ──(typed bridge endpoints)──▶ standard/ros4hri/* store keys   (the ros4hri profile)
                                                    │
                                             the ros4hri mapping graph
                                                    │
                                             standard/vizij/* controls        (the vizij-face profile)
                                                    │
                                             the face's own rig graph ──▶ morphs / bones

/skill/look_at action ──(bridge skill plane)──▶ look_at task run ──▶ standard/ros4hri/gaze/* keys
```

The mapping is one layer in that chain: it consumes the `ros4hri` profile — the
`standard/ros4hri/*` keys a bridge writes — and produces the `vizij-face`
profile, the `standard/vizij/*` controls. It is asset-independent — it only
writes standard control paths; what an expression or a viseme *looks like*
stays with the face.

> **The ROS side lives in
> [`arora-bridge-ros2`](https://github.com/semio-ai/arora-sdk/tree/main/crates/arora-bridge-ros2),
> not this repo.** Its `ExposureProfile::ros4hri()` preset subscribes the typed
> face topics — PAL's `/robot_face/{expression,look_at,tts}` and IIIA's
> `/expressive_face/{look_at,speech}` — and routes their fields onto these keys,
> and binds the [`/skill/look_at`](#the-look_at-skill) action. The `vizij`
> binary wires the preset automatically when run with `--ros2`. The message
> vocabulary (`hri_msgs`, `geometry_msgs`, `interaction_skills`, …) ships as
> typed ROS 2 messages in
> [`arora-msgs-ros2`](https://github.com/semio-ai/arora-sdk/tree/main/crates/arora-msgs-ros2).
> Without a bridge, drive the keys directly (any behavior or test can write
> them) — the mapping behaves identically regardless of who writes them.

## The `standard/ros4hri/*` key contract

The `ros4hri` profile — what a bridge (or a test) writes. Device-scoped: one
instance per device, shared by every face. The typed declaration is
[`profiles/ros4hri.json`](../crates/interop/vizij-arora-host/profiles/ros4hri.json)
(`vizij-bundle profiles` lists it, `@vizij/runtime`'s `profile("ros4hri")`
serves it); this table summarizes it.

| Key | Type | ROS4HRI source | Meaning |
|---|---|---|---|
| `standard/ros4hri/expression/name` | string | `hri_msgs/Expression.expression` | one-hots the named expression weight |
| `standard/ros4hri/expression/valence` | f32 `[-1,1]` | `hri_msgs/Expression.valence` | blends named weights by circumplex proximity when no name is set |
| `standard/ros4hri/expression/arousal` | f32 `[-1,1]` | `hri_msgs/Expression.arousal` | as above |
| `standard/ros4hri/gaze/target` | vec3 (m) | a look-at point (face frame: x forward, y left, z up) | per-eye gaze with vergence |
| `standard/ros4hri/gaze/frame` | string | the look-at point's frame id | consumed by the `look_at` skill, not the mapping |
| `standard/ros4hri/au/<code>` | f32 `[0,1]` | `hri_msgs/FacialActionUnits` | FACS action-unit intensity → muscle controls |
| `standard/ros4hri/speech/text` | string | `/robot_face/tts`, `/expressive_face/speech` | the utterance to lip-sync — **nothing consumes it yet** (see Lips below) |

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
- **Lips** — not the mapping's: ROS4HRI defines no viseme channel, and the
  face's lipsync is the viseme players' ([skills](skills.md): `play_viseme`,
  `say`), which write the face standard's viseme weights themselves. The
  standard's lipsync input is a *text* topic, so `speech/text` is a
  speech-synthesis request, not a face command: turning it into lip motion
  means spawning a `say` run for the text. Nothing does that today — the key
  is written and never read, so publishing on `/robot_face/tts` moves no
  mouth.
- **Blink** — an idle generator (≈8 s cycle, deterministically jittered, 0.2 s
  parabolic pulse) drives the eyelids, inhibited while the eyes are commanded
  closed or the face is asleep.

All continuous channels pass through a ~200 ms exponential smoother — the
incumbent ROS4HRI face's dynamics.

## Driving a key from ROS 2

With the bridge attached (`vizij --ros2 <namespace>`), every `standard/ros4hri/*`
key the mapping reads and nothing in the composition writes is a **free
input**, subscribed at `/<namespace>/keys/<path>` as the `std_msgs` type of
its default value (`Float64` for the weights). The keys are **device-global**:
the mapping prefixes only what it *writes* (`rig/<faceId>/standard/vizij/…`),
never what it reads, so the topic carries no face id. The store keeps the last
value, so one message holds a shape until the next one.

Open the jaw through the jaw-open action unit (AU 26) and hold it, then
release it (the device runs as `vizij --ros2 quori`; `--once` waits for the
device's subscription to match, then sends one message):

```bash
ros2 topic pub --once /quori/keys/standard/ros4hri/au/26 \
  std_msgs/msg/Float64 "{data: 1.0}"
ros2 topic pub --once /quori/keys/standard/ros4hri/au/26 \
  std_msgs/msg/Float64 "{data: 0.0}"
```

What happens on the device: the mapping's exponential smoother (half-life
0.14 s — a ≈200 ms time constant) carries the intensity to the muscle-tier
control `rig/<faceId>/standard/vizij/face/jaw_open` (and to the de-facto
`mouth/morph/jaw_open`), and the face's own graphs map that onto its rig. A
face without the muscle tier moves nothing — `vizij-bundle validate` reports
which tiers a face covers. The same goes for the typed topics: publishing
`hri_msgs/Expression` on `/robot_face/expression` lands on
`standard/ros4hri/expression/*`.

The lips are not driven this way: a viseme is a played thing, not a level,
so the mouth shapes belong to the viseme players ([skills](skills.md)).
Their state is a published key: while a `play_viseme` or `say` run drives
the lips, `rig/<faceId>/standard/vizij/viseme` carries the current shape.

Watch it land: the device publishes every key it writes on the same plane,
so the face's controls and, headless, its rendered frames are topics too:

```bash
ros2 topic echo /quori/keys/rig/quori_latest/pose/control/propsrig_mouth_jawud_value
ros2 topic echo /quori/keys/rig/quori_latest/standard/vizij/viseme
ros2 topic hz /quori/keys/view/frame
```

Scalar keys publish as the `std_msgs` type of their value (the face's
controls are `Float32`). The frame (`--headless --frame-rate 2`) is a record,
so it rides the bridge's non-scalar fallback: a `std_msgs/String` carrying
the value's canonical JSON — a `keyvalue` whose `width`, `height`, `format`
(`"png"`) and `data` fields hold the PNG bytes under `u8s`.

### With rmw_zenoh

The `ros2-zenoh` build feature swaps the bridge's RMW backend for the
rmw_zenoh-compatible one. Like rmw_zenoh itself it needs a running router and
reads the same environment: point the device at the router and it joins the
ROS graph of every rmw_zenoh node connected there.

```bash
# the ROS side (any host, e.g. a container with port 7447 published)
export RMW_IMPLEMENTATION=rmw_zenoh_cpp
ros2 run rmw_zenoh_cpp rmw_zenohd &

# the device
ZENOH_CONFIG_OVERRIDE='mode="client";connect/endpoints=["tcp/127.0.0.1:7447"]' \
  vizij --glb face.glb --headless --ros2 quori --frame-rate 2
```

## The `look_at` skill

The gaze skill is one of the device's [skills](skills.md); this section is
its ROS4HRI binding.

With the ROS 2 bridge attached, the device serves ROS4HRI's gaze skill as a
standard ROS 2 action server on **`/skill/look_at`**
(`interaction_skills/LookAt`). The goal is the standard's:
`meta` (`std_skills/Meta`: caller + priority), `policy`, and `target`
(`geometry_msgs/PointStamped`).

```bash
ros2 action send_goal /skill/look_at interaction_skills/action/LookAt \
  "{meta: {priority: 128}, policy: '', target: {header: {frame_id: face}, point: {x: 1.0, y: 0.3, z: 0.1}}}"
```

| Policy | Behaviour |
|---|---|
| *(empty)* | track `target` continuously; runs until cancelled or preempted |
| `glance` | look at `target`, then succeed once the gaze settles (~0.6 s dwell) |
| `reset` | return the gaze to rest, then succeed once settled |
| `random`, `social`, `auto` | not implemented — the goal aborts with `ROS_ENOTSUP` (134) |

Semantics, per the standard:

- **One active goal per skill.** A new goal with `meta.priority` at or above
  the active one **preempts it** (the preempted goal returns `ROS_EINTR`, 4);
  a lower-priority goal is rejected at `send_goal`.
- **Cancel** halts the run; the result carries `ROS_ECANCELED` (125). Success
  is `ROS_ENOERR` (0).
- To move a tracked target, send a new goal at the same (or higher) priority:
  it replaces the active one, which returns `ROS_EINTR`.

The behavior itself is not compiled in — it is a **graph fragment asset**
([`skills/look_at.json`](../crates/interop/vizij-arora-host/skills/look_at.json)
in `vizij-arora-host`), grafted into the device's graph per run. Like the
mapping, it is canonical JSON: regenerable (`vizij-bundle export-skill
look_at`), drift-tested, and **overridable per face** — a face GLB embedding a
`skill::look_at` graph entry runs its own copy instead of the built-in.
[`@vizij/runtime`](../npm/@vizij/runtime/README.md) exposes the registry
(`skills()`, `skillSource(id)`), and the vizij-web authoring app embeds and
edits skill fragments from **File → Skills**.

## Enabling the mapping

The mapping composes between a face's own graphs and any playing program, so a
performance overrides it (last-writer-wins).

- **From the `vizij` binary** it is **on by default**; opt out with
  `--no-ros4hri`.
- **From a library** it is opt-in: `vizij_arora_host::ros4hri::ros4hri_source(rig_prefix)`
  returns the composable graph source, or embed it into a face GLB so it travels
  with the asset (see below).
- **A face that embeds its own copy** (`standard::ros4hri`, see below) runs
  that copy instead: the embedded mapping is the author's pinned override of
  the shipped one, always composed, and the built-in of the same id is not
  composed for that face. Other mappings are unaffected — the suppression is
  per mapping id.

## Embedding and editing the mapping

The mapping graph is a canonical JSON asset,
[`mappings/ros4hri.json`](../crates/interop/vizij-arora-host/mappings/ros4hri.json).
Rust reads it (and regenerates it from the node-graph builder; a test fails if
the committed file drifts), so it is programmatically available *and* separately
editable and exportable.

The asset reads by channel. Node ids are hierarchical: `in/…` are the
`ros4hri` profile's keys it consumes, `out/…` the `vizij-face` controls it
produces (named by control, `out/expression/happy`, `out/left_eye/pos/x`),
and `expression/…`, `gaze/…`, `au/…`, `viseme/…`, `blink/…` the computation
of each channel — `expression/happy/kernel` is the circumplex weight of the
happy anchor, `gaze/left/yaw/atan` the left eye's yaw before normalization,
`blink/pulse` the idle pulse. Scalar constants are shared and named for their
value (`const/0.28`). Every node's id says what it holds; a diff of the asset
is a diff of the behavior.

- **Bundle it into a face** with
  [`vizij-bundle`](../crates/tools/vizij-bundle/README.md):
  `vizij-bundle add-standard face.glb --standard ros4hri -o out.glb` grafts the
  mapping under a stable id (`standard::ros4hri`), re-runnable to update in
  place. (The entry's `kind` is still spelled `standard-profile` on disk — see
  [profiles and mappings](profiles-and-mappings.md#mappings).)
- **From the web** the same asset is served by
  [`@vizij/runtime`](../npm/@vizij/runtime/README.md): `mappings()` lists the
  shipped mappings (the introspectable menu of what a face may opt into), and
  `mapping("ros4hri", rigPrefix)` returns the graph as an object for an
  authoring app to embed.
- **Reconcile it against the profiles** with `vizij-bundle surface`: the
  mapping reads 24 of the profile's 25 keys (`gaze/frame` belongs to the
  `look_at` skill) and writes 64 of the face standard's 81 (`jaw_left` and
  `jaw_right` have no FACS code, and the 15 viseme weights belong to the
  players) plus the de-facto `standard/vizij/mouth/morph/jaw_open`.

## Progressive compliance

A face implements the standard tiers it covers, and the mapping degrades to
them: gaze & lids (L0), expressions (L1), visemes (L2), muscle/AU (L3).
`vizij-bundle validate --min-level <n>` reports and gates a face's coverage of
the `vizij-face` profile.

## See also

- [Profiles and mappings](profiles-and-mappings.md) — the model this page
  instantiates.
- [The Vizij face standard](face-standard.md) — the `vizij-face` profile the
  mapping produces.
- [`vizij-bundle`](../crates/tools/vizij-bundle/README.md) — bundle, validate,
  and export profiles and mappings.
- [`arora-bridge-ros2`](https://github.com/semio-ai/arora-sdk/tree/main/crates/arora-bridge-ros2) —
  the bridge serving the typed endpoints and the skill plane
  (`ExposureProfile`, `ActionBinding`).
- [`arora-msgs-ros2`](https://github.com/semio-ai/arora-sdk/tree/main/crates/arora-msgs-ros2) —
  the ROS4HRI (`hri_msgs`, `interaction_skills`) message vocabulary as typed
  ROS 2 messages.
