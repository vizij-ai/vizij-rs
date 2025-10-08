# @vizij/value-json

> **TypeScript definitions and helpers for Vizij’s Value/Shape ecosystem.**

`@vizij/value-json` is the canonical TypeScript companion to `vizij-api-core`. It defines the normalised `{ type, data }` value union, offers coercion utilities, and keeps every Vizij npm package (animation, node graph, orchestrator, renderer) speaking the same JSON dialect. Install it whenever your tooling, UI, or Node service needs to produce or consume Vizij values.

---

## Table of Contents

1. [Overview](#overview)
2. [Key Concepts](#key-concepts)
3. [Installation](#installation)
4. [Key Types](#key-types)
5. [Utilities](#utilities)
6. [Usage Examples](#usage-examples)
7. [Development & Testing](#development--testing)
8. [Related Packages](#related-packages)

---

## Overview

- Mirrors the canonical `{ type: "...", data: ... }` envelope emitted by Vizij engines and WASM runtimes.
- Accepts legacy `{ float: 1 }`, `{ vec3: [...] }` shapes for backwards compatibility while gently nudging you toward the normalised form.
- Ships coercion helpers (`toValueJSON`, `valueAsNumber`, `valueAsTransform`, etc.) that front-ends and tooling can rely on.
- Ensures discriminants stay lowercase so string comparisons remain consistent across ecosystems.

---

## Key Concepts

- **ValueJSON** – Union type that handles both canonical `{ type, data }` payloads and legacy helpers (`{ float: 1 }`, `number[]`, primitives).
- **NormalizedValue** – Strict `{ type, data }` shape emitted by Vizij runtimes; use `isNormalizedValue` to detect it.
- **Shape Metadata** – Optional `ShapeJSON` structures travel alongside values so tooling understands numeric layout (`Vec3`, `Transform`, etc.).
- **Coercion Helpers** – Utilities (`toValueJSON`, `valueAsNumericArray`, `valueAsTransform`, etc.) convert between loose JavaScript data and the strict Vizij schema.

---

## Installation

```bash
pnpm add @vizij/value-json
# or npm install @vizij/value-json
```

Within the monorepo the package is built from `vizij-rs/npm/@vizij/value-json`.

---

## Key Types

```ts
type NormalizedValue =
  | { type: "float"; data: number }
  | { type: "vec3"; data: [number, number, number] }
  | { type: "quat"; data: [number, number, number, number] }
  | { type: "transform"; data: NormalizedTransform }
  | { type: "vector"; data: number[] }
  | { type: "enum"; data: [string, NormalizedValue] }
  | { type: "record"; data: Record<string, NormalizedValue> }
  | ...;

type ValueJSON = NormalizedValue | { float: number } | { vec3: number[] } | number | boolean | string | number[];
```

- `NormalizedValue` – canonical tagged union.
- `ValueJSON` – accepts both normalized values and legacy aliases/primitives for input convenience.
- `ValueInput` – alias for `ValueJSON | number[]`, used by staging helpers in other packages.
- `NormalizedTransform` – `{ translation: [x,y,z], rotation: [x,y,z,w], scale: [x,y,z] }`.

---

## Utilities

| Helper | Description |
|--------|-------------|
| `toValueJSON(value: ValueInput): ValueJSON` | Coerces primitives/arrays/legacy objects into the canonical union. |
| `isNormalizedValue(value: ValueJSON): value is NormalizedValue` | Type guard for lowercased `{ type, data }` values. |
| `valueAsNumber(value)` | Extracts the first numeric component (floats, vectors, transforms, enums). |
| `valueAsNumericArray(value, fallback = 0)` | Flattens numeric payloads into an array. |
| `valueAsVector(value)` | Returns a numeric array or `undefined` if coercion fails. |
| `valueAsTransform(value)` | Returns a `[translation, rotation, scale]` tuple with defaults for missing components. |
| `valueAsQuat`, `valueAsVec3`, `valueAsColorRgba`, `valueAsBool`, `valueAsText` | Convenience accessors for common types. |

All readers return `undefined` when coercion fails, letting callers handle optional values explicitly.

---

## Usage Examples

Normalising inputs before staging them into WASM bindings:

```ts
import { toValueJSON } from "@vizij/value-json";
import { Graph } from "@vizij/node-graph-wasm";

graph.stageInput("nodes.inputA.inputs.in", toValueJSON([1, 2, 3]));
graph.stageInput("nodes.mode.inputs.in", toValueJSON({ enum: { tag: "A", value: { float: 1 } } }));
```

Reading values emitted by the animation engine:

```ts
import { valueAsNumber, valueAsTransform } from "@vizij/value-json";

const value = outputs.changes[0]?.value;
const scalar = valueAsNumber(value);
const transform = valueAsTransform(value);
```

Type guard usage:

```ts
import { isNormalizedValue } from "@vizij/value-json";

if (isNormalizedValue(value)) {
  console.log(value.type); // narrow to canonical discriminants
}
```

---

## Development & Testing

From the package directory:

```bash
pnpm install
pnpm test
```

Vitest covers coercion edge cases and regression scenarios. Add to the suite whenever you extend the helper surface.

---

## Related Packages

- [`vizij-api-core`](../../../crates/api/vizij-api-core/README.md) – Rust source of truth for Value/Shape types.
- [`@vizij/node-graph-wasm`](../node-graph-wasm/README.md) • [`@vizij/orchestrator-wasm`](../orchestrator-wasm/README.md) • [`@vizij/animation-wasm`](../animation-wasm/README.md) – WASM packages that rely on these helpers.
- React bindings (`@vizij/node-graph-react`, `@vizij/orchestrator-react`, `@vizij/animation-react`) use the helpers to normalise staged inputs and render outputs.

Questions or improvements? Open an issue—aligned value handling keeps Vizij runtimes interoperable. 🔄
