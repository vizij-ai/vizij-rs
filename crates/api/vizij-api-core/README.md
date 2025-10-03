# vizij-api-core

`vizij-api-core` centralizes the shared data contracts that flow between Vizij engines, host applications, and tooling. It
defines JSON-friendly `Value` and `Shape` types, typed canonical paths, write batches, and helper utilities so that every domain
stack (animation, node graph, upcoming blackboard/behaviour-tree systems) can speak the same language.

## Overview

* Rust 2021 library with zero runtime dependencies outside of `serde`, `hashbrown`, and `thiserror`.
* Ships the canonical `Value` enum used for animation samples, graph ports, write operations, and WebAssembly bindings.
* Provides `Shape`/`ShapeId` metadata so hosts and tooling can validate or coerce structured values consistently.
* Implements `TypedPath` parsing/formatting for canonical target identifiers shared across engines and adapters.
* Exposes `WriteOp`/`WriteBatch` helpers that encapsulate external writes emitted by engines.
* Includes numeric coercion and blending helpers used by higher-level crates.

## Architecture

```
+--------------------+      +------------------+
| Value / Shape APIs |<---->| Engines & Tools  |
+--------------------+      +------------------+
          ^                          ^
          |                          |
+---------+----------+      +-------+--------+
| TypedPath grammar  |      | WriteOp/Batch  |
+--------------------+      +----------------+
```

* `value.rs` – Tagged enum for scalars, vectors, quaternions, transforms, structured records, enums, and text.
* `shape.rs` – Structural metadata describing the shape of `Value` instances (with optional metadata map).
* `typed_path.rs` – Parser/serializer for canonical target identifiers (namespace/target/field segments).
* `write_ops.rs` – JSON-compatible `WriteOp` and `WriteBatch` types with serde implementations.
* `blend.rs` / `coercion.rs` – Utilities for blending/comparing/coercing values that power engine runtimes.

## Installation

Add the crate to any Rust project that needs to interact with Vizij data contracts (replace the version with the published
release when available):

```bash
cargo add vizij-api-core
```

The crate exposes no optional features.

## Usage

* Construct values with convenience helpers such as `Value::vec3`, `Value::quat`, or by deserializing JSON via `serde_json`.
* Describe the expected shape of a value using `Shape::new(ShapeId::Vec3)` or composite helpers like
  `ShapeId::record_from_pairs`.
* Parse canonical target identifiers via `TypedPath::parse("robot/Arm.joint")`; format them with `to_string()`.
* Batch engine writes with `WriteBatch::push(WriteOp::new_with_shape(...))` before handing the results to adapters (Bevy, WASM,
  etc.).

## Key Details

### Values & serde

* The `Value` enum serializes with a `{ "type": "vec3", "data": [...] }` envelope to remain unambiguous in JSON.
* Convenience constructors (`Value::f`, `Value::vec3`, `Value::transform`) simplify host-side code when producing values
  programmatically.
* `Value::kind()` exposes a lightweight discriminant for quick pattern matching without matching on every variant.

### Shapes

* `ShapeId` mirrors the runtime surface (scalar, vectors, quaternions, transform, vector-of-f32, record, array, list, tuple,
  enum).
* `Shape` wraps a `ShapeId` plus optional metadata (units, coordinate frames, etc.). Metadata is stored as a string map.
* Helpers like `Shape::with_meta` allow hosts to attach annotations without changing the structural shape.

### Typed paths

* Canonical identifiers follow the `namespace/.../target.field.subfield` grammar, enabling deterministic lookups across engines
  and adapters.
* `TypedPath` implements `Display`, `FromStr`, and serde serialization as plain strings so that JSON payloads remain compact.

### Write operations

* `WriteOp` captures `{ path, value, shape }` produced by engines. The optional `shape` travels with the value to avoid schema
  guessing downstream.
* `WriteBatch` is a thin Vec wrapper with iterators, append helpers, and custom serde to preserve the inline JSON format.

### JSON helpers

* The `json` module centralizes normalization so that wasm adapters, fixtures, and the blackboard convert shorthand payloads
  (`{ float: 1.0 }`, `[0, 1, 0]`, enum records, etc.) into the canonical `{ "type": "...", "data": ... }` structure before
  deserializing into `Value`.
* `normalize_graph_spec_value` mirrors the GraphSpec normalizer used in wasm, ensuring Rust tests and JS bundles share identical
  behaviour when upgrading fixtures.
* Legacy serializers (`value_to_legacy_json`, `writebatch_to_legacy_json`) keep backwards compatibility with tooling that still
  expects `{ float: ... }` / `{ vec3: [...] }` envelopes during the migration window.

## Examples

```rust
use vizij_api_core::{Shape, ShapeId, TypedPath, Value, WriteBatch, WriteOp};

let path = TypedPath::parse("robot/Arm/Joint3.rotation").expect("valid path");
let value = Value::quat(0.0, 0.0, 0.0, 1.0);
let shape = Shape::new(ShapeId::Quat);

let mut batch = WriteBatch::new();
batch.push(WriteOp::new_with_shape(path.clone(), value.clone(), Some(shape.clone())));

for op in batch.iter() {
    println!("{} => {:?} (shape {:?})", op.path, op.value, op.shape.as_ref().map(|s| &s.id));
}
```

## Testing

The crate ships unit tests alongside each module. Run them with:

```bash
cargo test -p vizij-api-core
```
