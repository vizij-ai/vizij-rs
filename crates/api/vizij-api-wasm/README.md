# vizij-api-wasm

> WebAssembly helpers for validating and converting Vizij `Value` and `WriteBatch` JSON.

`vizij-api-wasm` is the smallest wasm crate in the workspace. It wraps `vizij-api-core` with `wasm-bindgen` so browser and Node tooling can reuse the Rust-side parsers without pulling in the animation or graph runtimes.

## Exports

| Function | Description |
|----------|-------------|
| `validate_value_json(json)` | Parses a `Value` JSON string and returns `undefined` on success. Throws a JS error on failure. |
| `value_to_js(json)` | Parses a `Value` JSON string and returns the JS object in canonical Arora `Value` serde form. |
| `validate_writebatch_json(json)` | Parses a `WriteBatch` JSON string (an array of `{ path, value, shape? }` objects) and validates it. |
| `writebatch_to_js(json)` | Parses a `WriteBatch` JSON string and returns the JS object with values in canonical Arora serde form. |

Values are exchanged in Arora `Value` serde form (externally tagged: `{"f32": 1}`, `{"bool": true}`, `{"str": "hi"}`, `{"f32s": [...]}`, `{"struct": {...}}`, ...). Ingress goes through the `vizij-api-core` normalizer, so legacy payload forms (`{"vec3": [0, 1, 2]}`, `{"type": "float", "data": 1}`, bare primitives, ...) are still accepted; outputs are always the canonical form.

The helpers are stateless and safe to call repeatedly.

## Build

This crate is private to the workspace and does not have a dedicated root script today. Build it directly with `wasm-pack`:

```bash
wasm-pack build crates/api/vizij-api-wasm --target web --release
```

That command writes output to `crates/api/vizij-api-wasm/pkg/`.

## Usage

```ts
import init, {
  validate_value_json,
  value_to_js,
  validate_writebatch_json,
  writebatch_to_js,
} from "./pkg/vizij_api_wasm.js";

await init();

validate_value_json('{"f32": 1.5}');
// Legacy forms normalize to the canonical Arora serde on ingress.
const value = value_to_js('{"vec3":[0,1,2]}'); // -> { struct: { id: ..., fields: [...] } }

validate_writebatch_json(
  '[{"path":"robot/Arm.joint","value":{"f32":1}}]'
);
const batch = writebatch_to_js(
  '[{"path":"robot/Arm.joint","value":{"float":1}}]' // legacy value form, still accepted
);

console.log(value, batch);
```

On parse failure the helpers surface the same error strings produced by `serde_json` and the core data model.

## Development And Testing

There is no dedicated wasm-bindgen test suite in this crate yet. The cheapest verification is to build it with `wasm-pack` and exercise the generated `pkg/` entry from a small Node or browser smoke script.

## Related Packages

- [`vizij-api-core`](../vizij-api-core/README.md)
- [`@vizij/value-json`](../../../npm/@vizij/value-json/README.md)
- The animation and graph wasm stacks all depend on the same JSON contracts.
