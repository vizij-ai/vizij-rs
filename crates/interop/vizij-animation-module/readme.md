# vizij-animation-module

`vizij-animation-core` packaged as an **Arora wasm module** — the "build once,
run anywhere" showcase (VIZ-53). One typed module builds to `wasm32-wasip1` and
runs inside any Arora runtime (native, browser, Web Worker).

## What it is

The module's state — the animation `Engine`, the key-to-track index and the
transport commands buffered for the next step — is an `AnimationModule`. The
module has no notion of which engine loaded it: the state is scoped by the
load. The wasm guest owns one in a **guest global**, in the linear memory of
the `Store` the executor creates per `load_module` — each engine that loads
the module gets its own, persisting across `dispatch` calls (no engine state
round-trips through the data store); the declared functions act on that
global. A host that links this crate as an rlib
(`vizij`'s native and browser devices) has no executor to scope it, so it
builds one `AnimationModule` per `host_module()` — the host-linked
counterpart of a load — and two devices in one process never share an
engine.

The module's interface is declared in Rust with
[`arora-module`](https://crates.io/crates/arora-module): `animation` in
[`src/lib.rs`](src/lib.rs) pins the module, function and parameter ids, and
from it come the header a `module.yaml` is written from (`animation::header`),
the store record, typed client stubs, and the wasm guest's entry points. The
boundary types derive `AroraType` with the ids and versions of their records.
There is no `module.yaml`, `build.rs` or generated source in the crate.

### Declared schema

| type | shape |
| --- | --- |
| `AnimationClip` | `{ name: str, duration: u32, tracks: [AnimTrack] }` |
| `AnimTrack` | `{ id: str, name: str, animatable_id: str, points: [Keypoint] }` |
| `Keypoint` | `{ id: str, stamp: f32, value: <dynamic Value>, transitions_in: [TransitionHandle], transitions_out: [TransitionHandle] }` |
| `TransitionHandle` | `{ x: f32, y: f32 }` — a cubic-bezier timing handle in normalized segment space; a keypoint carries zero or one per side (empty = the engine's default ease) |
| `TrackOutput` | `{ track_id: str, default_key: str, value: <dynamic Value> }` |
| `PlayerState` (record `1.2.0`) | `{ player: u32, state: str, time_ns: u64, duration_ns: u64, speed: f32, name: str, instances: [InstanceState], loop_mode: str, ended: bool, window_start_ns: u64, window_end_ns: u64? }` — `state` is `"playing" \| "paused" \| "stopped"`, as the last `play`, `pause` or `stop` left it (a player waiting for its `play_at` start keeps the state it had, `"playing"` from its start on); `duration_ns` is the player's length, the latest end over its instances; `speed` is the multiplier as `set_speed` set it, negative backwards; `name` is the one `create_player` gave it (empty without one); `loop_mode` is `"once" \| "loop" \| "ping_pong"`; `ended` says a `once` player has reached the window bound it plays toward; the window end is absent when it ends at the player's length |
| `InstanceState` (record `1.1.0`) | `{ instance: u32, anim: u32, weight: f32, start_offset_ns: i64, time_scale: f32 }` — an instance on a player, the animation it plays, its blend weight, and its timing on the player timeline |

A keyframe/output `value` is a **dynamic `Value`** (the `KEY_VALUE_ID` escape
hatch), so Vizij composites (`Vec3`/`Quat`/`Transform`/`ColorRgba`) ride through
as `Value::Structure` carrying **vizij-arora's Vizij-namespaced UUIDs** — no
per-composite type is declared here; the runtime `Value` carries the identity.

### Exports

Loading and unloading are structural edits, applied immediately:

- `load_animation(clip: AnimationClip) -> u32` — load a clip, return its `AnimId`.
- `reload_animation(anim: u32, clip: AnimationClip) -> bool` — replace a loaded
  animation's tracks and duration under the same id. Every instance of it
  stays on its player with its weight and samples the new tracks from the
  next step, each output naming its new track; the players keep their state,
  playhead, speed and loop mode. `false`, changing nothing, when `anim` is not
  loaded.
- `create_player(name: Option<str>) -> u32` — return a `PlayerId`; the name
  defaults to empty. A player plays from its creation, at speed 1, looping.
- `add_instance(player: u32, anim: u32) -> u32` — attach an instance of
  `anim` to `player`, blending at weight 1, and return its `InstId`.
- `add_instance_with_weight(player: u32, anim: u32, weight: f32) -> u32` — the
  same, blending at `weight`. Added at 0, it writes nothing until `set_weight`
  gives it a weight: a client learns the instance's id only from the reply, so
  its own `set_weight` lands a step after the instance.
- `remove_instance(player: u32, instance: u32) -> u32` — 1 when the instance
  was on the player, 0 otherwise.
- `remove_player(player: u32) -> bool` — the player and its instances; the
  animations it played stay loaded.
- `unload_animation(anim: u32) -> bool` — the animation and every instance of
  it, on every player.

Per tick and transport:

- `step(dt_ns: u64, time_ns: u64?) -> [TrackOutput]` — advance by `dt_ns`,
  the `arora/dt` built-in key, to `time_ns`, the `arora/time` key (left out,
  the previous step's time plus `dt_ns`, from 0), and return **per-track
  outputs keyed by track identity**, each carrying the track's **default
  authored key** (`animatable_id`) plus its sampled value. The consumer
  decides the final store key: default = the authored key, overridable.
- Transport — buffered into the engine's **next** `step`, in issue order (the
  same phase a device applies external calls in); each returns the player id
  (`set_weight`, `set_start_offset` and `set_time_scale` the instance id), or
  `u32::MAX` for a value it rejects. `play`, `play_at`, `pause` and `stop`
  set whether a player's time advances; `set_speed` only sets the multiplier
  it advances at while playing. A player keeps its speed through `pause` and
  `stop`, so `play` resumes at it; `set_speed` neither resumes a paused player
  nor pauses a playing one; a `seek` leaves a stopped player paused where it
  seeks.
  - `play(player)` — start or resume, at the player's speed.
  - `play_at(player, time_ns)` — start at `time_ns` in `step`'s time base:
    the player holds until then and from then on stands where it would had it
    started at that instant, whichever steps the instant falls between, so
    devices sharing a clock reference start in lockstep. An instant already
    past starts it there, the next step catching up. A `set_speed` while it
    waits sets the speed it starts at; `play`, `pause` or `stop` cancel it.
  - `pause(player)`, `stop(player)` — hold, or return to the window start;
    both keep the speed.
  - `seek(player, time_ns)`.
  - `set_speed(player, speed)` — negative plays backwards, 0 holds.
  - `set_loop(player, mode)` — `"once" | "loop" | "ping_pong"`.
  - `set_window(player, start_ns, end_ns?)` — the play window: `once`
    clamps into it and holds at the bound it reaches, `loop` wraps within it,
    `ping_pong` reflects within it, `stop` returns to its start. Without
    `end_ns` it ends at the player's length. A playhead outside the new
    window moves to its nearest bound.
  - `set_weight(player, instance, weight)` — the blend weight.
  - `set_start_offset(player, instance, offset_ns: i64)` — where the
    instance starts on its player's timeline; before it, the instance holds
    its clip's start, and a negative offset starts it partway in.
  - `set_time_scale(player, instance, time_scale)` — how the instance's clip
    stretches: its local time is (playhead − offset) / `time_scale`, so 2
    plays it over twice its duration. Finite and positive, else rejected.
- `player_states() -> [PlayerState]` — one entry per player, in creation
  order: its name, its playback, whether a `once` player has ended, its loop
  mode, its window and its instances. A client that did not load an
  animation finds it here: Vizij names an animation's player after the
  animation's id. The behavior that steps the module conveys it as a state
  value each step — Vizij's animation source writes it to
  `vizij/animations/players` — so a clip's end is the `ended` value there,
  not an event. A **patch**: the vision is state changes as first-class,
  combinable values the behavior conveys, not a second feedback channel.
- `bake(anim, frame_rate?, start_time?, end_time?) -> str` and
  `bake_with_derivatives(…)` — the sampled clip as JSON; the optional window
  defaults to 60 Hz over the whole clip.

A missing required argument fails the call, naming the parameter. An optional
one may be left out, sent as `Value::Option(None)`, or sent present — wrapped in
`Value::Option` or bare.

## Building & testing

```sh
# native logic test (an AnimationModule per test + per-track output contract):
cargo test -p vizij-animation-module --lib

# build the wasm artifact:
cargo build -p vizij-animation-module --target wasm32-wasip1
```

The host-side end-to-end test (`tests/host_ramp.rs`) loads the built `.wasm`
into a real Arora engine under the declared header and drives it through the
declaration's client stubs, proving the `arora_call` boundary. It is
`#[ignore]`d because building the artifact from inside the test deadlocks the
cargo build lock; pre-build it (the wasm command above), then run with
`cargo test -p vizij-animation-module --test host_ramp -- --ignored`.
