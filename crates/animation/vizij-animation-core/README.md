# vizij-animation-core

Engine-agnostic animation core for parsing, sampling, and baking animations. This crate defines the data model, transition semantics, sampling/baking pipeline, and a small runtime engine. It is consumed by thin adapters (Bevy plugin and WASM wrapper) in sibling crates.

Key features
- StoredAnimation parser: parse the standard JSON format (per-keypoint transitions with in/out control points).
- Single transition model: Cubic Bezier timing function per segment, with defaults.
- Broad value support: Scalar, Vec2/3/4, Quat, Color (RGB/HSL), Transform (TRS), plus step-only Bool and Text.
- Deterministic sampling and a simple baking API.
- Registry abstraction retained for future extensibility, but sampling is cubic-bezier-only by design.

## Data model

The core data types (see `src/data.rs` and `src/value.rs`):

- AnimationData
  - `name: String`
  - `tracks: Vec<Track>`
  - `duration: f32` in seconds
  - `meta: serde_json::Map` for optional metadata

- Track
  - `target_path: String` (canonical key describing the output target, e.g. `node/Transform.translation`)
  - `value_kind: ValueKind`
  - `keys: Vec<Keyframe>`
  - `default_interp: InterpKind` (the fallback interpolation if a segment doesn’t specify one)

- Keyframe
  - `t: f32` in seconds (monotonic)
  - `value: Value`
  - `interp: Option<InterpKind>` (applies to the segment [this_key .. next_key])

- Value and ValueKind
  - Numeric kinds: `Scalar`, `Vec2`, `Vec3`, `Vec4`
  - Rotations: `Quat` (nlerp + normalize)
  - Colors: `Color([r,g,b,a])` in 0..1
  - Transforms: `Transform { translation: [f32; 3], rotation: [f32; 4], scale: [f32; 3] }`
  - Step-only kinds: `Bool`, `Text`

## Transition model

- The engine uses a single transition model: Cubic Bezier timing function per segment.
- For segment [P0 → P1], eased `t` is computed from a timing function `cubic-bezier(x1,y1,x2,y2)` where endpoints are (0,0) and (1,1).
- The (x1,y1,x2,y2) control points are derived from per-keypoint transitions:
  - `cp0 = P0.transitions.out` or default `{x: 0.42, y: 0}`
  - `cp1 = P1.transitions.in` or default `{x: 0.58, y: 1}`
- Special cases:
  - Linear is normalized to Bezier(0,0,1,1)
  - A legacy "Cubic" ease is normalized to Bezier(0.42,0,0.58,1) (ease-in-out)
  - Bool/Text always use Step semantics (hold previous value across the segment; at exact endpoint move to next)

Interpolation behavior
- Numeric-like values (Scalar/Vec2/Vec3/Vec4/Color components/Transform TRS) are linearly interpolated component-wise using the eased `t`.
- Quaternion rotation uses shortest-arc NLERP with normalization.
- Transform rotates by NLERP, translates/scales linearly.

## Parsing the StoredAnimation format

This crate includes a tolerant parser for the StoredAnimation schema (see `src/stored_animation.rs` and the fixture `tests/fixtures/new_format.json`).

- Duration is in milliseconds at the root; internally converted to seconds.
- Keypoint `stamp` is normalized [0..1]; internally scaled to absolute time by duration.
- Per-keypoint transitions: `transitions.in` and `transitions.out` define control points; missing parts are defaulted.
- Values are untagged unions:
  - Number → Scalar
  - `{x,y}` → Vec2
  - `{x,y,z}` → Vec3 (Euler r/p/y mapped to Vec3)
  - `{r,g,b}` → Color RGB
  - `{h,s,l}` → Color via HSL→RGB conversion
  - Boolean → Bool (step)
  - String → Text (step)

Example usage:

```rust
use vizij_animation_core::parse_stored_animation_json;
use vizij_animation_core::{Engine, InstanceCfg};

let json_str = std::fs::read_to_string("tests/fixtures/new_format.json")?;
let anim = parse_stored_animation_json(&json_str).expect("parse");
let mut engine = Engine::new(vizij_animation_core::Config::default());
let aid = engine.load_animation(anim);
let pid = engine.create_player("demo");
let _iid = engine.add_instance(pid, aid, InstanceCfg::default());
// advance time, pull outputs
let _out = engine.update(1.0/60.0, Default::default());
```

## Sampling and baking

- Sampling util: `sampling::sample_track(&Track, t: f32) -> Value`
- Engine sampling: Instances are advanced in local clip time; per-Track values are sampled and accumulated per-target (with kind safety).
- Baking API: `baking::bake_animation_data` samples tracks at a fixed frame rate over a window and returns a baked struct; `export_baked_json` converts to serde JSON.

```rust
use vizij_animation_core::baking::{bake_animation_data, BakingConfig, export_baked_json};
use vizij_animation_core::{AnimId, AnimationData};

let cfg = BakingConfig { frame_rate: 60.0, start_time: 0.0, end_time: None };
let baked = bake_animation_data(AnimId(0), &animation_data, &cfg);
let json = export_baked_json(&baked);
```

## Accumulation and blending

- Contributions are accumulated per target and value kind.
- Numeric kinds are blended by weighted average; quaternions are normalized after NLERP blending.
- Step-only kinds (Bool/Text) prefer the last assignment (no blending).

## Interpolation registry

- `InterpRegistry` is retained for future extensibility, but sampling normalizes all transitions to cubic-bezier (or step).
- Legacy dead code has been removed; `cubic_value` has been pruned. Linear is handled as Bezier(0,0,1,1).

## Edge cases and policies

- Tracks with 0 keys: sampler returns a neutral Scalar(0.0) (fail-soft).
- Tracks with 1 key: sampler always returns that key’s value.
- Time outside key ranges holds the ends.
- Mixed value kinds on the same target are ignored (fail-soft, no panic).
- Bool/Text have no interpolation; they step to the left key’s value for t in [key_i .. key_{i+1}).

## Testing

- Native tests:
  - `cargo test -p vizij-animation-core`
- WASM (Node-based):
  - Run `scripts/run-wasm-tests.sh` at the workspace root (executes tests from the `vizij-animation-wasm` crate)
- Fixtures:
  - `tests/fixtures/new_format.json` demonstrates the StoredAnimation schema and Bezier control points.
  - Additional fixtures (const, ramp, cubic, etc.) are compatible with the simplified bezier model.

## Example JSON (StoredAnimation snippet)

```json
{
  "id": "anim-const-vec3",
  "name": "ConstVec3",
  "tracks": [
    {
      "id": "t0",
      "name": "Translation",
      "animatableId": "node/Transform.translation",
      "points": [
        { "id": "k0", "stamp": 0.0, "value": { "x": 1, "y": 2, "z": 3 } },
        { "id": "k1", "stamp": 1.0, "value": { "x": 1, "y": 2, "z": 3 } }
      ],
      "settings": { "color": "#ffffff" }
    }
  ],
  "groups": {},
  "transitions": {},
  "duration": 1000
}
```

## License

See the workspace root for licensing details.
