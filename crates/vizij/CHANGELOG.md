# Changelog

All notable changes to `vizij`, the desktop app. The format follows
[Keep a Changelog](https://keepachangelog.com/); versions follow
[Semantic Versioning](https://semver.org/). A version reaching `main` is
released by `release-vizij.yml`: a binary and an installer per desktop OS, and
the browser module, attached to the `vizij-v<version>` GitHub release.

## [Unreleased]

### Added

- The device describes itself as its face's model, which arora registers
  with Semio Studio: the family `vizij`, the face's hardware version — its
  bundle's `metadata.hardwareVersion`, else its `faceId` — and its software
  version, `metadata.softwareVersion`, when the face states one. The rig of
  each device generation describes its face, so a reload connects the Studio
  bridge with the new face's model, and the Studio bridge is built with the
  description (`connect_with_description`,
  `connect_with_operator_and_description`) where it was built with none.
  arora 15 builds it with none in a given device directory, so the Android
  app reports no model. The browser device describes its face the same way.
- The settings pane (`vizij::settings`, feature `settings`, which `desktop`
  enables and the Android app is built with): the Vizij standalone's
  Settings page over the face, drawn with egui (bevy_egui 0.42). A tap or
  click on the face shows its Settings button; the pane holds the face shown
  and Open a face… (the file dialog on desktop, the document picker on
  Android), the background (swatches and a colour picker), kept per face in
  `settings.json` in the app's data directory, and the device's standing
  with Semio Studio, what it is registered as and its association. Close,
  the Android back gesture and Escape close it; on Android the back gesture
  with the pane closed moves the app to the back, and the pane keeps clear
  of the system bars and the display's cutout.
- The pane is the device's operator (`settings::PaneOperator`, through
  `native::Mode::OperatedBy`): the registration's questions (`ask_text`) and
  the access requests (`decide_access`, allowed after
  `DEFAULT_ACCESS_GRACE` unanswered) open it. In the desktop window it
  answers in place of the terminal UI; `--headless` keeps the terminal. The
  device serves its Studio bridge's access requests through its operator.
- The pane's Device section, on desktop and Android alike: a device not
  associated shows the form its first association takes — its owners, one
  Studio user id per row with Remove and Add owner, and a name, generated
  when left blank — which Associate with Semio Studio registers; an
  associated one shows what Studio holds of it, read only, its name and
  owners being edited in Semio Studio. Each owner is registered as an
  owner: arora 15's device info carries no other role.
- A device registers the identity it keeps (`studio-identity.json`) only
  while Studio holds no record of it; otherwise it keeps Studio's record as
  its identity. A desktop that keeps no identity but has the registration
  arora's operator saved (`native::StudioConfig::saved_registration`) is
  associated as that.
- `RuntimeHandle::studio` says where the device stands with Studio
  (`native::StudioStatus`, whose `Idle` carries what a device not connected
  is associated as); `RuntimeHandle::associate` connects a run that did not
  ask for Studio for the rest of the run, the device restarting on its face
  with the Studio bridge. `native::StudioConfig::connect` (`native::Connect`)
  says when a device connects: at launch (`--studio`), at launch when it
  keeps an identity (Android), or once asked (the desktop without
  `--studio`). On Android, a fresh install registers with Studio through
  the pane's association form; one nobody associated still contacts nobody.

- The Android app: an APK around the library (`android/`, a Gradle project
  over the framework's `NativeActivity`), whose entry point runs the face it
  ships as a stand-alone device with the open local bridge, full screen,
  logging into logcat. minSdk 24, targetSdk 34, arm64-v8a and x86_64. CI's
  `android` workflow builds it and runs it on the x86_64 API 35 emulator
  with lavapipe, asserting that the face draws and animates with nothing
  caught in logcat.
- The Android app's activity, `VizijActivity` (Kotlin, a `NativeActivity`):
  a face opened from the system's document picker (a long press on the
  face) or with Vizij from a file manager (VIEW intents) is copied to the
  app's files directory as `model.glb` and shown; the app opens on the face
  last opened, the shipped one when there is none. Full screen in immersive
  sticky mode, the screen kept on. A lost GPU device recreates the renderer
  rather than ending the app.
- The Android app is built with the Studio bridge: its credentials live in
  the app's files directory, and the device registers under the identity
  kept there (`studio-identity.json`). `native::BridgeConfig::studio` is a
  `native::StudioConfig` (the identity file and an optional device
  directory) rather than the identity file's path.
- The Android app speaks: `say` goes to the cloud provider at its default
  deployment (vizij-arora-tts 7.1, whose HTTPS client trusts the bundled
  webpki roots), its audio played through cpal's oboe backend. The Android
  smoke invokes `say` through the open local bridge and passes when the run
  succeeds with the face reporting the utterance and showing its visemes.

### Changed

- A face's graphs run on arora-behavior-tree (vizij-arora-behavior 8): each
  frame evaluates the nodes the graphs' outputs read whose inputs changed,
  so a face at rest costs less per frame. The graphs compute what they did
  before. The device is built and ticked on the app's worker thread, as
  before.

### Fixed

- The browser module's `invoke` and `setValue` (and `writeValues`) read a
  value's text straight into an Arora `Value`, through
  `vizij_api_core::json::parse_value_str`. A large record no longer goes
  through a `serde_json::Value` tree that held about sixty times its text in
  the wasm heap; growing the heap that far made the first such call
  superlinear in the record's size. A form the normalizer does not recognize
  still reaches Arora as given, for Arora to report. `moduleAnimation`
  serializes its answer without such a tree either.

## [1.0.0] - 2026-10-24

### Changed

- `--ros2` requires `arora-bridge-ros2` 11.1, which speaks ros2-client 0.14
  (RustDDS 0.14): a reliable writer's send window is
  `ResourceLimits::max_samples` (8192) rather than its history depth, so a
  reliable writer (`/diagnostics`, a skill action's feedback, status and
  result) no longer stalls on a reader behind by one sample. 11's ROS4HRI
  preset serves
  `/skill/set_expression` as a topic skill spawning a `set_expression`
  method; Vizij asks for it on the `standard/ros4hri/expression/*` keys the
  face's graphs read, as before. The preset's `set_facial_action_units`,
  `set_viseme` and `play_visemes` skills name methods the device does not
  describe, so Vizij leaves them out of the profile it serves.
- The browser module's Vizij devices keep no time until their Vizij is
  first drawn (`ready`). Until then every `step` is a step at a zero dt,
  whatever dt the page passes; the first step after advances the time since
  the frame that drew the Vizij, at most the page's dt; and a `run` called
  before then starts its loop at that frame, or not at all if `stop` comes
  first. A face's springs, programs and animations start from rest where
  the page first shows them, however long its load and first draw took. A
  device with no Vizij (`fromGraph`) keeps time from its first step.

- The animation module's `play_at` counts on the device's clock: the
  animation source steps the module with `arora/time`, so a client that
  schedules a start at a device time gets it there.

- A face's settings — its lipsync gain, eye spacing and gaze range
  (`vizij_arora_host::standard::FACE_SETTINGS`) — rest in the store at the
  value the face's graphs author, so `reset` returns them to the face's own
  value. Every other key a profile types rests at the profile's rest, as
  before.

- The ROS4HRI mapping aims the eyes with the face's eye spacing and gaze
  range (`standard/vizij/gaze/{eye_spacing,range}`), which a face authors in
  its adaptation and a deployment overrides by writing the key; a face
  authoring neither is aimed as before.

- The browser module's `loadGraph` and `applyGraphEdits`, and the
  interpreter module's LOAD and EDIT over any bridge, refuse a graph that
  would fail every evaluation, such as one leaving a `remap`'s `in` unwired,
  with an error naming the node and the port. The face's running graph and
  its program and skill runs go on as they were.

- The browser module's operations that change a device settle on a device
  nothing steps: `loadGraph`, `applyGraphEdits`, `call`, `invoke`, `spawn`
  and `halt`, and `setValue`/`writeValues` with them. While no `run()` is
  going, a `step(0)` in the next microtask applies them, shared by the
  operations and writes of one turn, in the order they were issued. The
  graph a `loadGraph` or an `applyGraphEdits` leaves has run once in that
  step when its promise resolves. A page that steps in the same turn
  applies them with its own step; under `run()` the run's next step applies
  them, and the page adds no step. `listKeys` and `describeMethods` answer
  at once, with no step. A call issued from inside a step, by a callback the
  device makes, is an ordinary call: the next step applies it, a `step(0)`
  once the current step has ended while no `run()` is going (arora-web 10.2).
- Builds on arora 15 and, in the browser module, arora-web 12: the device's
  behavior-tree interpreter receives its method index at each tick
  (arora-behavior-tree 9).
- **Breaking.** A WebAssembly guest loads only as a component of Arora's
  `arora:module` world, built for `wasm32-wasip2`, whose header names the
  `wasm-component` executor; a `wasm32-wasip1` core module whose header names
  `wasm` no longer loads. A call to a guest whose arguments do not match the
  parameters its header declares fails, naming the parameter, and reaches no
  guest.
- **Breaking.** Under `--studio` the device speaks to Studio through
  `arora-studio-bridge-client` 10 (protocol 3): it publishes each key under
  its own path as it wrote it, a component's feedback under both
  `<id>.<axis>.position` and `<id>.position.<axis>`, which a Studio reads
  from its studio client 10 on.
- **Breaking.** A face's animations play through Arora's animation module,
  `@arora/animation` 3.0.0 (`arora-animation-module` 2, from crates.io),
  host-linked in place of Vizij's `@vizij/animation`: same module, function
  and parameter ids, one state per device.
  `modules::animation::module_animation` gives the module's format 2.0
  `Clip`, which `face::load_animations` loads with `load_clip`, as the
  browser module's `moduleAnimation` gives it for a page's `load_clip`.
  Each segment plays as its left keyframe's interpolation says, every side
  of every keypoint stated: `linear` with the linear easing; `step` held to
  the next keyframe by a keypoint there carrying the held value; `cubic` as
  glTF's `CUBICSPLINE` Hermite, on the keyframes' authored `inTangent` and
  `outTangent` and the segment's chord slope where one is absent, so an
  untangented `cubic` plays the straight line its keys make, as the
  authoring timeline draws it, where it played the ease (0.42, 0)/(0.58, 1).
  Keyframes keep their signed nanoseconds and an animation without a
  duration plays to its last keyframe. What the module changes, as a
  device's clients see it:
  - numbers play as `F64`, and an animation writes each key in the type
    the store declares for it (`KeyMeta::ty`, vizij-arora-behavior 7.3): a
    face's rig inputs keep holding `F32`; a key with no declared type takes
    `F32`, Vizij's number type, which keeps pages that read `.f32` working;
  - a player never moves back to an earlier device time: a client that
    steps the module directly, ahead of the device's `arora/time`, holds its
    players until the device's clock passes that time;
  - a player `play_at` holds before its time reports `waiting` with the
    state `playing`;
  - a command takes effect at the device time of the last step, so the
    player states read right after it show it;
  - a window no longer moves the playhead: a player outside it plays toward
    it;
  - travel past a bound carries on past the wrap or the turn;
  - `set_loop` takes `bounce`; `PlayerState` (1.3.0) adds `reversed` and
    `waiting`, `InstanceState` (1.2.0) adds `rate`;
  - a call the module refuses answers `u32::MAX`, which
    `face::load_animations` reports as an error.
- A device stepped with no time in it — the browser module's `step(0)` —
  carries changes through the face's graphs and runs and moves nothing that
  time drives: the springs, dampers and slews of its graphs,
  programs and skills hold with the clocks, oscillators, noise and
  animations (vizij-graph-core 3); an edit that reaches one of them is its
  new target, shown once time passes. They used to jump to their targets.
  A `say` run follows its audio, which plays in real time.
- `face::builder_with_guests` returns the device's `KeyHolds` with its
  builder.
- **Breaking — the next release is 1.0.0.** A face is ready —
  `Face::ready`, a new field, `view::all_faces_ready` (what the snapshot
  harness waits for) and the browser module's `ready` — once the GPU has
  finished a frame drawing every mesh visible when it was drawn, each with
  its pipeline compiled, rather than once its scene is indexed; that is
  `Bindings::indexed`, which was `Bindings::ready`. Until then its camera
  draws it under the clear, so the target shows the previous face or the
  background, never a face with meshes missing; a reload's previous face
  draws until the new one is ready. Under a software GPU, where a face's
  first draw compiles its pipelines for seconds, a page no longer stalls
  right after `ready`: from it, the page draws on. Should something keep a
  face from being drawn — a material that never loads, a mesh whose
  pipeline cannot be specialized, a camera that draws no frame — a warning
  names it after 120 frames, and after 600 (ten seconds at 60 Hz) the face
  is ready as it draws.
- The view lands only the pose values that changed since the last frame,
  read from the rig in place: an unchanged transform, morph weight or
  surface is not marked changed, so it is not propagated or uploaded
  again. Applying a face's pose takes 0.2 M instructions a frame instead
  of 2.2 M.
- The view's cameras cluster no lights — the faces are lit by the ambient
  term alone — so the renderer builds one cluster per view instead of a
  grid each frame: 8 % fewer instructions to render a frame.
- `--ros2` requires `arora-bridge-ros2` 10.4, whose ROS4HRI preset takes
  `/closed_captions` and publishes `/diagnostics`.

- `--ros2` requires `arora-bridge-ros2` 10.3, which escapes a key path into
  valid ROS 2 name tokens: a Studio-exported face's keys, named by UUID, get
  topics (an id starting with a digit gains a leading `_`).

### Added

- The browser module's `holdKeys(keys)` and `releaseKeys(keys?)` on a
  Vizij's device: held store keys keep the values clients write, against
  every write of the face's composition — its graphs, its program and skill
  runs, its animations — until released. A release settles as the device's
  operations do: on a device nothing steps, a step at a zero dt in which
  the released keys take their writers' values; under `run`, the run's next
  step.

- Captions over the face (feature `captions`, on by default): the face
  standard's `standard/vizij/caption/{text,speaker}` shows at the bottom of
  each face's rectangle, in eight-word chunks held long enough to read and
  queued behind the caption showing, blue for the face's own speech
  (`__system__`) and white otherwise, in the embedded Inter font. It is in
  every frame the view publishes. Under `--ros2` a closed caption on
  `/closed_captions` (`hri_actions_msgs/ClosedCaption`) captions the face
  through the ROS4HRI mapping; a face whose composition writes neither key
  has both open to any client.
- Under `--ros2` with the ROS4HRI profile, the device's health on
  `/diagnostics` once a second: the view's frame rate, and the face's id,
  its compliance level with the face standard and every path its
  composition writes from more than one source, a warning.
- A face's free input outside the shipped profiles takes its range, as
  `list_keys` states it, from: its input node's `params.min` and
  `params.max`, when both are there and `max` exceeds `min` (both optional);
  else the `range` the rig declares for it in `metadata.vizij.inputs`, as the
  authoring app exports for every rig input; else `[0, 1]`. A key a shipped
  profile declares keeps that profile's range; types are unchanged.
  `face::declare_keys` takes the rig's ranges, which `face::rig_ranges` reads
  off the bundle.
- The browser module's `channelKeys(gltf, channels)`: the store key each
  animation track channel drives on the Vizij whose glTF JSON document is
  `gltf`, without loading it — one key per channel, resolved through the
  bundle's rig as the loaded Vizij's `moduleAnimation` keys the track.

### Fixed

- An animatable that several elements share moves every one of them, as
  the three.js view does: shapes that share a material share its colour and
  opacity animatables, and a pose write to one recolours them all. It used
  to move only the last element the GLB lists. `Binding::node_names` lists
  every element an animatable drives (it was `node_name`), the view's
  `Bindings::by_id` holds a place per element, and the browser module's
  `describe` lists them in each animatable's `nodes`. An element that binds
  an animatable another binds to a different feature is skipped, with a
  warning.
- A face whose RobotData ids are no UUIDs loads: an element's and an
  animatable's id is any string, as the published three.js line reads it.
  It used to fail with "bad RobotData on node …: UUID parsing failed".
  `FaceMeta`'s element ids and animatable keys, the view's bindings, picks,
  selections, static-feature edits and holds take the id as written; on a
  per-axis feature, an id ending in `.<axis>` is still Studio's component of
  a compound animatable.
- The browser module requires `arora-web` 9.0.1, which holds what a page
  has not drained as the latest value per key: a page that never calls
  `drainChanges` no longer grows its memory with every step. A drain
  answers each changed key's latest value, as before.
- An ellipse's or a rectangle's fill named per axis, as Studio writes it
  (`fillColor.r`, `fillColor.g`, `fillColor.b`), colours it: a static
  component states that component of the fill, over a whole `fillColor`; an
  animated one binds that component of the colour, or the whole colour for
  a component `<id>.<axis>` of a compound animatable. Such a fill used to
  draw white and bind nothing. A per-axis stroke colour is dropped with the
  rest of the stroke.

## [0.2.0] - 2026-10-12

### Added

- Under `--studio` a target is taken in either of two forms: `k/target.c`
  as `arora_hal::set_targets` reads it — `<id>/target.position`,
  `<id>/target.velocity`, a component's `<id>/target.position.<axis>` — or
  in Studio's current spelling, `<id>.target_position`,
  `<id>.target_velocity`, a component's `<id>.<axis>.target_position`. Both
  are listed as open `f64` inputs — a position resting at the animatable's
  default, a velocity at 0 — and the device keeps them in agreement: a
  write in either form lands in both, so the latest write wins; an update
  carrying both forms of one target takes the `k/target.c` form's value, as
  `set_targets` does. A component's feedback is published under both
  spellings of its axis, `<id>.position.<axis>` beside `<id>.<axis>.position`.
  `<id>.studio_value` stays an input that drives nothing, until Studio no
  longer sends it. The device depends on arora-hal 4.2.
- A ROS4HRI viseme stream plays each viseme for its `duration`, and a
  repeated shape at a new `time` plays again, from the `time` and `duration`
  arora-bridge-ros2 10.1's ROS4HRI preset lands with each viseme. The ROS 2
  bridge is arora-bridge-ros2 10.1.
- Under `--studio` the face serves Semio Studio's keys (`vizij::studio`, the
  Studio-key profile, composed into the face's behavior from its RobotData
  bindings): Studio's `<id>.target_position` lands under the animatable id
  the view indexes — a vector, euler or colour written per component,
  `<uuid>.<axis>`, lands joined under `<uuid>` — and `<id>.position`
  reports it back; `<id>.studio_value` and `<id>.target_velocity` are
  accepted and ignored, so Studio's whole update for an entity lands. Each
  target rests at the animatable's RobotData default. Studio's keys for an
  animatable the face's own graphs write, or one bound to nothing the view
  draws (`view::meta::FaceMeta::unbound`), are accepted and move nothing.
  `FaceConfig::studio` turns it on (feature `studio`), and
  `view::meta::Binding::rest` carries the default.
- A face's rectangle is any rectangle of its target
  (`ViewEvent::PlaceFace` takes an `IRect`; the browser module's
  `placeVizij` any CSS rectangle): the face is framed on the whole rectangle
  and the part inside the target draws — a sub-camera view of the whole
  rectangle's projection — so a rectangle reaching past an edge shows its
  face cut off there, at the position and scale the whole rectangle gives
  it. An empty rectangle, or one wholly outside the target, draws nothing
  and takes no picks; a face never placed draws over the whole target. The
  safe area follows the whole rectangle. A camera of its own clears the
  target with the background, below every face's, so the target shows the
  background wherever no face draws, also when none does.
- The browser module's `outputKeys(graph)`: the store keys a run of `graph`
  writes and leaves holding when halted (`vizij_arora_behavior::run::outputs`),
  what `reset_keys` takes to return a halted program's outputs to rest.
  `describe` lists each graph with its `id` beside its `kind`.

- The face reports what it is saying: while a `say` run's audio plays, the
  utterance is the face's speech state, `standard/vizij/speech`, empty before
  playback starts and once it ends. Under `--ros2` it is published on
  `/robot_face/speech` for subtitles. Both providers (cloud and Piper) report
  it; the `say` example prints when speech starts and ends.
- The browser module's device (`VizijRuntime`) has the surface any client
  of an Arora device has, through `arora-web`: `call`, `invoke` (a described
  method by name), `spawn`, `halt` (by run id), `listKeys` and
  `describeMethods`, beside the store accessors and the interpreter's LOAD
  and EDIT (`loadGraph`, `applyGraphEdits`). What a face does goes through
  its modules' and interpreter's declared methods by name. Two codecs stay
  the module's own: `behaviorValue(graph)`, a graph spec as `run_behavior`'s
  `behavior` argument, and `runEdits(run, from, to)`, the edits that change
  a run's behavior in place. `describe` lists each program with its label
  and graph.

- A face's program runs beside its graph as a task run of the device's
  interpreter — its `run_behavior` method, spawned through the interpreter
  module like the skills — natively and in the browser
  (`face::spawn_program`), instead of being composed into the graph. Any
  client finds the run by the program's id in the store and halts it; its
  outputs then hold their last values. `--program` and `--no-autoplay` choose
  the program that runs from launch, as before.
- The `rest` module's `reset_keys(keys)` returns only the keys it names to
  rest — how a client returns a halted program's outputs to rest.
- The keys a face declares cover its programs: the inputs each program reads
  are opened like the graph's free inputs, and a key a program writes stays
  an input, resting at its authored default.

- A face's animations (its bundle's `animations`) load into its device when
  the face loads, natively and in the browser, through the animation
  module's declared functions — the calls any client sends
  (`face::load_animations`): `load_animation`, `create_player` named after
  the animation's id, `add_instance_with_weight` at 0, `stop`. Each track
  writes the rig input its channel names. A loaded animation is silent until
  played: a client finds its player by name in `player_states` (the
  `vizij/animations/players` key), gives its instance weight with
  `set_weight`, then drives it with `play`, `pause`, `stop`, `seek`,
  `set_speed` and `set_loop`; `remove_player` and `unload_animation` unload
  it. The browser module's `loadVizij` takes `animations: false` to leave
  them out.
- The animation module's `reload_animation(anim, clip)` replaces a loaded
  animation's tracks under the same id, its players keeping their playback
  and its instances their weights. A player keeps the speed `set_speed`
  gave it through `pause` and `stop`, and `set_speed` neither resumes nor
  pauses it.

- Faces exported from Semio Studio read: their per-axis RobotData features
  (`translation.x`, `rotation.r`, `color.g`, …) bind, an axis driven by an
  animatable of its own moving that one component, a component `<id>.<axis>`
  of a compound animatable binding `<id>` to the whole feature. A per-axis
  name also sets a static feature (`setStaticFeature`, `SetStaticFeature`).
- A RobotData `ellipse` or `rectangle` draws as a shape: a unit circle or
  1×1 plane, scaled by its `width` and `height`, where its node carries no
  mesh, filled by its `fillColor` and `fillOpacity`. Its stroke features are
  dropped with a warning.
- A GLB without RobotData — a plain Blender export — draws under the web's
  material model and is framed on its scene's bounding box, as the web's
  import derives it; so does a face whose RobotData declares no
  `rootBounds`. Quori's Blender export joins the snapshot references.
- The `shininess` and `specular` features of a `phong` material bind; under
  the ambient-only light they change nothing, as in the web renderer.

### Changed

- `face::builder_for` and `face::builder_with_guests` load the rest module
  themselves, over the device's store, so `reset_keys` naming either form of
  a Studio target rests both; a caller no longer adds it.
- A free input is typed as its default declares, so its ROS 2 topic matches
  the face: a number is `f32`, the graph's number type (a rig input like
  `mouth/smile` is subscribed as `std_msgs/Float32`, as are inputs with no
  default), and Studio's keys are `f64`, as Studio writes them. A numeric
  input outside the shipped profiles was `f64` whatever its default said.
- The browser module composes the built-in ROS4HRI mapping when a page asks
  for it: `loadVizij` and `composeVizij` take `ros4hri`, default `false`, and
  a page's face speaks the vizij standard (`standard/vizij/*`). With the
  mapping come the ROS4HRI keys, its idle blink and the `look_at` skill's
  gaze. The binary composes the mapping unless `--no-ros4hri`.
- A face's standard controls are declared as the `vizij-face` profile
  states them (`face::declare_keys`): each `standard/vizij/*` input is an
  `F32`, an eye position in `[-1, 1]` and a weight in `[0, 1]`, resting at
  the profile's rest, so a remote write of one is taken only as an `F32`.
- The ROS4HRI mapping shows a named expression at the commanded arousal as
  its intensity (clamped to 0..1) instead of at full weight; an arousal of 0
  or below shows none.

- Opening another face (`O`, a dropped `.glb`, a reload) keeps the current one
  on screen until the new one is ready, instead of an empty window meanwhile.
- A `phong` or `lambert` element is shaded without metalness, as three's
  `MeshPhongMaterial` and `MeshLambertMaterial` are: its diffuse term no
  longer dims by the `metallicFactor` its GLB material carries (0.5 from
  three's exporter).
- Every mesh of a face's scene is drawn under the web's material model, not
  only those of its RobotData elements: a mesh no element declares is shaded
  as three's glTF loader makes its material (basic for
  `KHR_materials_unlit`, standard otherwise).

### Removed

- The browser module's device no longer carries calls of its own for what
  Arora's client surface does: `spawnSkill` (`invoke` the skill by name),
  `reset` (`invoke("reset")`), and `call`'s inference of a missing
  `module_id`. A run is halted by its id (`halt(run)`).

### Fixed

- A face whose RobotData element sits on an unnamed glTF node is indexed:
  the element joins the node under the name Bevy's loader gives it
  (`GltfNode<index>`).
- A face whose RobotData carries static features (a value in place of an
  animatable) loads; it was refused as bad RobotData.
- A face comes into view only once it is indexed, posed and shaded. A load
  or a reload no longer shows one frame of the GLB as it spawned, and during
  a reload the face being replaced is the only one drawn until the swap.
- A face's behavior evaluates its sources in the order they compose: the
  face's own graphs, the mappings (ROS4HRI, Studio), the animations, then the
  runs (the program and every skill, in spawn order). Of two writers of one
  path the later wins, so a playing animation overrides a mapping, and a
  skill overrides a playing animation. The sources used to evaluate in the
  order of their prefixes, the animations first.

## [0.1.0] - 2026-10-02

### Added

- The `vizij` binary: a window over the Bevy view showing a face from its GLB
  (`--glb` takes a path or a URL and is remembered; a `.glb` dropped on the
  window or opened with `O` loads), with the face's Arora device behind it.
- The open local bridge on `--bind:--port` (`ws://127.0.0.1:9000` by default),
  with the control panel on the same port unless `--no-web-control`. A client
  lists the device's keys with their `__meta`, writes the face's inputs, calls
  the face's skills by name and `reset`.
- `--studio` registers the device with Semio Studio. The released binaries
  carry it; `--ros2` (a ROS4HRI face on a ROS 2 graph) is a build feature they
  leave out.
- Window flags: `--fullscreen`, `--display`, `--width`/`--height`,
  `--no-decorations`, `--always-on-top`; `vizij list-displays`.
