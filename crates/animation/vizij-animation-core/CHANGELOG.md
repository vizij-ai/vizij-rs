# Changelog

All notable changes to `vizij-animation-core`. The format follows
[Keep a Changelog](https://keepachangelog.com/); versions follow
[Semantic Versioning](https://semver.org/).

## [5.0.0] - 2026-10-08

### Breaking

- A scalar keeps the width of its keypoints. `TrackValue` gains
  `Float64(f64)`: a `Value::F64` keypoint decodes to it, where it decoded to
  `Float(f32)`, and it is interpolated, blended, differentiated and baked in
  `f64`, encoding as `Value::F64`. A scalar track with any `Float64`
  keypoint samples as `Float64` at every time, its `Float` keypoints
  widened; a key a `Float64` blends into on a step blends in `f64` on that
  step. A `Value::F32` keypoint still decodes to `Float` and outputs
  `Value::F32`.

- `AccumulatorWithDerivatives` accumulates by output index, not by key:
  `reset(outputs)`, `add(output, …)` and `take(output)` replace the
  key-taking `add` and the `HashMap`-returning `finalize`, and its buffers
  are reused across frames.
- `Instance` gains a private field, so it can no longer be built outside
  the crate (the engine builds it in `add_instance`).
- `Outputs::changes` come in output order — per player, in creation order,
  each key in the order its instances first write it — where they came in
  hash order, varying from step to step.

### Added

- `Engine::output_targets()`: the engine's outputs, one per key a player's
  instances write (`OutputTarget { player, key }`), laid out by structural
  edits (`add_instance`, `remove_instance`, `remove_player`,
  `unload_animation`, `replace_animation`, `prebind`);
  `Engine::output_revision()` changes whenever the targets do.
- `Engine::update_by_target(dt, inputs)`: steps and returns each output's
  value by index (`None` where no instance weighs on it), building and
  copying no key.

## [4.0.0] - 2026-10-06

### Breaking

- `bake_animation_data`, `bake_animation_data_with_derivatives`,
  `Engine::bake_animation` and `Engine::bake_animation_with_derivatives`
  return `Result<_, BakeError>`. The engine's bakes refuse an animation not
  loaded with `BakeError::NotLoaded`, where they returned `None`.

### Added

- `MAX_BAKE_SAMPLES` (2²⁰): a bake whose window at its frame rate would take
  more samples over all tracks — derivative samples counted — is refused
  with `BakeError::TooManySamples` before anything is allocated, so any rate
  a caller passes yields a result or a refusal. The samples are reserved
  fallibly (`BakeError::OutOfMemory`).

### Fixed

- A `start_time` past the clip's end bakes the clip's end, where it
  panicked.
- `bake_animation_data` samples the values alone; it computed every
  derivative and dropped it.

## [3.0.0] - 2026-10-06

### Breaking

- The play window bounds the playhead in every loop mode: `Loop` wraps within
  `[start_time, end_time]` and `PingPong` reflects within it, as `Once` clamps
  into it. `end_time: None` ends the window at the player's length. A player
  with no window set plays as before.
- A player's length (`PlayerInfo::length`, `Engine::player_total_duration`)
  is the latest end over its instances; a window does not shorten it.
- `PlayerCommand` gains `PlayAfter`, `Player` gains `starts_in` and
  `PlayerInfo` gains `ended` (below): code matching `PlayerCommand`
  exhaustively or building `PlayerInfo` literals must add them.

### Added

- `PlayerCommand::PlayAfter { player, delay }`: playback starts `delay`
  seconds into the update that applies it — a negative `delay` that long
  before — and the playhead holds until then, so it depends only on the time
  since that instant, wherever update boundaries fall. The player keeps its
  state until the start, then plays at its speed. `Play`, `Pause` and `Stop`
  cancel the wait; `Player::starts_in` holds what remains of it.
- `PlayerInfo::ended`: a playing `Once` player advancing at a non-zero speed
  has reached the window bound it plays toward (the start when playing
  backwards) and holds there.

### Fixed

- `Once` keeps the player time inside the window, so a player reversed after
  reaching the end moves back at once.
- An instance with a negative `start_offset` starts partway into its clip at
  playhead 0.

## [2.0.0] - 2026-10-05

### Breaking

- A player's playback state is explicit and independent of its speed:
  `Player::state` (a `PlaybackState`) is what `Play`, `Pause` and `Stop` set,
  and `speed` is what `SetSpeed` alone sets. Only a playing player's time
  advances, by `dt * speed`. `Pause` and `Stop` keep the speed, so `Play`
  resumes at the speed the player was given; `SetSpeed` does not start,
  pause or stop a player (a paused player stays paused, a playing one at
  speed 0 holds its time while it reads as playing). `Seek` leaves a stopped
  player paused at the time it seeks to; `SetWindow` keeps a stopped player
  at its window start. `PlayerInfo::state` reports the state as the last
  `Play`, `Pause` or `Stop` left it, and `PlayerInfo::speed` the multiplier
  as set. A new player plays at speed 1.
- `Player` is `#[non_exhaustive]`.

### Added

- `Engine::replace_animation(anim, data) -> bool` replaces a loaded
  animation's data in place, under the same id: its instances stay on their
  players with their configuration and sample every track of the new data,
  the players keep their playback, and their lengths follow the new
  duration. The animation's host bindings are dropped (a binding names a
  track by index); its tracks write their canonical paths until the next
  `prebind`.
- `BindingTable::remove_animation(anim)` removes every row bound to a
  channel of `anim`.
- `PlaybackState` is re-exported at the crate root.

### Changed

- Freshened workspace dependencies to current majors.

## [1.0.0] - 2026-07-10

### Breaking

- Built on the unified Value: `vizij_api_core::Value` is now
  `arora_types::value::Value` (vizij-api-core 1.0.0). Keypoints decode once at
  ingestion into POD `TrackValue` (`[f32; N]`); sampling, interpolation, and
  accumulation run on the PODs (same math, byte-identical blending), with the
  dynamic `Value` appearing only at boundaries. Values the engine emits (frame
  changes, baked tracks) are in arora serde form; consumers must read them
  through the arora accessors instead of pattern-matching the old JSON shape.

### Fixed

- Accumulation and pre-binding bugs in layered playback.

### Changed

- Standardized `Value` usage and type casing across the engine; test fixtures
  reorganized around the shared value-json helpers.

## [0.3.0] - 2025-09-28

### Added

- Derivative update and baking APIs: per-frame derivative calculations and
  baked-track output exposed through the core (and surfaced in the wasm/npm
  layers).

## [0.2.0] - 2025-09-23

### Added

- Full animation-player port; multi-slider node.

### Changed

- Extracted the shared types and interfaces into a common API
  (vizij-api-core); refactor around vector data; crate metadata prepared for
  publishing.

### Fixed

- Timing bugs; offset and scale calculations on instances.

## [0.1.0] - 2025-09-05

### Added

- Initial release: engine-agnostic core animation logic for Vizij.
