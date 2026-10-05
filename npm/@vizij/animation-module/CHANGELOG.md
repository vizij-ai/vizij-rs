# Changelog

## 2.0.0

### Major Changes

- 71026e1: The module (1.0.0) keeps a player's speed through `pause` and `stop`, and reloads an animation in place. `play`, `pause` and `stop` set whether a player's time advances; `set_speed` only sets the multiplier: `play` resumes at the speed the player was given, `set_speed` on a paused player leaves it paused, and `set_speed(player, 0)` holds a playing player's time while its state stays `"playing"`. `player_states` reports `state` as the last `play`, `pause` or `stop` left it and `speed` as set, so a paused player's `speed` is its multiplier, not 0; a `seek` leaves a stopped player `"paused"`. `reload_animation(anim, clip) -> bool` replaces a loaded animation's tracks and duration under the same id, immediately: every instance of it stays on its player with its weight and samples the new tracks from the next step, each output naming its new track, and the players keep their state, playhead, speed and loop mode; it returns `false`, changing nothing, for an animation not loaded.

### Minor Changes

- 9f81dfc: The module (1.0.0) adds an instance at a given weight and unloads through declared functions, and any client finds a player by name. `remove_player(player) -> bool` removes a player and its instances; `unload_animation(anim) -> bool` unloads an animation and every instance of it; both apply immediately, like `add_instance`. `add_instance_with_weight(player, anim, weight) -> u32` adds an instance blending at `weight`: added at 0, it writes nothing until `set_weight` gives it a weight, so a client loads an animation silent although it learns the instance's id only from the reply. `add_instance(player, anim)` is unchanged and adds an instance at weight 1. `PlayerState` (record 1.1.0) adds `name`, the name `create_player` gave the player, `instances`, each an `InstanceState { instance, anim, weight }`, and `loop_mode` (`once`, `loop` or `ping_pong`); a reader of the 1.0.0 record ignores the three fields.

All notable changes to `@vizij/animation-module`. The format follows
[Keep a Changelog](https://keepachangelog.com/); versions follow
[Semantic Versioning](https://semver.org/).

## [1.0.0] - 2026-09-27

### Changed

- **Breaking:** the module is declared in Rust (arora-module) and its header
  is written from that declaration. A call missing a required argument fails,
  naming the parameter, where it returned `u32::MAX`. `create_player`'s
  `name` and `bake`'s `frame_rate`, `start_time` and `end_time` are declared
  optional: they may be left out, sent as `Value::Option`, or sent bare.
- The header carries the module's version (0.2.0), author, license and
  description, and lists the functions in declared order.

## [0.3.0] - 2026-07-22

### Added

- Baking on the module surface: `bake(anim, frame_rate?, start_time?, end_time?)`
  and `bake_with_derivatives(...)`, returning the sampled result as a JSON string
  (`vizij-animation-core`'s `export_baked_json` / `export_baked_with_derivatives_json`
  shape). `frame_rate` defaults to 60 Hz, `start_time` to 0 s, and `end_time` to
  the clip duration; an unloaded `anim` returns an empty string. Brings the
  module to feature parity with the direct library for baking.

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
