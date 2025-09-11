# vizij-animation-wasm

WASM wrapper for `vizij-animation-core`, exporting a minimal, ergonomic JS/TS API via `wasm-bindgen`. This crate is intended for web and Node runtimes and provides JSON-centric interfaces for configuration, loading animations, prebinding, and stepping the simulation.

Key features
- Simple JS API around the core engine (create, load, add instance, prebind, update).
- Accepts both the core `AnimationData` JSON and the new StoredAnimation JSON format.
- Outputs are returned as JSON with tagged values compatible with TypeScript.
- Parity-tested against the native core for representative scenarios.

## Installation

This crate is built as part of the monorepo. You can consume the generated WASM package or run tests locally:

- Node-based tests: `scripts/run-wasm-tests.sh` from the workspace root.
- Browser-based tests are supported (see `wasm_bindgen_test_configure!(run_in_browser)` in tests), but the provided runner defaults to Node.

## API

The module exports:

- `class VizijAnimation`
  - `constructor(config?: JsValue) -> VizijAnimation`
    - `config` maps to `vizij-animation-core::Config` (uses defaults if `undefined`/`null`)
  - `load_animation(data: JsValue) -> number`
    - Expects core `AnimationData` as JSON; returns `AnimId (u32)`
  - `load_stored_animation(data: JsValue) -> number`
    - Accepts the new StoredAnimation JSON (see below); returns `AnimId (u32)`
  - `create_player(name: string) -> number`
    - Creates a named player; returns `PlayerId (u32)`
  - `add_instance(playerId: number, animId: number, cfg?: JsValue) -> number`
    - Adds an animation instance with optional `InstanceCfg` JSON; returns `InstId (u32)`
  - `prebind(resolver: (path: string) => string | number | null | undefined): void`
    - Resolves canonical target paths to string handles; stores results internally
  - `update(dtSeconds: number, inputs?: JsValue) -> Outputs JSON`
    - Steps the simulation and returns a JSON payload:
      - `{ changes: Array<{player, key, value}>, events: [...] }`
      - `value` is a tagged union (`{ type: "Scalar"|"Vec3"|... , data: ... }`)

- `function abi_version() -> number`
  - Numeric ABI guard for consumers (currently `1`)

## StoredAnimation format (new)

This API understands the new JSON format via `load_stored_animation`, which matches `types/animation.ts` and the fixture `tests/fixtures/new_format.json`:

- Root:
  - `id: string`
  - `name: string`
  - `tracks: Track[]`
  - `groups: object`
  - `duration: number` (milliseconds)
- Track:
  - `id: string`
  - `name: string`
  - `animatableId: string` (canonical target path, e.g., `"node/Transform.translation"`)
  - `points: Keypoint[]`
  - `settings?: { color?: string }`
- Keypoint:
  - `id: string`
  - `stamp: number` in [0..1]
  - `value: number | {x,y}|{x,y,z}|{r,p,y}|{r,g,b}|{h,s,l}|boolean|string`
  - `transitions?: { in?: {x,y}, out?: {x,y} }`

Transition semantics:
- Per segment [P0→P1], the cubic-bezier timing function uses control points:
  - `cp0 = P0.transitions.out || {x: 0.42, y: 0}`
  - `cp1 = P1.transitions.in  || {x: 0.58, y: 1}`
- Eased `t` drives linear blending across supported value kinds.
- Bool/Text are step-only (hold left value until the next key).

## Usage example (Node)

```ts
import { VizijAnimation, abi_version } from "@vizij/animation-wasm";

console.log(abi_version()); // 1

const eng = new VizijAnimation(undefined);

// StoredAnimation example object (can also be read from a JSON file)
const animObj = {
  id: "anim-const",
  name: "Const",
  tracks: [{
    id: "t0",
    name: "Position",
    animatableId: "node/Transform.translation",
    points: [
      { id: "k0", stamp: 0.0, value: { x: 1, y: 2, z: 3 } },
      { id: "k1", stamp: 1.0, value: { x: 1, y: 2, z: 3 } },
    ],
    settings: { color: "#fff" }
  }],
  groups: {},
  transitions: {},
  duration: 1000
};

const animId = eng.load_stored_animation(animObj);
const playerId = eng.create_player("demo");
const instId = eng.add_instance(playerId, animId, undefined);

// Optional: prebind canonical paths to string handles
eng.prebind((path: string) => path); // identity

// Step and get outputs
const out0 = eng.update(0.0, undefined);
console.log(out0.changes);

// Advance ~16ms
const out1 = eng.update(0.016, undefined);
```

## Resolver semantics (prebind)

`prebind(resolver)` is called with canonical target paths (e.g., `"node/Transform.translation"`). Return a string or number to use as the output key; return `null`/`undefined` if unresolved.

Example:
```ts
eng.prebind((path) => {
  const map: Record<string, string> = {
    "node/Transform.translation": "ENTITY_42/T.translation",
  };
  return map[path] ?? null;
});
```

## Testing

- Run Node-based tests from the workspace root:
  ```bash
  scripts/run-wasm-tests.sh
  ```
- Browser-only tests (e.g., Bool/Text value round-trips) are present but skipped by the Node runner.

## Notes

- This crate is intentionally thin; all transition logic and parsing are delegated to `vizij-animation-core`.
- The cubic-bezier timing function is used per segment with default control points.
- Quaternion interpolation uses shortest-arc NLERP with normalization.

## License

See the workspace root for licensing details.
