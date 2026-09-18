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

**Three things the table is not.** The copies share 24 distinct meshes and 24
materials, so Bevy batches them; the triangle throughput is real but the
draw-call count is optimistic against 1,536 genuinely distinct objects. And
this is desktop-native — Studio ships to the browser, where the same scene runs
through WebGPU in wasm and will cost more. The native figure bounds the
question from above; it does not settle the browser one.

**Third, and easiest to misread: this is a static scene.** Every run installs
`PoseFeed::new(Vec::new)`, and the copies runs spawn no controls, so nothing
animates, no transform or material is written, no morph weight updates and no
pointer interacts. The numbers are geometry, shadows and an orbiting camera,
with the face redrawing an unchanging pose every frame. **Read them as a
floor.** They answer whether the GPU can draw this; they say nothing about the
three write paths a live edit session exercises — a transform write, a material
mutation that may re-upload to the GPU, and a morph-weight update — none of
which has been timed at any scale.

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

## 7. What this means for vizij and a shared core

The scene crate is built on the face crate and reuses two pieces of it, so
most of what it found is a statement about `vizij-render` rather than about
scenes. Each item below is either a change vizij needs on its own terms, or a
constraint on what `arora-viz-core` can hold.

### Per-instance and per-renderer state, and why that split is the extraction

`Face`, `PoseFeed`, `ViewOptions`, `OffscreenTarget` and `FaceLayer` are
resources, and `setup_scene` / `setup_camera` run in `Startup`. One face per
app, decided before the first frame — a host cannot load a face later, swap
one, or show two.

**What describes a thing being rendered is a component on that thing's entity;
only what describes the renderer stays a resource.** A resource is a claim
that there will never be two, and every asset a host draws is something there
will eventually be two of — two robots, a picture-in-picture preview, a face
on each of several screens. The fault is the same one the scene hit with its
gizmos, and it is cheaper to fix in the face crate first, because that is the
half with callers to migrate.

**Which state lands on which side is a requirements question, not a mechanical
one.** A resource never has to declare what it belongs to; a component does,
because it is attached to something.

**The test: would two faces drawn at the same time ever need different
values?** That splits the current resources three ways rather than moving them
all. `Face`, `OffscreenTarget`, `FaceLayer` and `ViewOptions`'s `fit`/`zoom`
belong to the face — identity, where it draws, which world it is isolated
into, and how it fits a screen all differ between two faces. `background`
belongs to the render target, because what shows where the face is *not* is a
property of the surface: a screen texture wants transparent so the quad's own
material shows through. `ambient`/`unlit` stay with the renderer — they
describe the face rendering model, the web's ambient-Lambert pipeline, not an
individual face, and two faces would share them until someone wants per-face
art direction.

`PoseFeed` is the one worth arguing about. One closure per face is the obvious
reading, but the JS already does better: one shared values store addressed as
`getLookup(namespace, animatable.id)`, the namespace identifying the instance.
Values here arrive as `(TypedPath, Value)` pairs, so carrying the instance in
the path costs nothing and keeps one crossing of the host boundary rather than
one per face — which matters most in wasm, where that crossing is a JS call.

The contents of a shared core are the answer to "what is shared", so they
follow this split rather than preceding it.

### A join must be scoped to the subtree it belongs to

`index_scene` matched `Query<(Entity, &Name)>` across the whole world. Once a
second asset shares that world, a face feature binds to whatever else happens
to carry the same name — silently, because a name collision is a successful
lookup. Scoped to `FaceRoot` now; the rule generalises to every join a core
performs.

The same shape applies to visibility: **render layers are read per entity and
are not inherited**, so stamping a layer on a glTF root leaves every spawned
descendant on the default layer. Anything that isolates one asset's rendering
has to walk the subtree.

### A binding is many-to-many in the schema, so it must be many-to-many in the map

`FaceMeta::animatables` and `BindingIndex::by_uuid` both stored one target per
key and built it with `insert`, so a later binding silently replaced an
earlier one. Two meshes' materials may share a pointer to one colour — that is
what the format allows and what makes controllables generic — and on the Quori
face 8 of 95 animatables drive more than one element. Widening both maps to
`Vec<_>` took indexing from 95 bindings to 113: 18 targets were being dropped.

**The JS side cannot have this defect, and the reason is the useful part.**
`useFeatures` (identical in `packages/vizij` and `packages/scene` once the
context is renamed) walks an *element's own* features and subscribes once per
animated feature, so the only direction stored is element → animatable. Every
`animatables[animatable.id]` in both packages maps an id to that animatable's
definition, never to a target; there is no reverse index to collapse. Many
elements on one animatable are many subscribers to one key, and one element on
many animatables is several subscriptions.

So this defect is not inherited from the format or from the JS design — it is
created by choosing to push. **A renderer that indexes controllable → targets
in order to push values has to make that index many-valued by construction**,
because it is the only one of the two designs that can lose a binding.

### An anchor has to name its coordinate space

`publish_anchors` projects with `Camera::world_to_viewport`, which is the
*render target's* pixel space, not the window's. When the face camera draws
straight to the canvas those are the same and a DOM overlay lands correctly.
When it draws into an `OffscreenTarget` that becomes a texture on a quad in
another scene — which is exactly what `OffscreenTarget` is for, and what a
robot's declared screen is — the published numbers are positions *on the
texture*. A host that reads them as screen coordinates draws its labels
somewhere else entirely, with no error anywhere.

So the anchor contract must say which space it is in, and an embedded face
needs a second projection through the outer camera and the quad before a host
can anchor to it. This is the one place where the face-in-a-scene case breaks
an existing vizij API rather than merely extending it.

### A library plugin must not claim a third-party plugin for the host

`ViewPlugin` adds `MeshPickingPlugin` unconditionally (`view.rs:164`), and
Bevy panics on a duplicate plugin. Any host that wants mesh picking for its own
scene — which a scene with selection and draggable gizmos does — cannot add it,
because the face crate already did. Guard it with `is_plugin_added`, or leave
it to the host and require it in the docs.

### GLB extension reading belongs in the core, not the face crate

`meta::glb_json_chunk` lives in `vizij-render` because that is where it was
needed first, but nothing about it is face-specific: **Bevy's glTF loader
parses the extensions it knows and drops the rest**, so every Semio asset read
in Rust needs its own pass over the JSON chunk. The scene crate reuses it
verbatim for `RobotData`.

What belongs in the core is that parse, the extension schema types, and the
uuid → node index → `GltfNode{index}` resolution that ties an extension record
back to a spawned entity. What stays in the face crate is the interpretation —
elements, animatables, feature kinds.

That last step is also the weakest link in a platform-agnostic core. A robot's
glTF nodes are unnamed, so Bevy's index-derived names are the only thread back
to an entity; a platform whose exporter *does* name its nodes breaks the lookup
and returns nothing. It needs to resolve by node index directly rather than by
reconstructing the name Bevy would have chosen.

### The screen is the seam, and it is already in the right place

A `screen` in `RobotData` is `{width, height, resolution}` with no mesh — a
declaration. Whoever renders the robot builds the quad and sizes the texture;
whoever renders the face fills it. `OffscreenTarget` is that handoff and needed
no change to serve it. The split between the two projects survives contact with
the case that crosses it, which is the argument for extracting a core rather
than merging the two renderers.

### Per-element materials are not a cost

Giving every element its own material instead of sharing measured 8.32 / 8.38ms
against 8.30ms shared — inside the run-to-run spread. Runtime-editable colour,
opacity and material properties per mesh therefore have no batching argument
against them, which is what vizij needs to hear before making material
properties controllable per element.

Left standing from §3: the face redraws every frame whether or not its pose
changed. That, not material sharing, is what decides how many screens are
affordable.

### State each convention once, in a type

Bevy's gizmo primitives disagree about their own plane, the imported model is
Y-up where the Studio scene is Z-up, and `Torus::new` takes inner/outer radii
where three's `TorusGeometry` takes radius/tube. Every one of these produces a
render that is wrong and does not error. A core should carry each convention in
a type that holds a basis — the scene's `JointPlane` is the small version —
rather than leaving each call site to rediscover it.

### What gates the extraction: none of this is measured on wasm

Every number here is desktop-native. `vizij-render-web` builds the face half
for wasm, so that path is known; the scene crate has never been built for
`wasm32`, and it pulls in `bevy_panorbit_camera` and embedded WGSL assets.
Frame times, and any decision about what a shared core may depend on, should
not be settled on native measurements alone.
