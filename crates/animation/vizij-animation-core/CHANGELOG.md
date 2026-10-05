# Changelog

All notable changes to `vizij-animation-core`. The format follows
[Keep a Changelog](https://keepachangelog.com/); versions follow
[Semantic Versioning](https://semver.org/).

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
