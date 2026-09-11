# Scene spike: findings

What a Bevy scene costs and what it takes to build, measured on the real robot
(`Quori_2_RevB.glb`) rather than on a face's flat planes.

Machine: Apple Silicon, macOS, release build, 1280x800, vsync (see below).

## 1. Frame time on the real load

One robot is 24 meshes, 131,132 triangles, 24 materials. It renders at the
display's cadence with every mesh passing frustum culling — 16.67ms p50,
18.5ms p95.

That number alone says only that the load fits, because vsync cannot be lifted
here: wgpu falls back to Fifo for `AutoNoVsync` on macOS, so `--uncapped`
reports the same 16.67ms. Headroom therefore gets measured by multiplying the
real asset until the cadence breaks.

| robots | meshes | triangles | p50 ms | p95 ms | holds 60fps |
| -----: | -----: | --------: | -----: | -----: | ----------- |
|      1 |     24 |      131k |  16.71 |  18.53 | yes         |
|     16 |    384 |      2.1M |  16.70 |  18.08 | yes         |
|     36 |    864 |      4.7M |  16.69 |  17.95 | yes         |
|     49 |  1,176 |      6.4M |  16.66 |  17.95 | yes         |
|     64 |  1,536 |      8.4M |  16.77 |  18.61 | yes         |
|     81 |  1,944 |     10.6M |  17.62 |  25.72 | no          |
|    100 |  2,400 |     13.1M |  19.87 |  31.90 | no          |
|    225 |  5,400 |     29.5M |  51.35 |  56.83 | no          |

The knee sits between 64 and 81 robots. One robot is roughly 1/64th of what
this machine holds at 60fps.

**Two things that number is not.** The copies share 24 distinct meshes and 24
materials, so Bevy batches them; the triangle throughput is real but the
draw-call count is optimistic against 1,536 genuinely distinct objects. And
this is desktop-native — Studio ships to the browser, where the same scene runs
through WebGPU in wasm and will cost more. The native figure bounds the
question from above; it does not settle the browser one.

## 2. What the robot file actually carries

`RobotData` is a glTF **extension** on nodes, not `extras`, and the glTF node
names are empty — every real name lives inside it. 87 of 133 nodes carry it.

Types present: `body` x47, `shape` x24, `fixed` x8, `continuous` x4,
`revolute` x2 (`tilt_joint`, `neck_yaw_joint`), `prismatic` x1
(`head_lift_joint`), `screen` x1.

**The screen carries no geometry.** Node 102 is `{"type": "screen", "width":
0.235, "height": 0.148, "resolution": 1080}` with `mesh: None` — a declaration,
not a mesh. Whoever renders it builds the quad. Studio does exactly that in
`packages/scene/src/renderables/screen.tsx`: a `<Plane args={[width, height]}>`
whose material takes a `RenderTexture` of `width*resolution x height*resolution`
— here 254x160 — with the face rendered inside it. That maps one-to-one onto
Bevy's `RenderTarget::Image` plus `base_color_texture`.

**Joint limits are on the animatable**, at
`features.jointValue.value.constraints.{min,max}`, alongside `default` and a
`pub` block carrying `units`, `color` and a `public` flag. Studio's
`rotational-joint-gizmo.tsx` reads exactly those two fields and hands them to a
`PivotControls` as `rotationLimits`, on the third axis, after rotating the
control so its +Z lies along the joint axis.

## 3. Bevy 0.19 details this cost time on

- `AmbientLight` is a **component** (on the camera), not a resource.
- The glTF scene root is `WorldAssetRoot`, not `SceneRoot`.
- `DirectionalLight` spells it `shadow_maps_enabled`.
- `AppExit` goes through a `MessageWriter`, not an `EventWriter`.
- `WindowResolution` converts from integer pairs, not float pairs.
- The model is **Y-up and metre-scale** after import. A camera placed on the
  Studio scene's Z-up convention points at empty space and renders nothing —
  while the frame counter still reports a healthy 60fps. The camera is fitted
  to measured bounds for this reason, and visibility is reported as
  "N/M meshes visible after culling" so an empty frame cannot read as a fast one.

## 4. Reading a frame back does not work here

`Screenshot::primary_window()` returns pure black — not the clear colour —
when the window is not the composited frontmost surface, which is the normal
case for a run started from a terminal. Capturing an offscreen target instead
needs `TextureUsages::COPY_SRC`, which `Image::new_target_texture` does not
set; without it the readback silently returns the zero-filled CPU side. Adding
it was still not enough: a camera clearing to magenta reads back black, so the
readback path is not delivering the rendered texture on this platform.

This blocks screenshot-based verification only. It does not affect the screen
in the scene, which samples the texture on the GPU and never reads it back.
Visibility after culling is used as the in-engine substitute, and the live
window is the way to look at pixels.
