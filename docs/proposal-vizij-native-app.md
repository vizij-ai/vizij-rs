# Proposal: `vizij` — the native app (an arora with a head)

*Proposal for review ([VIZ-47](https://linear.app/semio-ai/issue/VIZ-47/vizij-crate-the-single-entry-point-cargo-run-shows-vizij-running-an)). The runtime facts below are current; the design is the intended end state.*

`cargo run` in vizij-rs opens the Vizij window: a native Bevy view rendering a
GLB face, driven by an arora device running the composed Vizij graphs — the
same runtime contract the web apps use, with no browser and no JS. With
`--headless` it renders offscreen and publishes frames into the data store;
with bridge flags the device's store is reachable over WebSocket, ROS 2, or
Semio Studio.

## What is already true

The runtime half of this app exists; only the head is new.

- **The native seams compile and are tested natively.** `vizij-arora-store`
  (`BlackboardStore`), `vizij-arora-hal` (`RigHal`), and `vizij-arora-behavior`
  (`ProcessingGraph`) have no wasm gating; only `vizij-arora-web` is
  `wasm32`-only, and it is a thin wrapper: its composition maps 1:1 onto
  `Arora::builder().with_hal(..).with_data_store(..)
  .with_behavior_interpreter(..).with_module(..)` + `Arora::run()/step()`.
- **The animation module needs no port.** It is a wasm artifact
  (`vizij-animation-module`) that arora-engine's wasmtime host executes
  natively — proven by `tests/host_ramp.rs`.
- **Graph edition is native-ready.** `ProcessingGraph` speaks `load` and
  `apply(GraphDiff)` (VIZ-79), reached through the device's caller — the same
  calls the wasm surface exposes as `loadGraph`/`applyGraphEdits`.
- **Bridges compose.** `AroraBuilder::with_bridge` is repeatable (fan-in reads,
  fan-out writes); arora auto-attaches the open local WS bridge
  (`ws://127.0.0.1:9000`) when none is injected; `arora-bridge-ros2` is a plain
  `Box<dyn Bridge>`. VIZ-47's original "mutually exclusive bridges" note
  predates this and is obsolete.
- **The process skeleton was validated once**: the unmerged `feat/vizij-binary`
  branch (arora 0.2 era) ran Bevy on the main thread with the arora device on a
  worker thread over a shared `RigHal`. Its view was a placeholder cube; the
  pattern carries over, the code does not.

## The head: what the view must render

Parity target is `@vizij/render` (vizij-web). The contract, extracted from the
web implementation and verified against the Quori and Toasty GLBs:

- **Binding model.** Faces are self-describing GLBs. Each scene node carries a
  `RobotData` glTF extension declaring its **animatables** — UUID-identified
  features. The embedded rig graph's `output` nodes write store paths that *are*
  those UUIDs; the view routes each changed `(namespace, uuid)` store value to
  one element property. Human-meaningful paths (`rig/{faceId}/…`) exist only on
  the input side; the view never parses them.
- **Property vocabulary.** Transforms (translation, rotation as ZYX Euler or
  scalar Z, scale), material color (RGB/HSL/vec3), opacity (drives alpha
  blending), PBR extras (roughness, metalness, shininess, specular, emissive),
  ellipse/rectangle fill+stroke, and **morph-target influences** — real faces
  use both layered planes *and* blendshapes (Quori: 7 morphing shapes; Toasty:
  15). No visibility, texture, or dynamic-text features exist.
- **Scene model.** Z-up, **orthographic** camera fit to the root's authored
  `rootBounds`, single ambient light, transparent background, sRGB output with
  **no tonemapping**, double-sided materials, layering by translation-Z, an
  optional safe-area outline.
- **Feed.** The view subscribes to the device store (`DataStore::subscribe` —
  the native twin of `drainChanges`), drops `arora/*` built-ins, and applies
  the rest. The step loop and the draw loop stay decoupled, exactly as on web.
- **Bundle.** `VIZIJ_bundle` sits on the scene's glTF `extensions`: graphs
  (`rig`, `pose-driver`, `motiongraph` programs), pose config (neutral inputs,
  poses, groups), baked animation clips, and metadata (`faceId`,
  `activeMotionGraphId` for autoplay).

## Design decisions

### D1 — Engine: Bevy, on the ros-viz-rs line

Bevy, `0.18` + `bevy_egui 0.39` to start — the exact stack
[ros-viz-rs](https://github.com/victorpaleologue/ros-viz-rs) ships on, whose
recipes this app copies directly:

- **Headless offscreen rendering is solved there** (`src/snapshot.rs`): disable
  `WinitPlugin`, drive with `ScheduleRunnerPlugin`, render the camera into a
  `RenderTarget` image, read pixels back with `bevy::render::gpu_readback`,
  encode PNG. Works from plain `#[test]`s; bounded readback waits fail fast
  without a GPU.
- **Packaging is solved there** too: `default-run` for a bare `cargo run`,
  cargo-deb/RPM metadata, Android via `bevy_android`.
- glTF loading with **morph targets** is first-class in Bevy (batched in 0.19,
  worth tracking); orthographic camera, ambient light, `Tonemapping::None`,
  alpha blend, and double-sided materials are all supported configuration.

Alternatives considered: **three-d** (lighter, but morph targets are not
first-class — the one non-negotiable feature); **raw wgpu** (reimplements glTF
materials and morphs for no benefit); **rend3/Fyrox** (maintenance/ecosystem).
The Bevy adapter crates removed by VIZ-63 were adapters for the pre-arora
standalone engines — orphaned by the architecture change, not evidence against
Bevy.

One caveat: Bevy's loader does not expose arbitrary per-node glTF *extensions*
(`RobotData`). The GLB is therefore parsed twice — Bevy loads the scene
(meshes, materials, morphs); the `gltf` crate reads `RobotData` +
`VIZIJ_bundle` from the same bytes; the two join on node index/name. One file
read, two parsers, no custom asset pipeline.

### D2 — One crate: `crates/vizij`

A single binary crate; no a-priori split. Internal modules, extracted into
crates only when a second consumer appears:

- `host` — parse `VIZIJ_bundle`, compose rig + pose + program sources into the
  one graph the device runs (the native port of runtime-react's composition),
  load animation clips into the animation module, own the transport calls.
- `world` — GLB → element tree + animatables table (the `RobotData` join).
- `view` — Bevy systems: store subscription → property application, camera,
  safe area.
- `ui` — egui (via bevy_egui): open a GLB (path/URL), input sliders built from
  the graph's input constraints, pose weights, animation/program transport,
  bridge endpoints + status, background color. This is `demo-vizij-player`'s
  panel surface, which is *richer* than vizij-standalone's.
- `headless` — the snapshot pipeline (D5).

`cargo run` must work bare at the workspace root: set workspace
`default-members = ["crates/vizij"]` (CI keeps building with `--workspace`).

### D3 — Composition fixes the pose-control alias at compose time

On web, compiled pose graphs emit internal `rig/{faceId}/pose/control/{input}`
outputs that the JS provider re-stages as rig inputs every tick. Natively the
alias is applied **at composition**: rewrite those output paths onto the
detected rig input paths when the sources are merged, so feedback flows through
the shared store with no per-tick host loop. (Candidate improvement to port
back to the web host later.)

### D4 — Bridges are flags, and they compose

The device owns its bridges through the builder:

- default: arora's open local WS bridge (`ws://127.0.0.1:9000`), as arora
  already auto-attaches;
- `--ros2 [namespace][:domain]` attaches `arora-bridge-ros2` (feature-gated,
  like vizij-web's ros2 flag);
- `--studio` (build feature `semio-studio-bridge`) attaches the Studio Zenoh
  bridge.

Any combination is valid. This supersedes VIZ-47's single-bridge constraint,
and it dissolves vizij-standalone's Tauri-event store-mirroring bridge
(VIZ-74): bridges attach to *the* device directly — there is no second store to
mirror.

### D5 — Headless: frames into the data store

`--headless` runs the same app without a window (ScheduleRunnerPlugin) and
publishes rendered frames into the device store, where any bridge can carry
them:

- path `view/frame`, value a record
  `{ width: u32, height: u32, format: text, data: ArrayU8 }` —
  `Value::ArrayU8` is already a first-class arora value;
- `--frame-format png|raw` (PNG default: raw RGBA is heavy over a bridge)
  and `--frame-rate <hz>` (decoupled from the device step rate);
- `--no-view` also exists: device only, no rendering at all — the pure
  headless-device mode.

This is the ROS4HRI path: a robot face rendered on-device with the frames
available as a topic, no browser anywhere.

### D6 — vizij-standalone is demoted, not deleted

The native app replaces standalone's substance: same runtime, better UI
(sliders included), direct bridges instead of the Tauri store mirror.
Standalone remains only as an installable wrapper while the native app's
packaging (cargo-deb/RPM/Android, per ros-viz-rs) catches up — then it retires.
No further investment.

## Out of scope (v1)

- Speech/viseme (bundle `speechConfig`) — extension after the head is stable.
- Authoring/editing UI — vizij-authoring remains the editor; the native app is
  a player/runtime host.
- A wasm build of the Bevy view — the web apps keep `@vizij/render`; nothing
  here forecloses converging later.

## Plan

1. **Skeleton** (revive VIZ-47 on arora 9): `crates/vizij` builds the device
   natively (RigHal + BlackboardStore + ProcessingGraph + animation module from
   the bindep artifact), default WS bridge, `cargo run` opens an empty Bevy
   window. Delete the untracked `devices/vizij-device` leftover.
2. **World + view**: GLB double-parse, element/animatables tables, store
   subscription → property application; Quori/Toasty render correctly against
   web-rendered references.
3. **Host**: bundle composition (rig+pose+programs), autoplay
   (`activeMotionGraphId`), animation clips + transport.
4. **UI**: egui panels (open, sliders, poses, transport, bridges, background).
5. **Headless**: snapshot pipeline → `view/frame` in the store; visual
   regression tests reuse the ros-viz-rs pattern.
6. **Bridges**: `--ros2`, `--studio`; then the standalone demotion note.

Each stage is a reviewable PR; tickets under VIZ-47 once this proposal is
agreed.
