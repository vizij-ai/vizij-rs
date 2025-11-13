# vizij-blackboard-wasm

> **wasm-bindgen bridge for Vizij blackboard  hierarchical key-value storage accessible from JavaScript.**

`vizij-blackboard-wasm` compiles `vizij-blackboard-core` to WebAssembly and exposes a friendly API for TypeScript tooling. The npm wrapper `@vizij/blackboard-wasm` builds on this crate.

---

## Table of Contents

1. [Overview](#overview)
2. [Exports](#exports)
3. [Building](#building)
4. [Usage](#usage)
5. [Key Details](#key-details)
6. [Testing](#testing)
7. [Related Packages](#related-packages)

---

## Overview

- Compiles to a `cdylib` via `wasm-bindgen` with ABI guard `abi_version() == 1`.
- Wraps an `ArcBlackboard` inside the `VizijBlackboard` class for thread-safe access.
- Provides hierarchical key-value storage using token-separated paths (e.g., `robot.arm.joint1.angle`).
- Supports basic JavaScript types: numbers, strings, booleans, and arrays.
- Optional features:
  - `console_error_panic_hook` (default)  enable `console_error_panic_hook` for clearer browser diagnostics.

---

## Exports

| Export | Description |
|--------|-------------|
| `class VizijBlackboard` | Methods: `new`, `set`, `get`, `remove`, `has`, `list_paths`, `clear`, `name`, `size`. |
| `abi_version() -> u32` | Returns `1`; used by npm wrappers to enforce compatibility. |

---

## Building

From the repository root:

```bash
pnpm run build:wasm:blackboard      # recommended path (invokes build script)
```

Manual build:

```bash
wasm-pack build crates/blackboard/vizij-blackboard-wasm \
  --target bundler \
  --out-dir pkg \
  --release
```

The `pkg/` directory is consumed by `npm/@vizij/blackboard-wasm`.

---

## Usage

Using the npm wrapper (recommended):

```ts
import { init, VizijBlackboard } from "@vizij/blackboard-wasm";

await init();

const bb = new VizijBlackboard("my-blackboard");

// Set values using dot-separated paths
const id = bb.set("robot.arm.angle", 45.0);
bb.set("robot.name", "R2D2");
bb.set("config.speeds", [1.0, 2.5, 3.0]);

// Get values
const angle = bb.get("robot.arm.angle");  // Returns 45.0
const name = bb.get("robot.name");        // Returns "R2D2"

// Check existence
const exists = bb.has("robot.arm.angle"); // Returns true

// Remove values
bb.remove("robot.arm.angle");
```

Direct WASM usage without the wrapper:

```ts
import initWasm, { VizijBlackboard } from "@vizij/blackboard-wasm/pkg";

await initWasm();
const bb = new VizijBlackboard("test");
bb.set("path.to.value", 42);
const value = bb.get("path.to.value");
console.log(value); // 42
```

---

## Key Details

- **Hierarchical Paths**  Uses dot-separated notation for accessing nested data (e.g., `robot.arm.joint1.angle`).
- **Value Types**  Supports JavaScript primitives (number, string, boolean) and arrays. Complex objects are not yet supported.
- **Type Inference**  Arrays are type-homogeneous; the type is inferred from the first element.
- **UUID Returns**  `set()` returns a UUID string identifying the stored item, or an empty string if the value was null/undefined (removal).
- **Thread Safety**  Internal `Arc<Mutex<ArcBlackboard>>` ensures safe concurrent access.
- **Placeholder Methods**  `remove()`, `list_paths()`, and `size()` are placeholder implementations pending core functionality.

---

## Testing

```bash
pnpm run build:wasm:blackboard      # ensure pkg/ is up to date
cd npm/@vizij/blackboard-wasm
pnpm test
```

For Rust-only coverage run:

```bash
cargo test -p vizij-blackboard-wasm
```

---

## Related Packages

- [`vizij-blackboard-core`](../vizij-blackboard-core/README.md)  core blackboard implementation used by this crate.
- [`npm/@vizij/blackboard-wasm`](../../../npm/@vizij/blackboard-wasm/README.md)  npm package built from this binding.

Need assistance? Open an issueshared blackboard state keeps Vizij components synchronized.
