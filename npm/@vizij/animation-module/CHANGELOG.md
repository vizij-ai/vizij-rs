# Changelog

All notable changes to `@vizij/animation-module`. The format follows
[Keep a Changelog](https://keepachangelog.com/); versions follow
[Semantic Versioning](https://semver.org/).

## [0.2.0] - 2026-07-22

### Added

- Transport functions on the module surface: `play` / `pause` / `stop` /
  `seek(time_ns)` / `set_speed` / `set_loop("once" | "loop" | "ping_pong")` /
  `set_weight`, buffered into the engine's next `step` in issue order, and
  `remove_instance`, applied immediately.
- `player_states() -> [PlayerState { player, state, time_ns, duration_ns,
  speed }]` playback feedback. A patch: the vision is state changes as
  first-class, combinable values the behavior conveys, not a second
  feedback channel.
- `Keypoint` carries its cubic-bezier timing handles: `transitions_in` /
  `transitions_out` (`[TransitionHandle { x, y }]`, zero or one each), so
  authored linear/step/cubic timing reaches the engine instead of the
  default ease.

### Changed

- The `Keypoint` structure has five required fields (record `1.1.0`):
  senders always include the two handle arrays, empty when a side has no
  authored handle. Clips in the 0.1.x three-field shape do not decode.

## [0.1.0] - 2026-07-16

### Added

- Initial release: the vizij animation engine packaged as an Arora wasm
  module, shipped as importable assets (`wasm32-wasip1` executable + Arora
  header JSON). `loadAnimationModule()` returns the `{ headerJson, wasmBytes }`
  pair `@vizij/arora-web-wasm`'s `startDevice` loads into the browser device;
  `headerUrl`/`wasmUrl` expose the raw assets.
