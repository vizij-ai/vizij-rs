# @vizij/value-json

Shared Value JSON types and coercion helpers for every Vizij WebAssembly wrapper.
The package defines the canonical `ValueJSON` union used by the animation, node
graph, orchestrator, and render runtimes, plus a set of utilities for taking
loosely typed inputs and recovering strongly typed values.

## Why this package exists

- **Single source of truth** - Rust crates emit JSON payloads encoded as tagged
  unions (e.g. `{ "type": "vec3", "data": [x, y, z] }`). This package mirrors
  that shape in TypeScript so all npm bindings stay in sync.
- **Consistent casing** - Discriminants are always lowercase (`"float"`,
  `"colorrgba"`, etc.). Older helpers in the web repo hard-coded mixed-case
  variants; using these utilities avoids drift.
- **Ergonomic readers** - The exported `valueAs*` helpers convert either the
  normalized union or a legacy JSON shape into numbers, vectors, transforms,
  quaternions, booleans, colours, or text.
- **Input normalization** - `toValueJSON` turns primitive numbers/booleans/
  strings/arrays into the JSON union expected by the WASM bindings.

## Installation

```bash
pnpm add @vizij/value-json
# or npm install @vizij/value-json
```

> In the Vizij monorepo the package is linked from `vizij-rs/npm/@vizij/value-json`.

## Core types

| Type | Description |
| ---- | ----------- |
| `ValueJSON` | Union that accepts both legacy JSON objects (`{ float: 1 }`) and the normalized `{ type, data }` shape. Numbers, strings, booleans, and arrays are also accepted for convenience. |
| `NormalizedValue` | Canonical tagged union (`{ type: "vec3", data: [...] }`). All WASM bindings emit values in this format. |
| `ValueInput` | Values that can be coerced via `toValueJSON`: any `ValueJSON` or a raw numeric array. |
| `Transform` / `NormalizedTransform` | Typed structures for position/rotation/scale payloads. |

```ts
type NormalizedValue =
  | { type: "float"; data: number }
  | { type: "vec3"; data: [number, number, number] }
  | { type: "transform"; data: NormalizedTransform }
  | ...;
```

## Normalising inputs

`toValueJSON` wraps untyped values in the JSON union so that the WASM APIs can
consume them safely:

```ts
import { toValueJSON } from "@vizij/value-json";

toValueJSON(0.5);        // { float: 0.5 }
toValueJSON(true);       // { bool: true }
toValueJSON("label");    // { text: "label" }
toValueJSON([1, 2, 3]);  // { vector: [1, 2, 3] }

// Already-normalised values are passed through untouched:
toValueJSON({ type: "vec3", data: [0, 1, 2] });
```

This helper is used internally by `@vizij/node-graph-wasm`, `@vizij/orchestrator-wasm`,
and other wrappers before staging inputs or updating parameters.

## Detecting the canonical shape

```ts
import { isNormalizedValue } from "@vizij/value-json";

if (isNormalizedValue(value)) {
  // value has lowercase `.type` and `.data`
}
```

## Reading values

Each `valueAs*` helper works with both legacy and normalized payloads. They
return `undefined` when coercion is impossible.

```ts
import {
  valueAsNumber,
  valueAsNumericArray,
  valueAsVector,
  valueAsTransform,
  valueAsQuat,
  valueAsColorRgba,
  valueAsText,
  valueAsBool,
} from "@vizij/value-json";

valueAsNumber({ float: "1.25" }); // 1.25
valueAsVector({ type: "transform", data: transform }); // translation+rotation+scale flattened
valueAsTransform({ transform: { translation, rotation, scale } });
valueAsBool({ type: "vec3", data: [0, 0, 0] }); // false
valueAsColorRgba({ type: "float", data: 0.8 }); // [0.8, 0.8, 0.8, 1]
```

### Transform helpers

`valueAsTransform` and `valueAsVec3` tolerate missing components by filling in
defaults (`translation: [0, 0, 0]`, `rotation: [0, 0, 0, 1]`, `scale: [1, 1, 1]`)
so downstream render code can rely on well-formed tuples.

### Enums, arrays, and records

Enums are flattened recursively. Arrays, lists, and tuples are walked element by
element, so `valueAsVector` returns a concatenated numeric array. Records are
left to the caller because there is no stable ordering - iterate over the values
and call the appropriate helper per entry.

## Integration patterns

- `@vizij/animation-wasm` emits `NormalizedValue` instances for animation
  tracks. Components in `@vizij/animation-react` surface those values directly,
  letting UI code format them via the `valueAs*` helpers.
- `@vizij/node-graph-react` stages inputs by calling `toValueJSON` before
  invoking the WASM graph bindings, guaranteeing consistent casing even when
  callers pass raw primitives.
- Apps can forward values between the animation and graph runtimes without
  bespoke switch statements:

  ```ts
  // Stage the latest animation value into a graph input.
  const value = useAnimTarget("float:controllers/jitter");
  if (value) {
    runtime.stageInput(path, toValueJSON(value));
  }
  ```

- When plotting or logging values, prefer `valueAsVector`/`valueAsNumber` rather
  than hand-rolling casing logic. This keeps demo apps aligned with whatever the
  Rust side emits.

## Testing

The package ships with Vitest coverage (`npm test` inside
`vizij-rs/npm/@vizij/value-json`) that exercises coercion edge cases. Add new
scenarios there when you extend the helpers.

## License

Apache-2.0, identical to the rest of the Vizij project.
