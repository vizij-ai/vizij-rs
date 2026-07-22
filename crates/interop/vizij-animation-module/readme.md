# vizij-animation-module

`vizij-animation-core` packaged as an **Arora wasm module** — the "build once,
run anywhere" showcase (VIZ-53). One typed module builds to `wasm32-wasip1` and
runs inside any Arora runtime (native, browser, Web Worker).

## What it is

The animation `Engine` lives in a **guest global** (`lazy_static`, like
`polly`): a wasm module's `Store`/`Memory` persist across `dispatch`, so the
engine's state survives between calls — no engine state round-trips through the
store.

The boundary types are declared in [`module.yaml`](module.yaml) + the type
records under [`types/`](types), and the arora-module-authoring `rust` generator
(0.2.0, ARORA-55) emits the typed `Struct <-> Value::Structure` glue into
`src/arora_generated/`.

### Declared schema (`module.yaml` types)

| type | shape |
| --- | --- |
| `AnimationClip` | `{ name: str, duration: u32, tracks: [AnimTrack] }` |
| `AnimTrack` | `{ id: str, name: str, animatable_id: str, points: [Keypoint] }` |
| `Keypoint` | `{ id: str, stamp: f32, value: <dynamic Value>, transitions_in: [TransitionHandle], transitions_out: [TransitionHandle] }` |
| `TransitionHandle` | `{ x: f32, y: f32 }` — a cubic-bezier timing handle in normalized segment space; a keypoint carries zero or one per side (empty = the engine's default ease) |
| `TrackOutput` | `{ track_id: str, default_key: str, value: <dynamic Value> }` |
| `PlayerState` | `{ player: u32, state: str, time_ns: u64, duration_ns: u64, speed: f32 }` — `state` is `"playing" \| "paused" \| "stopped"` |

A keyframe/output `value` is a **dynamic `Value`** (the `KEY_VALUE_ID` escape
hatch), so Vizij composites (`Vec3`/`Quat`/`Transform`/`ColorRgba`) ride through
as `Value::Structure` carrying **vizij-arora's Vizij-namespaced UUIDs** — no
per-composite type is declared here; the runtime `Value` carries the identity.

### Exports

- `load_animation(clip: AnimationClip) -> u32` — load a clip, return its `AnimId`.
- `create_player(name: str) -> u32` — return a `PlayerId`.
- `add_instance(player: u32, anim: u32) -> u32` — return an `InstId`.
- `step(dt_ns: u64) -> [TrackOutput]` — advance by the `arora/dt` golden-key
  nanoseconds and return **per-track outputs keyed by track identity**, each
  carrying the track's **default authored key** (`animatable_id`) plus its
  sampled value. The consumer decides the final store key: default = the
  authored key, overridable.
- Transport — `play(player)`, `pause(player)`, `stop(player)`,
  `seek(player, time_ns)`, `set_speed(player, speed)`,
  `set_loop(player, mode)` (`"once" | "loop" | "ping_pong"`), and
  `set_weight(player, instance, weight)` buffer into the engine's **next**
  `step`, in issue order — the same phase a device applies external calls in.
  `remove_instance(player, instance)` is a structural edit, applied
  immediately like `add_instance`.
- `player_states() -> [PlayerState]` — playback feedback, one entry per
  player. A **patch**: the vision is state changes as first-class,
  combinable values the behavior conveys, not a second feedback channel.

## Building & testing

```sh
# native logic test (guest global engine + per-track output contract):
cargo test -p vizij-animation-module --lib

# build the wasm artifact:
cargo build -p vizij-animation-module --target wasm32-wasip1
```

The host-side end-to-end test (`tests/host_ramp.rs`) loads the built `.wasm`
into a real Arora engine and proves the `arora_call` boundary. It is
`#[ignore]`d because building the artifact from inside the test deadlocks the
cargo build lock; pre-build it (the wasm command above), then run with
`cargo test -p vizij-animation-module --test host_ramp -- --ignored`.
