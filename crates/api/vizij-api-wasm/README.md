# vizij-api-wasm

> **WebAssembly helpers for validating and normalising Vizij Value/WriteBatch JSON.**

`vizij-api-wasm` wraps `vizij-api-core` with `wasm-bindgen` so JavaScript and TypeScript tooling can normalise payloads, ensure they conform to the canonical schema, and convert them into `JsValue` instances without bundling the heavier animation/graph engines.

---

## Table of Contents

1. [Overview](#overview)
2. [Exports](#exports)
3. [Building](#building)
4. [Usage](#usage)
5. [Development & Testing](#development--testing)
6. [Related Packages](#related-packages)

---

## Overview

- Compiles to a `cdylib` using `wasm-bindgen`.
- Depends solely on `vizij-api-core` for Value/Shape/TypedPath definitions.
- Provides string-based validation helpers plus converters that return proper JS objects via `serde_wasm_bindgen`.
- Emits ergonomic error messages (mirroring the Rust core) for tooling and editor integrations.
- Shared dependency: `vizij-animation-wasm`, `vizij-graph-wasm`, and `vizij-orchestrator-wasm` call into these helpers before exposing higher-level APIs. Keeping this crate current ensures every wasm surface normalises JSON in the same way.

---

## Exports

| Function | Description |
|----------|-------------|
| `validate_value_json(json: &str)` | Parses and validates a single `Value` JSON string. Throws a JS error on failure. |
| `validate_writebatch_json(json: &str)` | Parses and validates a `WriteBatch` JSON string. |
| `value_to_js(json: &str) -> JsValue` | Normalises and converts a value JSON string into a JS object matching the canonical `{ type, data }` shape. |
| `writebatch_to_js(json: &str) -> JsValue` | Converts a batch JSON string into a JS object `{ writes: [...] }`. |

All helpers accept UTF-8 strings and perform no global state mutations, making them safe to call repeatedly.

---

## Building

From the repository root:

```bash
wasm-pack build crates/api/vizij-api-wasm \
  --target bundler \
  --out-dir pkg \
  --release
```

The generated `pkg/` directory can be imported directly or repackaged as part of a larger bundle.

---

## Usage

```ts
import init, {
  validate_value_json,
  value_to_js,
  validate_writebatch_json,
  writebatch_to_js,
} from "vizij-api-wasm";

await init();

// Validate inputs
validate_value_json('{"type":"vec3","data":[0,1,2]}');

// Convert a Value JSON string to a JS object
const value = value_to_js('{"vec3":[0,1,2]}'); // shorthand accepted
console.log(value); // { type: "vec3", data: [0, 1, 2] }

// Convert a WriteBatch
const writes = writebatch_to_js(
  '{"writes":[{"path":"robot/Arm.joint","value":{"float":1}}]}'
);
console.log(writes.writes[0].value); // { type: "float", data: 1 }
```

Errors returned by `validate_*` include the same context as the Rust parsing layer, making them ideal for diagnostics in editors or CLI tooling.

```ts
try {
  validate_value_json('{"vec3":[0,1]}'); // missing component
} catch (err) {
  const message = err instanceof Error ? err.message : String(err);
  console.warn("Vizij value rejected:", message);
  // Example message: `invalid vec3 length (expected 3, found 2)`
}
```

### When to use the low-level APIs

- Use `vizij-api-wasm` when you need raw normalisation/validation (editor plugins, custom bridge code, Node CLIs) without pulling the heavier animation/graph/orchestrator wasm modules.
- Reach for `@vizij/value-json` when you are already in TypeScript and prefer type-safe helpers that work purely on the JS side. The npm package internally mirrors this crate’s logic and is tree-shakeable.
- Engine wasm packages (`@vizij/animation-wasm`, `@vizij/node-graph-wasm`, `@vizij/orchestrator-wasm`) call into these bindings automatically—most app code does not need to import them directly unless you are authoring tooling.

---

## Development & Testing

```bash
wasm-pack test crates/api/vizij-api-wasm --headless --chrome
```

Change the target (`--node`, `--firefox`, etc.) as needed. For quick smoke tests, import the generated `pkg/` output in a Node script and run the functions above.

---

## Related Packages

- [`vizij-api-core`](../vizij-api-core/README.md) – underlying data model shared across all Vizij stacks.
- [`npm/@vizij/value-json`](../../npm/@vizij/value-json/README.md) – TypeScript helpers that mirror the same JSON normalisation logic.
- WASM bindings for animation, node graphs, and the orchestrator all reuse this crate internally to validate payloads.

Questions or ideas? Open an issue—consistent JSON handling keeps Vizij tooling reliable. 📦
