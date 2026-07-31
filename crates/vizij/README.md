# vizij — the native app

An arora with a head. `cargo run -p vizij -- --glb <face.glb>` opens a Bevy
window rendering the face, driven by a natively-run arora device executing the
face's own graphs (rig + pose-driver from the embedded `VIZIJ_bundle`) — the
same runtime contract the web apps use, with no browser and no JS.

```bash
cargo run -p vizij -- --glb path/to/Quori_Current_Extended.glb
# headless: render one frame offscreen and exit
cargo run -p vizij -- --glb face.glb --snapshot out.png --size 763x760
```

## How it works

- **`meta`** reads what Bevy's GLB loader does not surface: the per-node
  `RobotData` extension (the animatables — UUID-identified features) and the
  scene-root `VIZIJ_bundle` (graphs, poses, clips, metadata). Bevy loads the
  same GLB for meshes/materials/morphs; the two worlds join on the glTF node
  name.
- **`device`** composes the bundle's graphs into one spec (node ids namespaced
  per source, store paths shared — the cross-source contract) and runs
  `RigHal` + `BlackboardStore` + `ProcessingGraph` as an arora, stepped at
  ~100 Hz on a worker thread. The `Arora` is built inside that thread — it is
  single-owner by design and not `Send`.
- **`view`** renders the web renderer's scene model: Z-up, faces in the XY
  plane layered along Z, orthographic camera fit to the authored `rootBounds`,
  sRGB output, no tonemapping, double-sided materials, opacity-driven alpha,
  morph-target influences. Each frame it reads the device's actuation state
  from the HAL seam (`RigHal::pose()`) and applies it: transforms (euler ZYX),
  material color/opacity, morphs.
- **`snapshot`** is the headless pipeline (recipe from ros-viz-rs):
  `WinitPlugin` disabled, `ScheduleRunnerPlugin`, camera → `RenderTarget`
  image, `gpu_readback` → PNG.

## Flags

`cargo run -p vizij -- --glb <face.glb>` plus:

| Flag | Default | Effect |
|---|---|---|
| `--glb <path>` | required | the face GLB (embedded `RobotData` + `VIZIJ_bundle`) |
| `--graphs <kinds>` | `rig,pose-driver,pose,standard-adaptation` | compose only these bundle graph kinds |
| `--no-ros4hri` | off (profile **on**) | drop the built-in [ROS4HRI](../../docs/ros4hri.md) profile |
| `--program <id>` | bundle's active program | autoplay this motiongraph program |
| `--no-autoplay` | off | hold the rig's authored/neutral pose |
| `--no-stage-neutral` | off | don't stage the bundle's `neutralInputs` at boot |
| `--snapshot <png>` | — | render one frame offscreen and exit (no window) |
| `--headless` | off | run windowless, streaming frames into the store |
| `--size WxH` | `763x486` | offscreen render size (`--snapshot` / `--headless`) |
| `--frame-rate <hz>` | `15` | publish rendered frames as HAL `view/frame` readings; 0 disables |
| `--frame-format <fmt>` | `png` | encoding of published frames |
| `--background <rrggbb>` | `000000` | clear color |
| `--ambient <f>` | `π/2` | three.js-style ambient intensity |
| `--unlit` | off | render materials unlit (albedo passthrough) |
| `--fit <contain\|cover>` | `contain` | how the face fits the window |

The ROS 2 and Studio bridges are build features (they compose with arora's local
WS bridge):

| Flag | Feature | Effect |
|---|---|---|
| `--ros2 [namespace][:domain]` | `ros2` | join the ROS graph as a ROS4HRI face (see below) |
| `--studio` | `studio` | attach the Semio Studio bridge (configured from the environment) |

`--ros2` attaches [`arora-bridge-ros2`](https://github.com/semio-ai/arora-sdk/tree/main/crates/arora-bridge-ros2)
with its ROS4HRI exposure preset:

- the typed face topics — `/robot_face/{expression,look_at,tts}` and
  `/expressive_face/{look_at,speech}` — routed onto the profile's
  `standard/ros4hri/*` keys;
- the **`/skill/look_at`** action server (`interaction_skills/LookAt`):
  track / glance / reset policies, priority preemption, standard error codes;
- every store key as a data topic under `/<namespace>/keys/<path>`.

[ROS4HRI support](../../docs/ros4hri.md) documents the key contract, the
per-channel behavior, and the skill's semantics.

## Speech (TTS)

The device registers a **`say(text, voice) → Status`** action that synthesizes
speech, plays it (rodio — pure Rust, nothing to install), and streams the
viseme at the audio playhead (a mutable out-parameter) — the face's lipsync
source. Two interchangeable providers share that one contract; a build carries
exactly one. Try either from the command line:

```bash
cargo run -p vizij --example say -- "Hello, world!"
cargo run -p vizij --features tts-piper --example say -- "Hello, world!"  # local Piper
```

**Piper — local, no credentials (`tts-piper`):**

```bash
cargo run -p vizij --features tts-piper -- --glb path/to/face.glb
```

The first build is the whole setup: `vizij-piper`'s build script provisions
everything itself — cmake-builds libpiper (espeak-ng + onnxruntime) from a
pinned commit and downloads + alignment-patches the default voice
(`en_US-lessac-medium`) — cached in `~/.cache/vizij-piper` (override:
`VIZIJ_PIPER_CACHE`), so it survives `cargo clean` and only needs the network
once. Build-time prerequisite: `cmake` and a C++ toolchain; runtime: nothing.
Pick another Piper voice at run time with `PIPER_VOICE` / `PIPER_VOICE_CONFIG`
(and `PIPER_ESPEAK_DATA` for a custom espeak data dir); the `voice` call
parameter is ignored by this provider. The viseme stream carries espeak-ng
phonemes. Note: this feature links GPLv3 code (libpiper/espeak-ng); default
builds stay GPL-free. Windows is not supported yet.

**AWS — the cloud provider (default build):** with no feature flag, `say` calls
the Vizij TTS cloud function — AWS Polly behind an HTTP endpoint — so there is
nothing to set up and **no AWS credentials in the app**. The `voice` parameter
names a Polly voice (default `Ruth`), and the viseme stream carries Polly
viseme codes. To use your own deployment (your AWS account), host the two
endpoints `POST /tts/get-audio` and `POST /tts/get-visemes` (body
`{"voice", "text"}`, returning the audio bytes and the Polly viseme speech
marks) and point the app at it with the `API_URL` environment variable.

**Sending text to it:** `say` is a described device method — behaviors (a
face's programs or spawned task runs) call it like any module function, and
bridges list it over `DescribeMethods` (its `Status` return is the action
shape). The ROS4HRI `/robot_face/tts` topic already lands text on the
`standard/ros4hri/speech/text` key; routing that key into `say` — and the
viseme stream onto the face — is the lipsync track, in progress.

## Lighting model

The web renders `MeshStandardMaterial` under a single `ambientLight(π/2)`,
which resolves to **albedo × 0.5 in linear space** (ambient intensity × the
Lambert 1/π). The native view reproduces this deterministically: materials
render unlit with `intensity/π` baked into the albedo (`--ambient`, default
π/2). Elements declaring `material: "basic"` render at full albedo — three's
`MeshBasicMaterial` ignores lights. Graph-driven `color` writes are linear
working-space floats (three `Color.setRGB` semantics), not sRGB.

Verified pixel-exact against the web renderer on flat regions of both
reference faces.

## Comparison harness

`docs/compare/` holds web | native | amplified-diff collages for the two
reference faces, rendered at the web viewer's canvas size (763×760):

| face | differing samples (channel Δ>32) |
|---|---|
| Quori (`Quori_Current_Extended.glb`) | 0.50% |
| Toasty (`Toasty_Current.glb`) | 0.66% |

The residual is anti-aliased contour fringes and idle-motion timing (the rig
graphs are time-driven). Checked properties: superposition (Toasty's drop
shadow, pupils over whites over face), masking (eye highlights inside pupils),
camera position and letterboxing, scales, coordinates, per-element colors.

Web references are captured with the scratch Playwright spec in
vizij-web (`apps/vizij-authoring/e2e/`, headed — headless Chromium does not
composite the WebGL canvas), loading the `quori:latest` / `toasty:basic`
presets.

## Not yet here

The native app is otherwise complete (VIZ-47): the animation module + clip
transport, the ROS 2 / Studio bridge flags, and `--headless` frames-to-store all
landed. The egui inspector panels (VIZ-82) were dropped — the operator surface is
the arora TUI + the store — and packaging (a distributable bundle) is still open.
See `docs/proposal-vizij-native-app.md`.
