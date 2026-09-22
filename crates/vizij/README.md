# vizij — an arora with a head

`cargo run -p vizij -- --glb <face.glb>` opens a Bevy window rendering the
face, driven by a natively-run arora device executing the face's own graphs
(rig + pose-driver from the embedded `VIZIJ_bundle`) — the same runtime
contract the web apps use, with no browser and no JS.

```bash
cargo run -p vizij -- --glb path/to/Quori_Current_Extended.glb
# a face from a URL, full screen on the second display, the bridge open to the LAN
cargo run -p vizij -- --glb https://example.org/face.glb --fullscreen --display 1 --bind 0.0.0.0
# the face loaded last time, again
cargo run -p vizij
# headless: render one frame offscreen and exit
cargo run -p vizij -- --glb face.glb --snapshot out.png --size 763x760
```

## One crate, every target

The crate is a library plus the desktop binary. The library is the
components a device composes — the view, the face, the modules — and builds
wherever a face is shown; the binary is the one entry point that spawns a
device out of them, and the other entry points — the browser module, the
Android activity — compose the same pieces. A robot that is a device already
folds them into its own builder beside its own hardware: the face is one more
piece of it, not a second device.

| Module | What it is | Where it builds |
|---|---|---|
| `view` | the Bevy rendering of a face from its GLB bytes, applying a device's pose each frame; `view::meta` (the bindings and the bundle read from the GLB), `view::snapshot` (offscreen rendering and readback), `view::frames` (rendered frames into the store) | everywhere |
| `face` | the composition: the GLB's bindings and bundle into one graph spec, folded with `RigHal` + `BlackboardStore` and the modules into an `AroraBuilder` a host may extend (`builder_for`); the free inputs, the neutral pose, the skills' fragments | everywhere |
| `modules` | the host modules any Arora loads: `animation`, `gaze`, `viseme`, `tts_piper` (feature) | everywhere (Piper native) |
| `native` | the stand-alone device: the face's Arora on a worker thread under arora's operator flow, the bridges the build adds, the `RuntimeHandle` front ends speak through | every target but the browser |
| `native::bridge` | the open local bridge: the WebSocket server with the face's inputs and skills in its registry, the control panel on the same port | every target but the browser |
| `web` | the browser module behind [`@vizij/runtime`](../../npm/@vizij/runtime/README.md): one App per page (`mount`), a JS-paced Arora per Vizij (`loadVizij`, a `VizijRuntime`), Vizijs as rectangles of the canvas (`placeVizij`), picks, `describe` | `wasm32` |
| `main.rs`, `open.rs` | the CLI, the window, the terminal operator UI, opening a face by drop or dialog | feature `desktop` (default) |

Features: `desktop` (default) is the CLI and the terminal UI; `studio`,
`ros2-dds` / `ros2-zenoh` and `tts-piper` add the bridges and the local
speech provider to any native build. Without `desktop`, `cargo check --lib
--no-default-features` gives the library the browser (`--target
wasm32-unknown-unknown`) and Android (`cargo ndk … --features studio`)
entry points build on; CI checks both. The browser bundle is `wasm-pack
build crates/vizij --target web --release -- --no-default-features`
(`pnpm run build:wasm:runtime` at the repository root), one WebGL2 bundle
of about 25 MB (7 MB gzipped); CI renders Quori and Toasty on a page with
it and holds them to the same references as the native snapshot.

A face enters as GLB bytes on every target: `view::meta::FaceMeta` reads the
bindings and the bundle from them, `face::load_face` composes them, and
the view serves them to Bevy's loader from memory (`view::FaceAssets`), so
nothing below the entry point touches a file system. The desktop binary
reads `--glb`; a front end loads another face by handing the running
device its bytes (`RuntimeHandle::reload`).

## How it works

- **`view::meta`** reads what Bevy's GLB loader does not surface: the
  per-node `RobotData` extension (the animatables — UUID-identified
  features) and the scene-root `VIZIJ_bundle` (graphs, poses, clips,
  metadata). Bevy loads the same GLB for meshes/materials/morphs; the two
  worlds join on the glTF node name.
- **`face`** composes the bundle's graphs into one spec (node ids namespaced
  per source, store paths shared — the cross-source contract) and runs
  `RigHal` + `BlackboardStore` + `ProcessingGraph` as an arora; on desktop
  `native` steps it at ~100 Hz on a worker thread. The `Arora` is
  built inside that thread — it is single-owner by design and not `Send`.
- **`view`** renders the web renderer's scene model: Z-up, faces in the XY
  plane layered along Z, orthographic camera fit to the authored `rootBounds`
  (`--fit` picks how, `--zoom` magnifies on top), sRGB output, no tonemapping,
  double-sided materials, opacity-driven alpha,
  morph-target influences. Each frame it reads each device's actuation state
  from the HAL seam (`RigHal::pose()`) and applies it: transforms (euler ZYX),
  material color/opacity, morphs. A face is an entity (`view::Face`) with its
  scene and its own camera, standing in a slot of its own along X so several
  faces share one App, one window or canvas — each camera confined to a
  rectangle of it (`ViewEvent::PlaceFace`) — and a click on a mesh reports
  the face and the RobotData element it belongs to (`view::Picks`). The
  desktop shows one face; the browser module shows as many as the page loads.
- **`view::snapshot`** is the headless pipeline (recipe from ros-viz-rs):
  `WinitPlugin` disabled, `ScheduleRunnerPlugin`, camera → `RenderTarget`
  image, `gpu_readback` → PNG.

## Flags

`cargo run -p vizij -- --glb <face.glb>` plus:

| Flag | Default | Effect |
|---|---|---|
| `--glb <path or url>` | the face loaded last time | the face GLB (embedded `RobotData` + `VIZIJ_bundle`); an `http(s)://` URL is fetched whole; the source is remembered in the app's data directory (`~/Library/Application Support/vizij`, `~/.local/share/vizij`, `%LOCALAPPDATA%\vizij`) |
| `--port <n>` | `9000` | the local bridge's port — the WebSocket and the control panel |
| `--bind <addr>` | `127.0.0.1` | the address the local bridge binds; `0.0.0.0` opens it to the LAN (the link is unauthenticated) |
| `--no-web-control` | off | don't serve the control panel on `GET /` of the bridge's port |
| `--fullscreen` | off | borderless full screen on `--display` |
| `--display <i>` | the primary | the display the window opens on, by the index `list-displays` prints |
| `--width <px>` / `--height <px>` | the face's aspect at 720 px high | the window's size, in logical pixels |
| `--no-decorations` | off | no title bar or borders |
| `--always-on-top` | off | the window stays above the others |
| `--graphs <kinds>` | `rig,pose-driver,pose,standard-adaptation` | compose only these bundle graph kinds |
| `--no-ros4hri` | off (ROS4HRI **on**) | drop the built-in [ROS4HRI](../../docs/ros4hri.md) mapping and, under `--ros2`, the ROS4HRI exposure (typed topics, face image, skills) |
| `--program <id>` | bundle's active program | autoplay this motiongraph program |
| `--no-autoplay` | off | hold the rig's authored/neutral pose |
| `--no-stage-neutral` | off | don't stage the bundle's `neutralInputs` at boot |
| `--snapshot <png>` | — | render one frame offscreen and exit (no window) |
| `--headless` | off | run windowless; streams frames when exposed as ROS4HRI or given `--frame-rate` |
| `--size WxH` | `763x486` | offscreen render size (`--snapshot` / `--headless`) |
| `--frame-rate <hz>` | `15` when exposed as ROS4HRI, else off | publish rendered frames as HAL readings under the key of their transport; 0 disables |
| `--frame-format <fmt>` | `png` | encoding of published frames: `png` writes `display/face/compressed`, a `sensor_msgs/CompressedImage`; `raw` writes `display/face`, a `sensor_msgs/Image` |
| `--frame-id <name>` | the face's id from its GLB | the TF frame published frames are stamped with (`header.frame_id`) |
| `--background <rrggbb>` | `000000` | clear color |
| `--ambient <f>` | `π/2` | three.js-style ambient intensity |
| `--unlit` | off | render materials unlit (albedo passthrough) |
| `--fit <contain\|cover\|stretch>` | `contain` | how the face fits the window: letterbox, crop the excess axis, or distort to the window's aspect |
| `--zoom <f>` / `<fx>x<fy>` | `1` | magnify the fitted face — one factor for both axes, or width x height; below 1 shrinks it |

`vizij list-displays` prints the displays by index and exits. While the
window runs, a `.glb` dropped on it loads, and `O` opens the system's file
dialog (an XDG portal on Linux, so no toolkit is linked); either way the
running device restarts on the new face and the view follows.

### The local bridge

Every run serves the open local bridge of
[`arora-bridge-ws`](https://docs.rs/arora-bridge-ws) on `--bind:--port`:
`ws://127.0.0.1:9000` by default, with the control panel on
`http://127.0.0.1:9000/`. Its registry advertises the face's **free
inputs** — the input paths no graph in the composition writes, each typed
and with its rest value — and the methods a client may `invoke`:

| Method | Effect |
|---|---|
| `reset` | every input back to its rest value: the authored default, the rig's under the bundle's neutral pose |
| `look_at` | the gaze skill: `policy` (`track`/`glance`/`reset`), `target` (meters), `frame` |
| `play_viseme` | one viseme `shape` at a `weight` through the lipsync envelope |
| `say` | speak `text` in `voice` (only when the build has a speech provider) |
| `stop` | halt the last run of the skill named by `method` |

A skill's `invoke` answers as soon as the run is spawned; the run reports on
its status key. Writes and reads travel as the wire format documents
(`{"type": "write_values", "values": {"standard/ros4hri/au/12": {"f64": 0.5}}}`);
`arora/*` built-ins (the clock at step rate) are not pushed to clients. The
server lives with the device generation: a reload frees the port before the
next generation binds it. `tests/local_bridge.rs` is a client on it
(`cargo test -p vizij --test local_bridge` with `VIZIJ_FIXTURES` set).

### The other bridges

The ROS 2 and Studio bridges are build features; they compose with the
local bridge:

| Flag | Feature | Effect |
|---|---|---|
| `--ros2 [namespace][:domain]` | `ros2-dds` (alias `ros2`) or `ros2-zenoh` | join the ROS graph as a ROS4HRI face (see below) |
| `--studio` | `studio` | attach the Semio Studio bridge: the device registers under the identity kept in the app's data directory (`studio-identity.json`), else as the operator answers on the terminal (then kept), else from the environment (`DEVICE_OWNERS`, …) |

`--ros2` attaches [`arora-bridge-ros2`](https://github.com/semio-ai/arora-sdk/tree/main/crates/arora-bridge-ros2)
with its ROS4HRI exposure preset:

- the typed face topics — `/robot_face/{expression,look_at,tts}` and
  `/expressive_face/{look_at,speech}` — routed onto the `ros4hri` profile's
  `standard/ros4hri/*` keys;
- the **`/<namespace>/actions/{play_viseme,say}`** action servers, synthesized
  from the viseme players' signatures ([skills](../../docs/skills.md));
- the **`/skill/look_at`** action server (`interaction_skills/LookAt`):
  track / glance / reset policies, priority preemption, standard error codes;
- the **face image** on the `image_transport` pair PAL OS documents:
  `display/face/compressed` as a `sensor_msgs/CompressedImage` on
  `/robot_face/image_raw/compressed` (`--frame-format png`, the default), or
  `display/face` as a `sensor_msgs/Image` on `/robot_face/image_raw` (`raw`);
- data topics under `/<namespace>/keys/<path>`: every store key **published**,
  and the face's **free inputs** — input paths no graph in the composition
  writes — **subscribed** as `std_msgs` (`Float64` for numeric controls,
  `String`/`Bool` by the input's default; `ros2 topic info -v` shows each).
  Keys a graph writes every step (the ROS4HRI profile's `standard/vizij/*`
  outputs, the autoplaying program's outputs) are not inputs: drive them
  through the ROS4HRI topics or the program's own inputs.

The two RMW backends are mutually exclusive per build. `ros2-dds` speaks
DDS, ROS 2's default (`ros2` is its alias). `ros2-zenoh` speaks rmw_zenoh's
protocol and, like rmw_zenoh, needs a running router (`ros2 run rmw_zenoh_cpp
rmw_zenohd`), reached through the same environment rmw_zenoh reads —
`ZENOH_CONFIG_OVERRIDE='mode="client";connect/endpoints=["tcp/127.0.0.1:7447"]'`
or a full `ZENOH_SESSION_CONFIG_URI`. The live ROS tests run under `ros2-dds`.

[ROS4HRI support](../../docs/ros4hri.md) documents the key contract, the
per-channel behavior, how to drive a key from a ROS 2 shell, how to join a
real graph over rmw_zenoh, and the skill's
semantics.

## Speech (TTS)

The device registers a **`say(text, voice) → Status`** provider that
synthesizes speech, plays it (rodio — pure Rust, nothing to install), and
streams the viseme at the audio playhead through a mutable out-parameter, as
one of the [face standard](../../docs/face-standard.md#visemes)'s shapes. The
**say skill** ([skills](../../docs/skills.md)) hosts that call in a run and
drives the face's lips from the stream. The contract and the cloud provider
ship as the [`vizij-arora-tts`](https://crates.io/crates/vizij-arora-tts)
crate — the same module the vizij-web standalone registers; the Piper provider
implements the contract here, behind its feature. A build carries exactly one
provider. Try either from the command line:

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
parameter is ignored by this provider. The viseme stream is espeak-ng's
phonemes mapped to the standard shapes by articulation. Note: this feature
links GPLv3 code (libpiper/espeak-ng); default builds stay GPL-free. Windows
is not supported yet.

**AWS — the cloud provider (default build):** with no feature flag, `say` calls
the Vizij TTS cloud function — AWS Polly behind an HTTP endpoint — so there is
nothing to set up and **no AWS credentials in the app**. The `voice` parameter
names a Polly voice (default `Ruth`), and the viseme stream is Polly's viseme
codes mapped to the standard shapes. To use your own deployment (your AWS
account), host the two
endpoints `POST /tts/get-audio` and `POST /tts/get-visemes` (body
`{"voice", "text"}`, returning the audio bytes and the Polly viseme speech
marks) and point the app at it with the `API_URL` environment variable.

**Swapping the provider:** a build carries one, and the face never sees
which. `--features tts-piper` picks the local one, `API_URL` points the cloud
one at your own deployment (the browser module takes it as `loadFace`'s
`speechApiUrl`, and plays the audio through the page's hook), and a provider
of your own is a host module
implementing the `say` contract `vizij-arora-tts` re-exports — a sibling of
[`src/modules/tts_piper.rs`](src/modules/tts_piper.rs), registered behind a feature the same
way. The contract is text in, status and a viseme stream out, so a
text-to-speech that produces no visemes (derive them from the text) or a
viseme generator with no audio at all plugs in there too. The guidebook walks
through each option: [Swap the Speech Provider](https://github.com/vizij-ai/vizij-docs/blob/main/current_documentation/guidebook/deploy/swap-the-speech-provider.md).

**Sending text to it:** `say` is a described device method — a behavior
calls it like any module function, and a bridge spawns it as a task run
(bridges list it over `DescribeMethods`; its `Status` return is the action
shape, and `--ros2` serves it as the `/<namespace>/actions/say` action).
Spawned, the run is the say skill's: the lips follow the speech and the
run's feedback is the current viseme; halting the run cuts the audio within
250 ms and puts the lips at rest. `play_viseme(shape, weight)` plays one
shape the same way without speech. The fifteen viseme weights are free inputs
too: under `--ros2` a producer with its own timing writes them raw, no player
involved —

```bash
ros2 topic pub --once /<namespace>/keys/rig/<faceId>/standard/vizij/viseme/aa \
  std_msgs/msg/Float64 "{data: 1.0}"
```

— and [Bring Your Own Visemes](https://github.com/vizij-ai/vizij-docs/blob/main/current_documentation/guidebook/deploy/bring-your-own-visemes.md) compares the three entry points. The
ROS4HRI `/robot_face/tts` topic lands text on the `standard/ros4hri/speech/text`
key, which nothing routes into `say` yet.

## Lighting model

The web renders `MeshStandardMaterial` under a single `ambientLight(π/2)` and
no environment map, which resolves to **base × (1 − metalness) × 0.5 +
emissive × emissiveIntensity in linear space** (ambient intensity × the
Lambert 1/π on the diffuse term; a metal has no diffuse and nothing to
reflect, so a metallic plate is black; roughness shapes nothing). The native
view reproduces this deterministically: every material renders unlit with
that composition baked into its albedo (`--ambient`, default π/2), from the
GLB material at load and from the `color`, `opacity`, `metalness`,
`roughness`, `emissive` and `emissiveIntensity` bindings as the rig writes
them. Elements declaring `material: "basic"` render at full albedo with no
metalness or emissive term — three's `MeshBasicMaterial` ignores lights and
has neither. Graph-driven `color` and `emissive` writes are linear
working-space floats (three `Color.setRGB` semantics), not sRGB.

Verified pixel-exact against the web renderer on flat regions of the
reference faces: Quori and Toasty, whose look is in `color`, and Emy, whose
look is a black metallic plate with emissive features.

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

## Memory

`memory_tests` wraps the allocator and measures the process' **heap floor** —
the lowest live-byte reading in a window — before and after a stretch of
traffic, so anything the device keeps shows up as a rising floor while
transient allocation does not. Two tests bracket the seams:
`the_device_alone_keeps_a_flat_heap_under_a_frame_feed` runs the device with no
bridge under the view's frame feed, and `the_device_keeps_a_flat_heap_in_a_ros_graph`
(in `ros2_tests`, live DDS) runs the same device in a ROS graph with a peer
driving its free input and reading its frames back. A flat floor in the first
and a rising one in the second puts the retention on the ROS 2 path rather than
the device.

The second is `#[ignore]`d and fails when run: RustDDS retains every large
sample it publishes, so a face streaming frames over the DDS backend grows in
step with what it has already delivered — 21 MB kept against 20 MB delivered
over a 20 s window, with the peer reading every frame back ([VIZ-118]). The
Zenoh backend does not. Run it with `--ignored` to re-measure.

[VIZ-118]: https://linear.app/semio-ai/issue/VIZ-118


## Not yet here

The native app is otherwise complete (VIZ-47): the animation module + clip
transport, the bridges, and `--headless` frames-to-store all landed. The
operator surface is the arora TUI + the store: an in-window operator panel
(egui) — the Studio registration prompt where there is no terminal, as on
Android — is not built, and packaging (a distributable bundle) is still open.
See `docs/proposal-vizij-native-app.md`.
