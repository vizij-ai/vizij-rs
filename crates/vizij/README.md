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

## Not yet here (VIZ-47 stages)

Animation module loading (clip transport), egui panels (inputs, transport,
bridges), bridge flags (`--ros2`, `--studio`; arora's local WS bridge attaches
when the device runs via `Arora::run`), `--headless` frames-to-store
(`view/frame` as `ArrayU8`), and packaging. See
`docs/proposal-vizij-native-app.md`.
