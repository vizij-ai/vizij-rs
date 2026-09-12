# Scene spike: findings

What a Bevy scene costs and what it takes to build, measured on the real robot
(`Quori_2_RevB.glb`) rather than on a face's flat planes.

Machine: Apple Silicon (14" MacBook Pro, ProMotion), macOS, release build,
1280x800, shadows on, camera orbiting.

## 1. Frame time on the real load

One robot is 24 meshes, 131,132 triangles, 24 materials, with shadows on and
the camera orbiting. **It costs 2.40ms a frame.**

| robots | meshes | triangles | p50 ms | p95 ms |  fps |
| -----: | -----: | --------: | -----: | -----: | ---: |
|      1 |     24 |      131k |   2.40 |   2.72 |  417 |
|      4 |     96 |      525k |   2.86 |   3.21 |  349 |
|     16 |    384 |      2.1M |   8.25 |   8.69 |  121 |
|     36 |    864 |      4.7M |  16.35 |  17.08 |   61 |
|     64 |  1,536 |      8.4M |  27.33 |  32.38 |   37 |

So a 120Hz budget holds about 16 robots and a 60Hz budget about 36. One robot
spends roughly a third of a 120Hz frame and a seventh of a 60Hz one.

**Measuring this at all takes care, because a windowed run measures the
display, not the renderer.** `--uncapped` does nothing: wgpu ignores
`AutoNoVsync` on macOS and stays on Fifo. Worse, this machine is ProMotion, so
the refresh rate itself moves — the same robot read 16.67ms in one run and
8.33ms in another with *more* work in the scene, because the panel had settled
at 60Hz in the first and 120Hz in the second. Both are exact multiples of a
refresh interval, which is the tell. The numbers above come from `--offscreen`,
which renders into an image and leaves the window empty: with nothing
presenting to a surface, there is no vsync to wait on and the frame time is the
work.

**Two things the table is not.** The copies share 24 distinct meshes and 24
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

### The material data, and what the file cannot say

`scripts/fix-glb-materials.py` rewrites the two PBR fields and writes a new
GLB.

- **`metallicFactor` is 0.5 on every shape `RobotData` calls `phong`**, and
  `roughnessFactor` is 0.5. Neither is derived from the authored material:
  Phong has no metalness at all, and the shininess the file still carries (30)
  would convert to a roughness of 0.25. At metalness 0.5 half the base colour
  stops being diffuse and becomes specular reflectance, which without an
  environment map simply goes missing. The script sets metalness to 0 and
  roughness from shininess — a fit, not an authority, to be replaced the moment
  real values exist.
- **Colour is carried faithfully**: `RobotData`'s colour equals
  `baseColorFactor` on every shape.
- **A black base colour with a near-white `emissive` is a lamp**, not a
  contradiction. It is how a self-lit part is authored, so that it glows evenly
  and scene lighting cannot touch it. On Quori these are the bands at the top
  of the pole and around the base and the ring around the chest button, and the
  script leaves them alone.

**The real material description is `RobotData`'s**
`color`/`specular`/`shininess`/`emissive`/`opacity`, and Studio rebuilds a
Phong material from it. A renderer reading the glTF's PBR values inherits
numbers nobody authored, so a scene crate that wants to match Studio drives
materials from `RobotData` — as the face crate already does for faces, where
it reproduces the web's ambient-Lambert model.

**And `RobotData` cannot express PBR.** Its `standard` kind carries `color` and
`opacity` and nothing else, while Bevy, glTF 2.0 and three.js all support
metallic-roughness in full. The schema is the only layer that cannot, which is
why `standard` parts render as flat matte plastic — they fall back to three.js's
defaults of metalness 0 and roughness 1. Reaching parity means metalness and
roughness on the authored material; glass over the screen additionally means
transmission and IOR, which are a different axis from `opacity`.

This spike still reads the glTF material and lights the scene with numbers
picked for the spike, so its absolute colour matches nothing in particular.

## 3. The face on the screen

`scene --face Quori_Current_Extended.glb` draws a Vizij face into the screen
the robot declares. It costs **1.05ms** on top of the bare robot — 3.45ms
against 2.40ms — for a second camera pass into a 254x160 target and 15 more
meshes. The face crate reports `95 bindings over 18 elements`, so it is bound
and drivable, not merely drawn.

The path itself was short, because the face crate already had most of it:
`OffscreenTarget` puts a `RenderTarget::Image` on the face's camera, and the
quad's `StandardMaterial` takes that same handle as `base_color_texture`. That
is the same shape as Studio's `RenderTexture` attached as `map`.

**What embedding it actually required**, and what this says about the split:

- **A render layer.** Render layers are read per entity and are *not*
  inherited, so a face sharing a world with a scene needs its layer stamped on
  every entity its glTF spawned, not on the root. Without it the face's camera
  draws the robot and the robot's camera draws the face. Added as an optional
  `FaceLayer` resource, applied to the camera and, in `index_scene`, to the
  whole spawned subtree.
- **A scope for the name lookup.** `index_scene` matched elements against
  `Query<(Entity, &Name)>` over the *entire world*. A robot's links and joints
  carry names too, so embedding could bind a face feature to whatever else
  happened to share a name. It now walks the face's own subtree, marked by a
  new `FaceRoot`.
- **Nothing else changed.** Both are additive; a host that inserts no
  `FaceLayer` behaves exactly as before.

**The limit this leaves** is that `Face`, `PoseFeed`, `ViewOptions`,
`OffscreenTarget` and `FaceLayer` are all *resources* — one of each per world —
so one app can host exactly one face. Studio needs more than one: several
robots in a scene, or a picture-in-picture preview beside the scene. Making
that work means these become components on a face-instance entity and the
systems iterate instances rather than reading a global. That is the single
biggest structural item, and it is the one that should decide what
`arora-viz-core` looks like.

A smaller one: the face re-renders every frame whether or not its pose moved.
Studio's `RenderTexture` has the same default. Gating it on a pose change is
the obvious saving, and it is what makes many screens affordable.

### What the scene pulled from the face crate

Two things, and only one of them is about faces:

- `ViewPlugin` with `Face` / `PoseFeed` / `ViewOptions` / `OffscreenTarget` /
  `FaceLayer` — drawing a face into a texture.
- `meta::glb_json_chunk` — reading a GLB's JSON chunk, which the scene needs
  for `RobotData` and which has nothing to do with faces.

The second is the seam. A shared foundation starts with GLB/`RobotData`
reading and the render-target plumbing; the face-specific half stays in the
face crate.

## 4. The joint gizmo

`scene --joint neck_yaw_joint` puts a limited rotator on a joint. All 15
joints parse with their authored ranges — `revolute` x2 (`tilt_joint` ±0.785,
`neck_yaw_joint` ±1.571), `prismatic` x1 (`head_lift_joint` ±0.1m),
`continuous` x4 (0..2pi), `fixed` x8 — and driving one past its range holds it:
asked for 9.0, held at 1.571.

**Binding a joint to what it moves runs through the node index.** A robot's
glTF nodes have no names, so Bevy names them `GltfNode{index}` — and that is
the only thread tying `RobotData` back to a spawned entity. `RobotData`'s
`child` is a uuid, so it resolves uuid -> node index -> `GltfNode{index}` ->
entity. Studio never needs this: three.js hands the extension to the
`Object3D` it belongs to.

**What Bevy does not give.** Studio's `rotational-joint-gizmo.tsx` is a drei
`PivotControls`, which brings the ring, hover and annotation states, the drag
projection and `rotationLimits` with it. Bevy has immediate-mode `Gizmos` for
the drawing — an arc over the range, stops at each end, a spoke at the current
value, all a few lines — and nothing for the interaction. The drag here is
pointer-x times a constant, which is enough to show the clamp but is not a
control: a real one projects the pointer ray onto the joint's plane, needs a
pickable handle rather than a gizmo line, and has to carry hover and selection
states of its own. That gap is the honest cost estimate for this piece.

## 5. Bevy 0.19 details this cost time on

- **Gizmo primitives disagree about their own plane.** `circle` draws in the
  XY plane about +Z; `arc_3d` starts at +X and sweeps about **+Y** through XZ.
  A frame built for one and used for the other puts a ring and its arc at right
  angles. And `arc_3d` over a full turn does not close into a circle — it
  degenerates to a line, so a complete ring needs `circle`. Both faults look
  like a wrong axis, which is the expensive part: derive one explicit basis and
  orient each primitive from it.
- Gizmos depth-test by default, so a control drawn inside the geometry it
  controls is buried in it.
- Bevy UI draws to the primary window unless given a `UiTargetCamera`, so a
  label is missing from every offscreen capture while looking right on screen.
- The bundled fallback font is ASCII only; a degree sign renders as tofu.
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

## 6. Reading a frame back

`--screenshot` renders the main camera into an image and reads that back.
Two things make the obvious route fail silently instead of erroring:

- `Screenshot::primary_window()` returns pure black — not even the clear
  colour — whenever the window is not the composited frontmost surface, which
  is the normal case for a run started from a terminal.
- `Image::new_target_texture` sets every usage needed to render *into* a
  texture and not `COPY_SRC`, so a readback returns the zero-filled CPU side.

Both produce a black PNG, which is indistinguishable from a scene that drew
nothing. The visibility count in the report exists so that case cannot be
mistaken for a fast one.
