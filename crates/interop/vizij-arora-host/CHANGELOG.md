# Changelog

All notable changes to `vizij-arora-host`. The format follows
[Keep a Changelog](https://keepachangelog.com/); versions follow
[Semantic Versioning](https://semver.org/).

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
