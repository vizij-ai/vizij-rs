# vizij-api-wasm

`vizij-api-wasm` exposes helpers for working with Vizij value and write-batch JSON inside WebAssembly contexts. It wraps
`vizij-api-core` with `wasm-bindgen`, providing small utility functions that validate JSON payloads or convert them into JS values
without pulling higher-level engines into the WASM binary.

## Overview

* Compiles to a `cdylib` via `wasm-bindgen`.
* Re-uses `vizij-api-core`'s `Value` and `WriteBatch` types for schema validation.
* Provides ergonomic entry points for JavaScript/TypeScript tooling that need to sanity-check JSON before handing it to engines.
* Returns friendly `JsValue` errors so callers can surface human-readable diagnostics.

## Architecture

```
JSON string  -->  vizij-api-core parsing  -->  wasm-bindgen exports  -->  JavaScript tooling
```

* `validate_value_json` / `validate_writebatch_json` parse JSON into the Rust types and bubble up any serde errors.
* `value_to_js` / `writebatch_to_js` parse the JSON and convert the result into `JsValue` via `serde_wasm_bindgen::to_value`.
* No global state is kept—each helper performs pure conversions and returns either `()` or a converted JS object.

## Installation

Inside the workspace build the crate with the usual wasm tooling. For example:

```bash
wasm-pack build crates/api/vizij-api-wasm --target bundler --out-dir pkg --release
```

This crate is typically consumed indirectly by other WASM bindings, but the command above is sufficient when testing it
independently.

## Usage

```ts
import init, {
  validate_value_json,
  value_to_js,
  validate_writebatch_json,
  writebatch_to_js,
} from "vizij-api-wasm";

await init();

// Validate a candidate Value payload
try {
  validate_value_json('{"type":"Vec3","data":[0,1,2]}');
} catch (err) {
  console.error("Value JSON invalid", err);
}

// Convert a WriteBatch JSON string into a JS object
const writes = writebatch_to_js(
  '{"writes":[{"path":"robot/Arm.joint","value":{"type":"Float","data":1.0}}]}'
);
console.log(writes);
```

## Key Details

* All helpers accept UTF-8 JSON strings. Invalid UTF-8 should be sanitized before calling into WASM.
* The conversion helpers (`*_to_js`) allocate new JS objects each call—cache them if you call them every frame.
* Error messages mirror the `thiserror` implementations in `vizij-api-core`, giving tooling the same context as the Rust side.

## Testing

Build the WASM crate and exercise the bindings from Node or browser tests. From the workspace root:

```bash
wasm-pack test crates/api/vizij-api-wasm --headless --chrome
```

(Adjust the target or browser to match your local environment.)
