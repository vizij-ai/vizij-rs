# vizij-api-wasm

> WebAssembly helpers for validating and converting Vizij `Value` and `WriteBatch` JSON.

`vizij-api-wasm` is the smallest wasm crate in the workspace. It wraps `vizij-api-core` with `wasm-bindgen` so browser and Node tooling can reuse the Rust-side parsers without pulling in the animation, graph, or orchestrator runtimes.

## Exports

| Function | Description |
|----------|-------------|
| `validate_value_json(json)` | Parses a `Value` JSON string and returns `undefined` on success. Throws a JS error on failure. |
| `value_to_js(json)` | Parses a `Value` JSON string and returns the serde-produced JS object. |
| `validate_writebatch_json(json)` | Parses a `WriteBatch` JSON string and validates it. |
| `writebatch_to_js(json)` | Parses a `WriteBatch` JSON string and returns the serde-produced JS object. |

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

validate_value_json('{"type":"vec3","data":[0,1,2]}');
const value = value_to_js('{"vec3":[0,1,2]}');

validate_writebatch_json(
  '{"writes":[{"path":"robot/Arm.joint","value":{"float":1}}]}'
);
const batch = writebatch_to_js(
  '{"writes":[{"path":"robot/Arm.joint","value":{"float":1}}]}'
);

console.log(value, batch);
```

On parse failure the helpers surface the same error strings produced by `serde_json` and the core data model.

## Development And Testing

There is no dedicated wasm-bindgen test suite in this crate yet. The cheapest verification is to build it with `wasm-pack` and exercise the generated `pkg/` entry from a small Node or browser smoke script.

## Related Packages

- [`vizij-api-core`](../vizij-api-core/README.md)
- [`@vizij/value-json`](../../npm/@vizij/value-json/README.md)
- The animation, graph, and orchestrator wasm stacks all depend on the same JSON contracts.
