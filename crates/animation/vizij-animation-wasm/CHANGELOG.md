# Changelog

All notable changes to `vizij-animation-wasm`. The format follows
[Keep a Changelog](https://keepachangelog.com/); versions follow
[Semantic Versioning](https://semver.org/).

## [Unreleased]

### Breaking

- Built on `vizij-animation-core` 5: a number track's values come out in
  the width of its keypoints. Keypoints given as `{"f64": …}` output
  `{"f64": …}` from updates and bakes, derivatives included, where they
  output `{"f32": …}`. A track with any `{"f64": …}` keypoint outputs
  `{"f64": …}` throughout; a key an `f32` and an `f64` track blend into
  outputs `{"f64": …}` on the steps the `f64` track weighs on it.

### Changed

- Built on `vizij-animation-core` 5: the changes of an update come in a
  stable order, per player and then in the order its instances first write
  each key.

## [3.0.0] - 2026-10-06

### Breaking

- Built on `vizij-animation-core` 3: the play window bounds `Loop` and
  `PingPong` as it bounds `Once`, a player's length ignores the window,
  players report `ended`, and `PlayAfter` starts playback a delay into an
  update.

### Changed

- Built on `vizij-animation-core` 4: `bake_animation` and
  `bake_animation_with_derivatives` throw for a bake of more than 2²⁰
  samples over all tracks (derivatives counted) instead of allocating it,
  and for an animation not loaded with the core's message ("no animation is
  loaded under id N"); a `start_time` past the clip's end bakes its end.

## [2.0.0] - 2026-10-05

### Breaking

- Built on vizij-animation-core 2.0.0: a player's playback state is
  independent of its speed. `Pause` and `Stop` keep the speed and `Play`
  resumes at it; `SetSpeed` only sets the multiplier (it neither resumes a
  paused player nor pauses a playing one at 0). `list_players` reports
  `state` as the last `Play`, `Pause` or `Stop` left it and `speed` as set;
  a `Seek` leaves a stopped player `Paused`.

### Changed

- Freshened workspace dependencies to current majors.

## [1.0.0] - 2026-07-10

### Breaking

- Built on vizij-animation-core 1.0.0 / vizij-api-core 1.0.0: values the engine
  emits (frame changes, baked tracks) are now in arora serde form. Read them
  through the `@vizij/value-json` accessors; code that pattern-matched the raw
  JSON shape must switch to the accessors. Legacy input forms are normalized on
  ingress.

### Changed

- Standardized `Value` usage and casing; fixtures and value-json packages
  refactored.

### Fixed

- Accumulation and pre-binding bugs surfaced through the wasm bridge.

## [0.3.0] - 2025-09-28

### Added

- Baked animation exposed through the wasm/npm API; derivative update API.

## [0.2.0] - 2025-09-23

### Added

- Full animation-player port; multi-slider node.

### Changed

- Extracted the shared API (vizij-api-core); refactor around vector data; crate
  metadata prepared for publishing.

### Fixed

- Offset and scale calculations on instances.

## [0.1.0] - 2025-09-05

### Added

- Initial release: wasm-bindgen interface for the Vizij animation engine.
