# Changelog

## 3.0.0

### Major Changes

- e459227: The module (2.0.0) returns a bake as typed records of Arora values instead of a JSON string. `bake(anim, frame_rate?, start_ns?, end_ns?)` and `bake_with_derivatives(…)` return `BakedAnimation? { frame_rate, start_ns, end_ns, tracks: [BakedTrack { animatable_id, values, derivatives }] }`: `values` is a `Value::ArrayValue` with each frame's sample as a dynamic value, as `Keypoint::value` is, and `derivatives` one `Value::Option` per frame from `bake_with_derivatives`, empty from `bake`. An animation not loaded, or a bake beyond 2²⁰ samples, returns none, where it returned an empty string. The window arguments are nanoseconds of the clip's time, like the module's other times, where they were seconds: they keep their parameter ids, so a caller still sending seconds as `f32` is refused, naming the parameter, rather than misread (an integer it sends is read as nanoseconds). The window reads back as requested, clamped into the clip.

### Minor Changes

- 8ec8636: The module (2.0.0) steps returning values by position, with the keys sent only when they change. `output_keys() -> OutputKeys { revision, keys }` gives the key of each position: per player, in creation order, each key its instances write, in the order they first write it. Only a structural edit changes it (`add_instance`, `add_instance_with_weight`, `remove_instance`, `remove_player`, `unload_animation`, `reload_animation`), each that does giving a new `revision`. `step_values(dt_ns, time_ns?) -> StepValues { revision, values }` is `step` returning the values alone, a `Value::ArrayValue` as long as the table, `Value::Unit` at a position no instance weighs on this step; a `revision` other than the one the keys were read at says to read them again. `step`'s outputs come in the same order, where they came in an order that varied from step to step.

### Patch Changes

- 0faddff: The module (1.1.2) refuses a bake it cannot hold with a value. `bake` and `bake_with_derivatives` return no result, as for an animation not loaded, when the window at the frame rate would take more than 2²⁰ samples over all tracks (derivative samples included); such a bake used to trap the guest. A window starting past the clip's end bakes the clip's end. `bake` samples the values alone, without computing derivatives it does not return.
- a1f0cfa: The module (1.1.1) reaches its state without a lock, so the calls after a trap are answered, where a lock the trapped call held stayed held and failed every later call of the instance. A trap still leaves the instance unsound to keep — what the trapped call had changed stays changed, and its stack frames and argument buffer are not reclaimed — so a host that sees a trap should retire the instance. `set_speed` rejects a speed that is not finite, and `set_weight` and `add_instance_with_weight` a weight that is not finite and non-negative, with `u32::MAX`: a NaN or infinite value used to reach the engine and leave the playhead or the blend at NaN, and a negative weight, which the engine ignored, is now refused.

## 2.1.0

### Minor Changes

- 346c6fd: The module (1.1.0) exports what a timeline transport needs. `set_window(player, start_ns, end_ns?)` sets a player's play window: `once` clamps into it and holds at the bound it reaches, `loop` wraps within it, `ping_pong` reflects within it, and `stop` returns to its start; without `end_ns` it ends at the player's length. `set_speed` takes a negative speed, which plays backwards through the window. `set_start_offset(player, instance, offset_ns)` and `set_time_scale(player, instance, time_scale)` set where an instance starts on its player's timeline and how its clip stretches there: its local time is (playhead − offset) / `time_scale`, and a time scale that is not finite and positive is rejected with `u32::MAX`. `play_at(player, time_ns)` starts a player at `time_ns` in the time base `step` is given, wherever that instant falls between steps, so players on devices sharing a clock reference start in lockstep; `step(dt_ns, time_ns?)` takes the device's `arora/time` as `time_ns`, and without it counts the steps' `dt_ns` from 0. `PlayerState` (record 1.2.0) adds `ended`, set while a `once` player holds at the window bound it plays toward, `window_start_ns` and `window_end_ns`; `InstanceState` (record 1.1.0) adds `start_offset_ns` and `time_scale`; `duration_ns` is the player's length, which a window does not shorten.

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
