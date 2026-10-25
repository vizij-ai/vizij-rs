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

## Installing a release

Each version of this crate that reaches `main` is released by
[`release-vizij.yml`](../../.github/workflows/release-vizij.yml) as the GitHub
release `vizij-v<version>`. The release carries a binary and an installer per
desktop OS, built with the Studio bridge (`--features studio`), and the
browser module. Its notes are the version's [CHANGELOG](CHANGELOG.md) section.

To try a version before it reaches `main`, run the workflow by hand on its
branch with no tag (`gh workflow run release-vizij.yml --ref <branch>`): it
builds and packages the same way, and the packages are the run's artifacts.
Nothing is tagged or released.

| OS | Installer | Binary |
|---|---|---|
| macOS (Apple silicon) | `vizij-<version>-macos-arm64.dmg` | `vizij-<version>-macos-arm64.tar.gz` |
| Linux (x86_64) | `vizij_<version>-1_amd64.deb`, `vizij-<version>-1.x86_64.rpm` | `vizij-<version>-linux-x86_64.tar.gz` |
| Windows (x86_64) | `vizij-<version>-windows-x86_64.exe` (the binary itself) | |

The installers are not signed. macOS quarantines the app until it is opened
once from the context menu (or `xattr -dr com.apple.quarantine
/Applications/vizij.app`), and Windows SmartScreen asks to confirm it. The
local Piper voice (`tts-piper`) is not in a release, because it links GPLv3
code; build it from source.

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
| `view` | the Bevy rendering of a face from its GLB bytes, applying a device's pose each frame; `view::meta` (the bindings and the bundle read from the GLB), `view::snapshot` (offscreen rendering and readback), `view::frames` (rendered frames into the store), `view::diagnostics` (the device's health report into the store), `view::captions` (the face standard's caption over each face; feature `captions`) | everywhere |
| `face` | the composition: the GLB's bindings and bundle into one graph spec, folded with `RigHal` + `BlackboardStore` and the modules into an `AroraBuilder` a host may extend (`builder_for`); the bundle's animations loaded into the built device through the animation module's declared functions (`load_animations`); the program that runs from load, spawned beside the graph as a run of the interpreter's `run_behavior` (`spawn_program`); the free inputs and what the store says of them (`declare_keys`), the neutral pose, the skills' fragments | everywhere |
| `modules` | the host modules any Arora loads: `animation` (Arora's `@arora/animation`, from `arora-animation-module`), `rest` (`reset`, `reset_keys`), `tts_piper` (feature) | everywhere (Piper native) |
| `native` | the stand-alone device: the face's Arora on a worker thread under arora's operator flow, the bridges the build adds, the `RuntimeHandle` front ends speak through | every target but the browser |
| `web` | the browser module behind [`@vizij/runtime`](../../npm/@vizij/runtime/README.md): one App per page (`mount`), a JS-paced Arora per Vizij (`loadVizij`, a `VizijRuntime`), Vizijs as rectangles of the canvas (`placeVizij`), picks, `describe` | `wasm32` |
| `main.rs`, `open.rs` | the CLI, the window, the terminal operator UI, opening a face by drop or dialog | feature `desktop` (default) |
| `settings` | the settings pane over the face (egui): the background kept per face, the device info, the Studio association; the pane is the device's operator (`settings::PaneOperator`); see [The settings pane](#the-settings-pane) | feature `settings` (in `desktop`), every target but the browser |
| `android` (private) | the Android app's entry point (`android_main`): the device on the face last opened or the one the APK ships, the view full screen, faces received from the activity, logging into logcat; see [Android](#android) | Android |

Features: `desktop` (default) is the CLI, the terminal UI and the settings
pane; `settings` is the pane alone, which the Android app is built with; `captions`
(default) draws the face standard's caption over the face, with Bevy's UI
text and an embedded Inter font; `studio`,
`ros2-dds` / `ros2-zenoh` and `tts-piper` add the bridges and the local
speech provider to any native build. Without `desktop`, `cargo check --lib
--no-default-features` gives the library the browser (`--target
wasm32-unknown-unknown`) and Android (`cargo ndk … --features studio`)
entry points build on; CI checks both. The browser bundle is `wasm-pack
build crates/vizij-wasm --target web --release` (`pnpm run
build:wasm:runtime` at the repository root): `vizij-wasm` links this crate's
`web` module as a cdylib of its own, so the release profile's fat LTO
applies, which it does not to this crate's `rlib` + `cdylib` library. One
WebGL2 bundle of 24.3 MB (6.9 MB gzip, 4.3 MB brotli); CI renders Quori and
Toasty on a page with it and holds them to the same references as the
native snapshot.

A face enters as GLB bytes on every target: `view::meta::FaceMeta` reads the
bindings and the bundle from them, `face::load_face` composes them, and
the view serves them to Bevy's loader from memory (`view::FaceAssets`), so
nothing below the entry point touches a file system. The desktop binary
reads `--glb`; a front end loads another face by handing the running
device its bytes (`RuntimeHandle::reload`).

## Android

The APK is this crate's cdylib (`libvizij.so`) in the Gradle project under
`android/`. Its one activity, `VizijActivity` (Kotlin), is a
`NativeActivity` (Bevy's `android-native-activity` backend) that loads the
library and does what native code cannot through a NativeActivity: it takes
a face from the system's document picker (a long press on the face opens
it) or from a VIEW intent (a `.glb` or `model/gltf-binary` opened with
Vizij), copies it to the app's files directory as `model.glb`, and keeps the
screen on with the system bars hidden (immersive sticky). The library's
entry point starts the device on `model.glb` when it holds a face that
composes, so the face last opened comes back on every launch, after a
reboot too, and on the face the APK ships (`assets/face.glb`) otherwise; a
face received while the app runs reloads the device. The device runs under
arora's headless front end, with the open local bridge on `127.0.0.1:9000`
(`adb forward tcp:9000 tcp:9000` reaches it). A lost GPU device recreates
the renderer (`RenderErrorPolicy::Recover`) rather than ending the app.
Logging goes to logcat under the `vizij` tag through `android_logger`;
Bevy's `LogPlugin` stays disabled, as on desktop. minSdk 24 (the build
takes no Bevy `audio`, whose AAudio backend would raise it to 26), targetSdk
34; arm64-v8a for devices, x86_64 for the emulator.

The app is built with the settings pane ([below](#the-settings-pane)): a
tap on the face shows its Settings button, the back gesture closes the pane,
and with the pane closed moves the app to the back, as the standalone's back
did. The pane keeps clear of the system bars and the display's cutout, which
the activity reports (`insetsChanged`), and keeps its settings in the app's
files directory.

The app is built with the `studio` feature. Studio keeps the device's
credentials in the app's files directory
(`arora::studio::connect_with_device_dir`), and the device's identity
there in `studio-identity.json` (`name`, `owners`, `model_family`). A launch
with none kept contacts nobody, so an install nobody associated, a CI
emulator's among them, registers no device; the settings pane's association
form (the owner UIDs and a name) keeps one and connects. The registration's
questions do not reach the pane on Android:
arora 15 asks nobody in a given device directory
(`connect_with_device_dir`), and handing it the app's directory as
`DEVICE_DIR` for `connect_with_operator` is not sound there, as the JVM's
threads may read the environment while `set_var` writes it. The pane rules
on access requests on both platforms. `say` speaks through the cloud provider at its
default deployment, its audio played through cpal's oboe backend; the
provider's HTTPS client trusts the Mozilla CA set webpki-roots bundles, as on
every native target, so nothing of the platform verifier's Kotlin side is
packaged.

Building it takes the NDK `android/app/build.gradle` names (`ndkVersion`),
`cargo-ndk`, a JDK 17 or later and Gradle 8.14:

```sh
export ANDROID_NDK_HOME=$ANDROID_HOME/ndk/30.0.16248370
# The library, release-profile: a debug build's naga debug info crashes the
# emulator's SPIR-V validator. -t x86_64 for the emulator on an x86_64 host.
# libc++_shared.so goes beside it: the speech provider's audio output (oboe)
# is C++ on the shared runtime.
cargo ndk -t arm64-v8a -P 24 --link-libcxx-shared -o crates/vizij/android/app/src/main/jniLibs \
  build -p vizij --lib --release --no-default-features --features captions,studio,settings
# The face the APK ships.
mkdir -p crates/vizij/android/app/src/main/assets
cp Quori_Current_Extended.glb crates/vizij/android/app/src/main/assets/face.glb
gradle -p crates/vizij/android assembleDebug
```

`android/scripts/smoke-test.sh <apk> <out-dir> [second-face.glb]` is the
rendering smoke, on the emulator or device adb reaches: it installs and
launches the APK, waits for the face (at least 15 s), and passes when a
screenshot shows it, two screenshots 5 s apart differ in it, the second
face, opened through a VIEW intent, replaces it and is the one shown after
a relaunch, a `say` through the local bridge (`android/scripts/say.py`)
succeeds with the face reporting the utterance and showing its visemes, a
tap shows the settings pane's Settings button, which opens the pane, a
background picked there fills the screen once the back gesture closes it
and again after a relaunch (the buttons found on the screenshots by their
colours, `android/scripts/frames.py find`), the
app is alive and in front, and logcat has no `Caught` line,
panic or fatal signal. `dumpsys gfxinfo` counts
no Bevy frame, so the screenshots are the evidence. CI's `android`
workflow builds both ABIs, packages the APK and runs the smoke on the
x86_64 API 35 `google_apis` image under KVM with `-gpu lavapipe`
(emulator 36.4.9 or later).

The face draws under Vulkan only, and on the emulator only on the API 34
and 35 images with `-gpu host` or `-gpu lavapipe`: `-gpu
swiftshader_indirect` loses the device (wgpu turns a frame over its 1 s
acquire timeout into a device loss), the API 32 image reports a zero buffer
limit, and no GLES build draws.

## How it works

- **`view::meta`** reads what Bevy's GLB loader does not surface: the
  per-node `RobotData` extension (the animatables — UUID-identified
  features) and the scene-root `VIZIJ_bundle` (graphs, poses,
  animations, metadata). Bevy loads the same GLB for meshes/materials/morphs; the two
  worlds join on the glTF node name (`GltfNode<index>` for an unnamed node,
  as Bevy names it). Both RobotData feature models read: the web's whole
  features (`translation`, `color`) and Semio Studio's per-axis ones
  (`translation.x`, `rotation.r`, `color.g`), an axis binding its own
  scalar animatable to one component, or a component `<id>.<axis>` of a
  compound animatable binding `<id>` to the whole feature. An `ellipse` or
  `rectangle` reads as a shape drawn with a unit circle or 1×1 plane scaled
  by its `width` and `height` (where its node carries no mesh), its fill as
  the material; its stroke features are dropped with a warning. A GLB
  declaring no `rootBounds` is framed on its scene's XY bounding box, as
  the web's import derives it.
- **`face`** composes the bundle's graphs into one spec (node ids namespaced
  per source, store paths shared — the cross-source contract) and runs
  `RigHal` + `BlackboardStore` + `ProcessingGraph` as an arora; on desktop
  `native` steps it at ~100 Hz on a worker thread. The `Arora` is
  built inside that thread — it is single-owner by design and not `Send`.
- **`view`** renders Vizij's scene model: Z-up, faces in the XY
  plane layered along Z, orthographic camera fit to the authored `rootBounds`
  (`--fit` picks how, `--zoom` magnifies on top), sRGB output, no tonemapping,
  double-sided materials, opacity-driven alpha,
  morph-target influences. Each frame it reads each device's actuation state
  from the HAL seam (`RigHal::pose()`) and applies it: transforms (euler ZYX),
  material color/opacity, morphs. A face is an entity (`view::Face`) with its
  scene and its own camera, standing in a slot of its own along X so several
  faces share one App, one window or canvas — each face framed on a
  rectangle of it (`ViewEvent::PlaceFace`), any rectangle, the part inside
  the target drawing — and a click on a mesh reports
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
| `--program <id>` | bundle's active program | run this motiongraph program from launch, beside the face's graph (a task run any client halts) |
| `--no-autoplay` | off | run no program from launch: hold the rig's authored/neutral pose |
| `--no-stage-neutral` | off | don't stage the bundle's `neutralInputs` at boot |
| `--snapshot <png>` | — | render one frame offscreen and exit (no window) |
| `--headless` | off | run windowless; streams frames when exposed as ROS4HRI or given `--frame-rate` |
| `--size WxH` | `763x486` | offscreen render size (`--snapshot` / `--headless`) |
| `--frame-rate <hz>` | `15` when exposed as ROS4HRI, else off | publish rendered frames as HAL readings under the key of their transport; 0 disables |
| `--frame-format <fmt>` | `png` | encoding of published frames: `png` writes `display/face/compressed`, a `sensor_msgs/CompressedImage`; `raw` writes `display/face`, a `sensor_msgs/Image` |
| `--frame-id <name>` | the face's id from its GLB | the TF frame published frames are stamped with (`header.frame_id`) |
| `--background <rrggbb>` | `000000` | clear color; in a window, that of a face the settings pane keeps no background for |
| `--ambient <f>` | `π/2` | ambient intensity of the [lighting model](#lighting-model) |
| `--unlit` | off | render materials unlit (albedo passthrough) |
| `--fit <contain\|cover\|stretch>` | `contain` | how the face fits the window: letterbox, crop the excess axis, or distort to the window's aspect |
| `--zoom <f>` / `<fx>x<fy>` | `1` | magnify the fitted face — one factor for both axes, or width x height; below 1 shrinks it |

`vizij list-displays` prints the displays by index and exits. While the
window runs, a `.glb` dropped on it loads, and `O` opens the system's file
dialog (an XDG portal on Linux, so no toolkit is linked); either way the
running device restarts on the new face and the view follows.

### The local bridge

Every run serves arora's open local bridge
([`arora-bridge-ws`](https://docs.rs/arora-bridge-ws)) on `--bind:--port`:
`ws://127.0.0.1:9000` by default, with the control panel on
`http://127.0.0.1:9000/`. The bridge relays the device as its store and
modules describe it, so the app declares nothing to it. `list_keys` gives each
key with its `__meta`: the face's **free inputs** — the input paths no graph in
the composition writes — are the keys a client may write, each typed, with
the value it rests at: the authored default, the rig's under the bundle's
neutral pose. A number takes its range from, in order: its input node's
`params.min` and `params.max`, both optional, when both are there and `max`
exceeds `min`; the `range` the rig declares for the input in its
`metadata.vizij.inputs`, which the authoring app writes for every rig input;
otherwise `[0, 1]`. A key a shipped profile declares takes that profile's
type, range and rest. The device refuses a write to any other key.
`reset`, the rest module's function, returns every key to its rest.

The face's **skills** are the device's own, listed and called by name on the
signatures their modules describe, so `list_methods` carries them beside
`reset`:

| Method | Effect |
|---|---|
| `look_at` | the gaze skill: `policy` (`track`/`glance`/`reset`), `target` (meters), `frame` |
| `play_viseme` | one viseme `shape` at a `weight` through the lipsync envelope |
| `say` | speak `text` in `voice` (only when the build has a speech provider) |

Each is a **run** (`"task": true`): the `invoke` answers as soon as it starts,
with the run's id and the key that says how it ends, and
`{"type": "halt", "run": "<id>"}` stops it. Writes and reads travel as the wire
format documents
(`{"type": "write_values", "values": {"standard/ros4hri/expression/valence": {"f32": 0.5}}}`),
and a client is pushed the keys it subscribes to — subscribe to what you
display, or take the whole feed, the clock included, by not subscribing. The
server lives with the device generation: a reload frees the port before the
next generation binds it. `tests/local_bridge.rs` is a client on it
(`cargo test -p vizij --test local_bridge` with `VIZIJ_FIXTURES` set).

### The other bridges

The ROS 2 and Studio bridges are build features; they compose with the
local bridge:

| Flag | Feature | Effect |
|---|---|---|
| `--ros2 [namespace][:domain]` | `ros2-dds` (alias `ros2`) or `ros2-zenoh` | join the ROS graph as a ROS4HRI face (see below) |
| `--studio` | `studio` | attach the Semio Studio bridge: the device connects under the identity kept in the app's data directory (`studio-identity.json`), registering it only while Studio holds no record of the device, else as the operator answers — the settings pane in a window, the terminal without one — (Studio's record then kept), else from the environment (`DEVICE_OWNERS`, …); without the flag, the pane's Associate (a device not associated) or Connect (one associated) attaches it for the rest of the run |

Under `--studio` the face serves **Studio's keys** (`vizij::studio`, the
Studio-key profile), so a Studio user who claims the device drives the
animatables its view draws. Studio's update for an entity carries, for each
animatable, its target and, while Studio's timeline plays, its target
velocity; each target lands under the animatable id the view indexes and
its feedback reports it back. `<id>` is the animatable's id for a number,
whose feedback is `<id>.position`; a vector, euler or colour is written per
component, `<id>.<axis>`, and lands joined under `<id>`, its feedback
published under both spellings of the axis, `<id>.position.<axis>` and
`<id>.<axis>.position`, which always agree. Each target rests at the animatable's RobotData
default. Studio's keys for an animatable a rig graph writes, or one the view
binds to nothing (a joint's value, a stroke), are accepted too, so the rest
of the update lands, and move nothing: the rig keeps its animatable, and
neither reports feedback.

A target is taken in either form, and the device keeps the two in
agreement, so the latest write wins whichever form it took:

| Form | A number `<id>` | A component `<id>.<axis>` |
|---|---|---|
| `k/target.c`, as [`arora_hal::set_targets`](https://docs.rs/arora-hal) reads it | `<id>/target.position`, `<id>/target.velocity` | `<id>/target.position.<axis>`, `<id>/target.velocity.<axis>` |
| Studio's current spelling | `<id>.target_position`, `<id>.target_velocity` | `<id>.<axis>.target_position`, `<id>.<axis>.target_velocity` |

Studio's production line writes the current spelling for a whole number
only: it sends no per-axis target. The per-axis spelling is the one the
Studio-on-Arora line writes.

An update carrying both forms of one target takes the `k/target.c` form's
value, as `set_targets` does
([proposal](https://github.com/semio-ai/arora-sdk/pull/333)).
`<id>.studio_value` — the target again, which Studio sends beside
`.target_position` — and its per-axis `<id>.<axis>.studio_value` are legacy: an `f64` input that
drives nothing, kept until Studio no longer sends it. Velocities, in both
forms, are accepted, rest at 0 and move nothing. Every animatable the face
draws is numeric, so the device takes no `<id>/target.value`.

The device serves its face's GLB to Studio as its HAL model: `RigHal` holds
the GLB it shows and states it as the `device` component's model, servable,
with its content hash, through Arora's HAL module. Studio's client lists it
with `retrieveModels` and fetches it with `retrieveModelGlb`
([studio-bridge#104](https://github.com/semio-ai/studio-bridge/pull/104)).
The older `retrieveGlb` (`GET_MODEL_GLB`) has no answer on the device.

`--ros2` attaches [`arora-bridge-ros2`](https://github.com/semio-ai/arora-sdk/tree/main/crates/arora-bridge-ros2)
with its ROS4HRI exposure preset:

- the typed face topics — `/robot_face/{expression,look_at}` — routed onto the
  `ros4hri` profile's `standard/ros4hri/*` keys, and `/robot_face/speech`
  (`std_msgs/String`) publishing what the face is saying, empty at rest. An expression command
  naming an expression shows that expression alone at its arousal as the
  intensity (clamped to 0..1, so a calm, negative arousal shows none); one
  with an empty name blends the expressions by its valence and arousal
  (each -1..1) on the circumplex;
- the **`/<namespace>/actions/{play_viseme,say}`** action servers, synthesized
  from the viseme players' signatures ([skills](../../docs/skills.md));
- the **`/skill/look_at`** action server (`interaction_skills/LookAt`):
  track / glance / reset policies, priority preemption, standard error codes;
- the **face image** on the `image_transport` pair PAL OS documents:
  `display/face/compressed` as a `sensor_msgs/CompressedImage` on
  `/robot_face/image_raw/compressed` (`--frame-format png`, the default), or
  `display/face` as a `sensor_msgs/Image` on `/robot_face/image_raw` (`raw`);
- **closed captions** on `/closed_captions` (`hri_actions_msgs/ClosedCaption`),
  shown over the face — and so in its published image — in eight-word
  chunks, blue for the robot's own speech and white for anyone else's;
- the device's **health** on `/diagnostics` (`diagnostic_msgs/DiagnosticArray`,
  once a second): the view's frame rate, and per face its id, its compliance
  level with the face standard and any path its composition writes from more
  than one source;
- data topics under `/<namespace>/keys/<path>`: every store key **published**,
  and the keys the store opens to remote writers — the face's **free inputs**,
  input paths no graph in the composition writes, and the inputs its
  programs read — **subscribed** as `std_msgs` of the type each input
  declares (`Float32` for numbers, the graph's number type, but `Float64` for
  Studio's keys, as Studio writes them; `String`/`Bool` by the input's
  default; `ros2 topic info -v` shows each).
  Keys a graph writes every step (the ROS4HRI profile's `standard/vizij/*`
  outputs) are not inputs: drive them through the ROS4HRI topics. A program
  runs beside the graph as a task run, so the keys it writes stay inputs; a
  running program writes over them every step.

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
crate — the same module the browser build (`@vizij/runtime`) registers; the Piper provider
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
way. The contract is text in, status, a viseme stream and the utterance out, so a
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
  std_msgs/msg/Float32 "{data: 1.0}"
```

— and [Bring Your Own Visemes](https://github.com/vizij-ai/vizij-docs/blob/main/current_documentation/guidebook/deploy/bring-your-own-visemes.md) compares the three entry points.

**What it is saying:** while a `say` run's audio plays, the utterance is the
face's speech state, `standard/vizij/speech` (empty before playback starts
and once it ends), next to the current viseme at `standard/vizij/viseme`. The
ROS4HRI mapping relays them to `standard/ros4hri/speech/text` and
`standard/ros4hri/speech/viseme`, which `--ros2` publishes as
`/robot_face/speech` for subtitles and `/robot_face/viseme` for anything that
follows the lips.

## Lighting model

Faces are lit by a single ambient light of intensity π/2 — three.js's
`ambientLight(π/2)` — and no environment map. A `MeshStandardMaterial` (`material: "standard"`) resolves
to **base × (1 − metalness) × 0.5 + emissive × emissiveIntensity in linear
space** (ambient intensity × the Lambert 1/π on the diffuse term; a metal
has no diffuse and nothing to reflect, so a metallic plate is black;
roughness shapes nothing). A `MeshPhongMaterial` or `MeshLambertMaterial`
(`"phong"`, `"lambert"`) has no metalness and resolves to **base × 0.5 +
emissive × emissiveIntensity** — the metallic factor their GLB material
carries is ignored, and phong's `specular` and `shininess` need a direct
light, so they shape nothing either. A `MeshBasicMaterial` (`"basic"`)
ignores lights: its base color, with no metalness or emissive term.

The view computes it deterministically: every material renders
unlit with that composition baked into its albedo (`--ambient`, default
π/2), from the GLB material at load and from the `color`, `opacity`,
`metalness`, `roughness`, `emissive`, `emissiveIntensity`, `shininess` and
`specular` bindings as the rig writes them. A mesh no element declares — a
GLB with no RobotData, such as a plain Blender export — is shaded as
three's glTF loader makes its material: basic for `KHR_materials_unlit`,
standard otherwise. Graph-driven `color` and `emissive` writes are linear
working-space floats (three `Color.setRGB` semantics), not sRGB.

On flat regions of the reference faces this matches three.js pixel for pixel:
Quori and Toasty, whose look is in `color`, and Emy, whose look is a black
metallic plate with emissive features. Quori's Blender export carries its
colors in `emissiveFactor` over a black metallic base and renders them
exactly (its flat face color is the sRGB encoding of its emissive factor).


## Look-at policies

The ROS4HRI `/skill/look_at` action is implemented by the `look_at` skill fragment in `crates/interop/vizij-arora-host/src/skills.rs`. Its generated graph is committed as `crates/interop/vizij-arora-host/skills/look_at.json`.

The currently supported policies are:

* **`track`** — continuously looks at the supplied `target` and `frame` until the goal is cancelled or replaced.
* **`glance`** — looks at the supplied target, holds the fixation briefly, then succeeds.
* **`reset`** — returns the gaze to the forward/rest target, holds briefly, then succeeds.
* **`idle`** — continuously wanders around the forward direction with smooth, relatively regular movement.
* **`random`** — continuously wanders with a more variable, sporadic trajectory.

### Adding or changing a policy

Policy behavior is defined in `generate_look_at()` in `crates/interop/vizij-arora-host/src/skills.rs`.

For a new continuous policy:

1. Create the policy's target trajectory in `generate_look_at()`.
2. Add the policy to the `gaze/target` and `gaze/frame` `case` nodes.
3. Add the policy to the lifecycle `status` case and connect it to `status/running`.
4. Add the policy to the `errno` case as a supported policy.
5. Update the policy description/comments if needed.
6. Regenerate the committed asset.

The generated `look_at.json` should not be edited manually. Regenerate it from the Rust generator with:

```bash
cargo run -p vizij-bundle -- export-skill look_at \
  -o /tmp/look_at.json

cp /tmp/look_at.json \
  crates/interop/vizij-arora-host/skills/look_at.json
```

Then verify that the generated asset matches the generator:

```bash
cargo test -p vizij-arora-host --lib \
  skills::tests::committed_asset_matches_the_generator
```

### Tuning `idle` and `random`

The gaze target uses the standard ROS point convention:

* **X** = forward/backward. The gaze currently keeps X fixed at `10.0`, so it is not used to control movement speed.
* **Y** = horizontal gaze, left/right.
* **Z** = vertical gaze, up/down.

For `idle`, the Y and Z trajectories are generated with `simplenoise` and then smoothed with `damp`. Increasing the **frequency** makes the gaze move faster; the amplitudes control how far it moves.

For example:

```rust
// Y = horizontal speed
"frequency": 0.25,

// Z = vertical speed
"frequency": 0.20,
```

The `idle` damping keeps this movement smooth and natural.

For `random`, the same Y/Z frequency parameters control the overall rate, but the noise trajectory naturally produces more variation in instantaneous speed:

```rust
// Y = horizontal speed
"frequency": 0.45,

// Z = vertical speed
"frequency": 0.32,
```

The random amplitudes can be changed separately when a larger or smaller gaze range is desired.

After **any change to these parameters**, regenerate `look_at.json` with `vizij-bundle` as shown above. The runtime uses the generated asset, while the generator in `skills.rs` remains the source of truth.


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


## The settings pane

The Vizij standalone's Settings page, over the face in the desktop window
and on Android (`vizij::settings`, drawn with egui through a camera of its
own above the faces'). A tap or click on the face shows the Settings button
at the bottom of the screen, and the device's name at the top once Studio
registered it; another tap hides them. The button opens the pane, which
Close, the Android back gesture and Escape close. It holds:

- **Current face**, by its id, and **Open a face…**: the file dialog on
  desktop (as `O` does), the document picker on Android (as a long press
  does);
- **Background**: swatches and a colour picker. The colour is kept for the
  face shown, by its id, in `settings.json` in the app's data directory (the
  desktop's data directory above, Android's files directory), so a face
  opened again shows the background chosen for it; a face with none kept
  shows the app's (`--background` on desktop, black on Android);
- **Device**, in a build with `studio`: where the device stands with Semio
  Studio (`RuntimeHandle::studio`), on desktop and Android alike.
  - An associated device — registered, or keeping an identity
    (`studio-identity.json`), or on desktop the registration arora's
    operator saved (`studio/device.json` in arora's device directory) —
    shows what Studio holds of it, read only: its name, owners, model family
    and software. Its name and owners are Studio's, edited in Semio Studio:
    the device registers its identity only while Studio holds no record of
    it, and otherwise keeps Studio's record as its identity. A desktop run
    that did not ask for Studio connects with **Connect to Semio Studio**.
  - A device not associated shows the form its first association takes:
    its owners, one Studio user id (Firebase UID) per row with Remove and
    Add owner, and a device name, generated when left blank. **Associate
    with Semio Studio** keeps them as the device's identity and restarts it
    on its face with the Studio bridge, which registers them. Each owner is
    registered as an owner: arora 15's device info carries one owner list
    (`arora_bridge::DeviceInfo::owners`), so the editor and viewer roles of
    Studio's device record are not set from the device.

  On desktop, arora 15 itself registers the registration its operator saved
  as the client connects (arora's `studio` docs), so a device associated
  through arora's operator has the name and owners saved there written to
  Studio at each connection.

The pane is the device's operator (`settings::PaneOperator`, an
`arora::operator::Operator`, given as `native::Mode::OperatedBy`): a
question the registration asks (`ask_text`) opens the pane on it, and waits
for the answer, skippable unless it is required; a client's access request
(`decide_access`) opens it with Allow, Reject, Always allow and Always
reject, and is allowed after `arora::operator::DEFAULT_ACCESS_GRACE`
unanswered, as the unattended operator rules. On desktop the pane answers
in place of the terminal UI, which keeps its log pane and commands; a run
with no window (`--headless`) answers on the terminal.
