# Changelog

All notable changes to `vizij-arora-host`. The format follows
[Keep a Changelog](https://keepachangelog.com/); versions follow
[Semantic Versioning](https://semver.org/).

## [7.6.0] - Unreleased

### Added

- `contents::Keyframe::in_tangent` and `out_tangent`: the keyframe's
  `inTangent` and `outTangent` as the bundle carries them, the slopes a
  `cubic` segment arrives and leaves with, in value units per second (glTF's
  `CUBICSPLINE` tangents); `None` when absent. They serialize as `inTangent`
  and `outTangent`.

### Changed

- Reads graph specs and values with arora-shape 1, where it used
  vizij-api-core 2: the crate moved to arora-sdk under that name. No public
  item changes.

## [7.5.0] - 2026-10-19

### Changed

- `animations_source` feeds the device's `arora/time` to the animation
  module's `step` as its `time_ns`, beside `arora/dt`: `play_at(player, t)`
  counts `t` on the device's clock, where it counted from the module's first
  step.

## [7.4.0] - 2026-10-19

### Added

- `standard::GAZE_EYE_SPACING` (`standard/vizij/gaze/eye_spacing`, metres)
  and `standard::GAZE_RANGE` (`standard/vizij/gaze/range`, radians): how far
  apart the face's eyes are and how far they turn, settings of the face like
  its lipsync gain. The `vizij-face` profile declares both as inputs with no
  tier, the spacing from 0 and resting at 0.06, the range in [0.01, 0.9] and
  resting at 0.78; it has 98 keys.
- `standard::FACE_SETTINGS`: the face's settings — the lipsync gain, the
  eye spacing and the gaze range.

### Changed

- The ROS4HRI mapping aims the eyes with the face's eye spacing and gaze
  range, read under the face's rig prefix, where it held 6 cm and ±0.78 rad
  as constants. A face that authors neither is aimed as before. The mapping
  reads each within the span the profile declares: the spacing from 0, and
  the gaze range in [0.01, 0.9] rad, the span its arctangent reaches the end
  of an eye's travel over.
- `Bundle::sources` (and so `Bundle::compose`) rests a mapping's input on
  one of the face's settings at the default the face's base graphs author
  for it when that is of the setting's type, else at the `vizij-face`
  profile's rest, so a face states a setting once, in its adaptation, and
  every mapping composed over it reads that value.
- Licensed under Apache-2.0, where it was MIT OR Apache-2.0. Releases up
  to 7.3.0 keep the licence they were published under.

## [7.3.0] - 2026-10-12

### Added

- `standard::CAPTION_TEXT` and `standard::CAPTION_SPEAKER`
  (`standard/vizij/caption/{text,speaker}`): the caption a host shows over
  the face, empty at rest, and who says it — `standard::CAPTION_SPEAKER_SELF`
  (`__system__`, ROS4HRI's id for the robot) for the face itself. A command
  of the face's display, in no tier; the `vizij-face` profile declares both
  as string inputs and has 96 keys.
- `captions`: how a host shows a caption, as the incumbent ROS4HRI face
  does — eight-word chunks held for their words at 200 words a minute, at
  least 2 s, the last 1.5 s longer, queued behind the caption showing, blue
  for the face's own speech and white otherwise (`CaptionQueue`, `chunks`,
  `speaker_color`).
- The `ros4hri` profile declares `standard/ros4hri/caption/{text,speaker}`
  (`ros4hri::CAPTION_TEXT_KEY`, `ros4hri::CAPTION_SPEAKER_KEY`), which a ROS
  bridge writes from `hri_actions_msgs/ClosedCaption`, and the ROS4HRI
  mapping relays them as is to the face's caption.
- `diagnostics` (feature `publish-diagnostics`): the device's health as the
  `diagnostic_msgs/DiagnosticArray` the ROS4HRI exposure profile publishes
  on `/diagnostics` from the key `diagnostics` — a status for the view's
  frame rate, and one per face with its id, its compliance level and the
  paths its composition contends. Requires arora-msgs-ros2 2.2.
- `contended_outputs` and `Contention`: the paths more than one source of a
  composition writes, with their writers in composition order;
  `compose_sources` logs each, as it did.
- `Bundle::sources`, the graph sources `Bundle::compose` unions, and
  `Bundle::input_paths`, the paths a face's graphs read with its rig prefix
  stripped.
- `profile::coverage`, `profile::Coverage` and `profile::TierCoverage`: a
  face's compliance with the face standard, graded from its input paths —
  what `vizij-bundle validate` reports.

## [7.2.0] - 2026-10-12

### Added

- `standard::GAZE_TARGET_WEIGHT` (`standard/vizij/gaze/target/weight`): how
  strongly the eyes hold the gaze target, in `[0, 1]`, resting at 0. At 0 the
  eyes do what the face itself does, whatever the target says; at 1 they hold
  the target, with no more than micro-motion; in between, a blend. A face
  applies it wherever it implements the target. The `vizij-face` profile
  declares it in the gaze tier, resting at 0; the profile has 94 keys.
  `vizij-bundle validate` reports it with the target, ungraded.

## [7.1.0] - 2026-10-12

### Added

- `standard::GAZE_TARGET_X` and `standard::GAZE_TARGET_Y`
  (`standard/vizij/gaze/target/{x,y}`): the gaze target, a point on the
  face's frame in screen conventions — origin top-left, `x` to the right, `y`
  downward, each in `[0, 1]` — resting at the centre, (0.5, 0.5). A face
  implements it in its program, blended with the program's own eye motion,
  or in its standard adaptation, mapped onto the eye positions statelessly;
  never both. The `vizij-face` profile declares both in the gaze tier,
  resting at 0.5; the profile has 93 keys. The ROS4HRI mapping does not
  write them.

## [7.0.0] - 2026-10-12

### Removed

- **Breaking:** `ros4hri::au_key` and the `ros4hri` profile's 20
  `standard/ros4hri/au/<code>` keys. ROS4HRI publishes facial action units
  only as perception output (`hri_msgs/FacialActionUnits` on
  `/humans/faces/<faceID>/facs`) and defines no command for them; the profile
  has 10 keys, the expression, gaze and viseme inputs and the speech state
  outputs. A FACS-granular pose is the `vizij-face` muscle tier's,
  `standard/vizij/face/<control>`, which is unchanged.

### Changed

- The ROS4HRI mapping writes none of the muscle tier nor
  `standard/vizij/mouth/morph/jaw_open`. Its idle blink is inhibited by the
  `asleep` expression alone, which also holds the lids shut.

## [6.6.0] - 2026-10-10

### Added

- `standard::LIPSYNC_GAIN` (`standard/vizij/lipsync/gain`): the face's
  lipsync gain, which its adaptation scales every viseme weight by before
  mapping it onto a pose. The `vizij-face` profile declares it as an input
  with no tier, resting at 1, so coverage grading is unchanged; the profile
  has 91 keys.
- `ros4hri::SPEECH_VISEME_KEY` (`standard/ros4hri/speech/viseme`): the lip
  shape being spoken, a standard shape name, `sil` at rest, as
  arora-bridge-ros2 10.2's ROS4HRI preset publishes it on
  `/robot_face/viseme`. The `ros4hri` profile declares it as an output; the
  profile has 30 keys.

### Changed

- The ROS4HRI mapping relays the face's current viseme
  (`standard/vizij/viseme`) to `standard/ros4hri/speech/viseme`, as it relays
  the utterance to `standard/ros4hri/speech/text`.

## [6.5.0] - 2026-10-10

### Added

- `play_viseme` takes an optional `duration`, seconds: the shape holds until
  it has passed since the run's start, then ramps out. Without one, or with
  one not positive, it holds `VISEME_ATTACK + VISEME_HOLD` as before.
- `ros4hri::VISEME_TIME_KEY` (`standard/ros4hri/viseme/time`) and
  `ros4hri::VISEME_DURATION_KEY` (`standard/ros4hri/viseme/duration`): a
  streamed viseme's place in the utterance and how long it is held, seconds,
  as arora-bridge-ros2 10.1's ROS4HRI preset lands them. The `ros4hri`
  profile declares both as inputs, resting at 0.

### Changed

- The ROS4HRI mapping plays a streamed viseme for its `duration`, and a new
  `time` starts a new run even for the same shape, so a repeated viseme plays
  again. The asset needs vizij-graph-core 2.4's `spawn` `when` input.

## [6.4.0] - 2026-10-10

### Added

- `Bundle::graph_ids`: the `id` each of `Bundle::graphs`' entries carries, by
  position (`graph_ids[i]` is `graphs[i]`'s), `None` for an entry without
  one — the rig's, the pose driver's and the programs' ids beside their
  kinds.
- A `Bundle` built with a struct literal that names every field must name
  the new one, or end in `..Default::default()`.

## [6.3.0] - 2026-10-10

### Added

- The ROS4HRI mapping plays the streamed viseme: each new
  `standard/ros4hri/viseme` code spawns a `play_viseme` run of its shape at
  full weight, which takes over from the run before it, and `sil` lets the
  lips settle at rest. The mapping writes no lip key itself — the run does —
  so a viseme stream and the device's own `say` share the lips the way
  players do. The asset needs vizij-graph-core 2.3's `spawn` node.

## [6.2.0] - 2026-10-10

### Changed

- `Bundle::compose` composes the program after the animation source: base
  graphs, then mappings, then animations, then the program. A playing program
  overrides a playing animation on a path both write, as a program or skill a
  device runs as a task run does.

## [6.1.0] - 2026-10-06

### Added

- `ros4hri::VISEME_KEY`, `standard/ros4hri/viseme`: the lip shape a ROS4HRI
  TTS node streams at the audio playhead (`hri_msgs/Viseme.value` from
  `/tts/viseme` or `/tts/visemes`, which arora-bridge-ros2's ROS4HRI preset
  lands here), as a `u8` index into `standard::VISEME_SHAPES`, resting at
  `0` (`sil`). The `ros4hri` profile declares it as an input, 27 keys in all.
  Nothing maps it yet: the face's lips belong to whichever viseme player is
  running, and a mapping channel would clobber a run's weights (VIZ-162).

## [6.0.0] - 2026-10-06

### Added

- `standard::SPEECH`, `standard/vizij/speech`: what the face is saying — the
  utterance of a `say` run from the moment its audio starts playing until it
  ends, empty at rest. State the face reports, not a command.
- The `vizij-face` profile declares its state keys as `output` keys, the
  controls staying `input`: `standard/vizij/viseme` (resting at `sil`) and
  `standard/vizij/speech` (resting empty), 90 keys in all.
  `Profile::paths_of(kind)` lists a profile's keys of one kind.
- The ROS4HRI mapping relays the face's speech state to the device-scoped
  `ros4hri::SPEECH_TEXT_KEY`, `standard/ros4hri/speech/text`, the `ros4hri`
  profile's one `output` key, which arora-bridge-ros2's ROS4HRI preset
  publishes on `/robot_face/speech`.
- The say skill writes the provider's `speech` out-parameter as the face's
  speech state.

### Changed

- **Breaking:** the `Say` contract's `say` gains a mutable `speech: &mut
  String` out-parameter (`say::ids::say::SPEECH`): a provider reports the
  utterance while its audio plays — from the moment playback starts, whether
  or not synthesis has finished — and empty before and after. Every provider
  implements it.

## [5.2.0] - 2026-10-06

### Changed

- The ROS4HRI mapping shows a named expression at the commanded arousal as
  its intensity, clamped to [0, 1], where it showed it at full weight. A
  command naming an expression with an arousal of 0 or below (calm, or left
  unset) now shows no expression. With no name, valence and arousal still
  blend the expressions on the circumplex.

## [5.1.0] - 2026-10-04

### Added

- The `vizij-face` profile declares the blink, two more expressions and the
  conversation state, 88 keys in all:
  - `standard/vizij/blink` (`standard::BLINK`), tier `gaze`: a weight in
    [0, 1], 0 open, 1 both eyes closed, resting at 0. A level the writer
    shapes over time, independent of the eyelid positions; a face closes
    each lid at least as far as the stronger of the two.
  - `standard/vizij/expression/concerned` and `…/sleepy`, tier `expression`:
    `standard::VIZIJ_EXPRESSION_NAMES`, the expressions beyond ROS4HRI's
    vocabulary. `standard::expression_names()` is the standard's whole set,
    `standard::ROS4HRI_EXPRESSION_NAMES` then these.
  - `standard/vizij/conversation/{speaking,user_speaking,thinking}`
    (`standard::CONVERSATION_STATES`, `standard::conversation_path`), tier
    `conversation`: weights written 0 or 1 by the agent the face speaks
    for, resting at 0.
- `standard::ROS4HRI_EXPRESSION_NAMES`: the 25 names of
  `hri_msgs/Expression`, which the ROS4HRI mapping commands.

### Deprecated

- `standard::EXPRESSION_NAMES`: ROS4HRI's subset of the standard's
  expressions. Use `ROS4HRI_EXPRESSION_NAMES`, or `expression_names()` for
  the whole set.

The ROS4HRI mapping is unchanged: it writes none of the new keys.

## [5.0.0] - 2026-10-04

### Changed

- **Breaking:** the bundle's authored animations are `contents::Animation`
  and `contents::AnimationTrack`, held in `Bundle::animations` (serialized
  `animations`) — named as the bundle and Semio Studio name them. They
  replace `contents::Clip`, `contents::ClipTrack` and `Bundle::clips`;
  `Keyframe` keeps its name.

### Added

- `contents::animation` reads one animation: a bundle `animations` entry, or
  an animation in the shape `Animation` serializes to — what a client hands a
  device to load at run time.
- `Bundle::channel_keys` resolves an animation track's channel to the store
  key it drives on the face (`ChannelKeys::key`): the rig input at
  `<rig prefix><channel>`, a rig input path as is, an input by the name the
  rig's node gives it, else the prefixed path.

## [4.1.0] - 2026-10-02

### Added

- `Bundle` reads what an app builds its controls from, typed in the new
  `contents` module: `poses` (`Pose`: id, name, description, the ids of the
  groups it belongs to — its `groupIds`, its `groupId`, and the group its
  `group` path names — and its input values), `pose_groups` (`PoseGroup`),
  `rig_inputs` (`RigInput`: the rig graph's `metadata.vizij.inputs`, each
  path relative to the rig prefix, with its label, group, default and range),
  `clips` (`Clip`, `ClipTrack`, `Keyframe`: the authored `animations`,
  keyframes in time order), `program_labels` (program id → its graph entry's
  `label`), and `metadata`, the bundle's open-ended `metadata` as authored.
  The `contents` types serialize in camelCase.
- A `Bundle` built with a struct literal that names every field must name the
  new ones, or end in `..Default::default()`.

## [4.0.0] - 2026-09-28

### Changed

- **Breaking:** each skill's contract is a trait declared with arora-module's
  `#[contract]`: `LookAt`, `PlayViseme` and `Say`, each beside a module
  holding its ids (`look_at::ids::look_at::FUNCTION`, …), `NAME`,
  `record(parent)` and `exports(implementation)`. They replace `SAY_ID`,
  `SAY_TEXT_PARAM_ID`, `SAY_VOICE_PARAM_ID`, `SAY_VISEME_PARAM_ID`,
  `LOOK_AT_FUNCTION`, `PLAY_VISEME_FUNCTION` and `SAY_FUNCTION`. The ids on
  the wire are unchanged: the parameter ids of look_at and play_viseme,
  hashed from their names, are literals of the same values.
- **Breaking:** `say`'s `voice` is optional (`Option<String>`); a call
  without one is spoken in the provider's default voice.
- The say fragment passes `sil` as the provider's `viseme`, stated in its
  task-run node's `value` (vizij-graph-core 2.1): a provider declared from
  the contract fails a call that lacks a required argument.
- Depends on arora-module 2.1.

## [3.0.0] - 2026-09-26

### Changed

- **Breaking:** depends on arora-types 3, arora-behavior 9 and vizij-api-core 2;
  the optional ROS 2 frames on arora-msgs-ros2 2.

## [2.1.0] - 2026-09-10

### Added

- `frames` (feature `publish-frames`): the rendered face as a ROS image value.
  `raw_frame` builds a `sensor_msgs/Image`, `compressed_frame` a
  `sensor_msgs/CompressedImage` (`PNG_FORMAT` names the PNG buffer the way the
  message requires), each seeded as the registry's own typed value under the
  store key of its `image_transport` transport — `FrameFormat::key`:
  `display/face`, `display/face/compressed` — so the ROS 2 bridge's ROS4HRI
  profile publishes it on `/robot_face/image_raw[/compressed]` with no
  conversion. The pixel payload bypasses per-element seeding: one copy of the
  pixels, not a `Value` per byte.
- The `play_viseme` and `say` skills: their generated fragments and shipped
  assets (`skills/play_viseme.json`, `skills/say.json`), the `SAY_*` contract
  ids, the envelope timing (`VISEME_ATTACK`, `VISEME_HOLD`, `VISEME_RELEASE`),
  the feedback field names (`FEEDBACK_VISEME`, `FEEDBACK_INTENSITY`), and
  `prefix_controls`, which addresses a fragment's standard paths to a face's
  rig prefix. `SKILLS` lists three. The fragments run on `vizij-graph-core`
  1.4: the say fragment wires the provider's viseme out-parameter from the
  task run's keyed `mutated` output.
- `standard::VISEME` (`standard/vizij/viseme`), the face's current-viseme
  state key, written by the players. `standard::MOUTH_JAW_OPEN`
  (`standard/vizij/mouth/morph/jaw_open`), the jaw path every face implements
  and the ROS4HRI mapping has always written, is a declared muscle-tier key:
  `vizij-face` is 82 keys, 36 in the muscle tier.

### Changed

- Visemes leave the ROS4HRI mapping: `mappings/ros4hri.json` no longer writes
  `standard/vizij/viseme/<shape>`. ROS4HRI has no viseme channel, and a viseme
  is a played thing, not a level — the players own the lips.
- The ROS4HRI mapping is generated by channel with every node named (`in/…`,
  `out/…`, `expression/<name>/…`, `gaze/<eye>/…`, `au/<code>/…`, `blink/…`)
  and one shared node per scalar constant: the same function on 428 nodes
  instead of 674, readable in review and in the authoring app. The `look_at`
  fragment regenerates the same way.

## [2.0.0] - 2026-09-09

### Breaking

- Profiles and mappings are two things, named apart. A **profile** is an
  interface — the typed store paths one party exposes to another — in the
  `profile` module: `Profile`, `ProfileKey`, `KeyMeta`, `Scope`,
  `vizij_face_profile()`, `ros4hri_profile()`, `PROFILES`, `shipped(id)`,
  `profile(id)`, `profiles_json()`, `surface`. A **mapping** is an operation
  — a graph that implements one profile in terms of another — in the
  `mappings` module: `StandardMapping`, `STANDARD_MAPPINGS`,
  `standard_mapping`, `standard_mapping_source`, `standard_mappings_json`,
  `MAPPING_JSON`, `STANDARD_MAPPING_KIND`. The `profiles` module,
  `StandardProfile`, `STANDARD_PROFILES`, `standard_profile*`,
  `STANDARD_PROFILE_KIND` and `PROFILE_JSON` are gone;
  `Bundle::standard_profiles` is `Bundle::standard_mappings`, and `compose`
  takes mappings. `STANDARD_MAPPING_KIND` keeps the on-disk value
  `standard-profile`, which faces in the wild carry.

### Added

- Profiles as data: `profiles/vizij-face.json` (81 keys) and
  `profiles/ros4hri.json` (40 keys), each key in the bridge's `KeyInfo` shape
  — an arora `Type`, a default `Value`, a range — plus `meta` (FACS action
  unit, ARKit blendshape, tier), generated from the constants and held to them
  by a drift test. A profile carries its `Scope`: `vizij-face` is face-scoped
  (`Profile::with_rig_prefix` addresses it under `rig/<faceId>/`), `ros4hri`
  device-scoped and absolute, as a bridge writes it.
- A face declares the profiles it speaks: `Bundle::profiles` reads the GLB's
  top-level `profiles` array, skipping a malformed entry rather than failing
  the face.
- `profile::surface(spec, side, id, scope)` lifts either side of a mapping
  graph as a profile — its `input` nodes are what it consumes, its `output`
  nodes what it produces — so a mapping is reconciled against the profiles it
  claims. A lifted surface can be a strict subset of the profile: compare, do
  not replace.

## [1.0.0] - 2026-07-31

First published release: the spec and data transforms above a Vizij device,
shared by the native app and the browser runtime — `Bundle` (the face's
`VIZIJ_bundle`: graphs, programs, the program to autoplay, the neutral-pose
config), `compose_sources` and `Bundle::compose`, `ProgramSelect`,
`Bundle::neutral_stage_writes`, the face standard's paths and tiers in
`standard`, the ROS4HRI mapping generator and asset in `ros4hri`, and the
`look_at` skill fragment in `skills`.
