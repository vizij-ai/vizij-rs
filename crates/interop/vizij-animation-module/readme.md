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
| `Keypoint` | `{ id: str, stamp: f32, value: <dynamic Value> }` |
| `TrackOutput` | `{ track_id: str, default_key: str, value: <dynamic Value> }` |

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

  Player-command inputs (`Inputs`) are a future extension: a dynamic `Value`
  cannot be a function parameter directly (only a struct field), so they would
  arrive as a declared `AnimInputs` struct. Today the player plays by default.

## Building & testing

```sh
# native logic test (guest global engine + per-track output contract):
cargo test -p vizij-animation-module --lib

# build the wasm artifact:
cargo build -p vizij-animation-module --target wasm32-wasip1
```

The host-side end-to-end test (`tests/host_ramp.rs`) loads the built `.wasm`
into a real Arora engine. It is currently a **repro** for an arora-buffers 0.2.0
wire-format discrepancy for arrays of structures across the `arora_call`
boundary (guest codegen writes full self-describing elements; the engine's
generic `serde_uuid` codec writes raw elements). Reconciling the two is a buffer
wire-format change and is gated on a design discussion.
