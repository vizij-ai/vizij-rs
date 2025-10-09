# vizij-api-core

> **Shared Value/Shape/TypedPath contracts used by every Vizij engine, adapter, and tooling surface.**

`vizij-api-core` provides the canonical data model for Vizij. Engines emit `Value` changes and typed write batches; hosts, Bevy plugins, and WebAssembly bindings all depend on this crate to speak the same language.

---

## Table of Contents

1. [Overview](#overview)
2. [Features](#features)
3. [Installation](#installation)
4. [Key Concepts](#key-concepts)
5. [Examples](#examples)
6. [Development & Testing](#development--testing)
7. [Related Packages](#related-packages)

---

## Overview

- Rust 2021 library with minimal dependencies (`serde`, `hashbrown`, `thiserror`).
- Defines the tagged `Value` enum and `Shape` metadata shared by animation, graph, and orchestrator stacks.
- Implements canonical `TypedPath` parsing/formatting for target identifiers.
- Supplies `WriteOp` / `WriteBatch` helpers so engines can communicate deterministic side effects.
- Ships JSON normalisation utilities used by the WASM bindings and fixtures.

---

## Features

- Structured values covering scalars, vectors, quaternions, colours, transforms, enums, records, arrays, lists, tuples, text, and bool.
- Shape metadata (`ShapeId`, `Shape`) for validation and tooling.
- Deterministic path grammar with serde support.
- Lightweight write-batch helpers with provenance-friendly metadata.
- JSON coercion helpers for parsing shorthand payloads into the canonical `{ "type": "...", "data": ... }` envelope.

---

## Installation

```bash
cargo add vizij-api-core
```

No feature flags are required; the crate is always built with the full surface enabled.

---

## Key Concepts

### Value & Shape

- `Value` is a tagged enum serialised with `{ "type": "...", "data": ... }`. Helper constructors (`Value::vec3`, `Value::quat`, etc.) simplify native code.
- `ShapeId` mirrors the possible structural forms (`Scalar`, `Vec3`, `Transform`, `Record`, …). `Shape` wraps the ID plus optional metadata (`HashMap<String, String>`).
- Consuming crates rely on shape metadata to catch schema drift early and to build “null-of-shape” placeholders (e.g., NaN vectors).

### TypedPath

- Canonical identifiers follow `namespace/.../target.field.subfield`.
- `TypedPath::parse` validates grammar; the type implements `Display`, `FromStr`, `Serialize`, and `Deserialize`.
- Used everywhere a value needs to be identified consistently across engines (animation targets, graph sinks, blackboard keys).

### Write Operations

- `WriteOp` captures a single `{ path, value, shape? }` produced by an engine.
- `WriteBatch` is a thin wrapper around `Vec<WriteOp>` with append helpers and serde support.
- Engines use `WriteBatch` to communicate external side effects to orchestrators or hosts.

### JSON Normalisation

- The `json` module converts shorthand objects (`{ float: 1 }`, `{ vec3: [0,1,0] }`) into canonical `Value` envelopes.
- Shared by WASM bindings (`vizij-*-wasm`), fixtures, and hosted tools to ensure identical parsing across environments.
- Legacy helpers (`value_to_legacy_json`, `writebatch_to_legacy_json`) keep older tools functioning during transitions.

---

## Examples

```rust
use vizij_api_core::{Shape, ShapeId, TypedPath, Value, WriteBatch, WriteOp};

let path = TypedPath::parse("robot/Arm/Joint3.rotation")?;
let value = Value::quat(0.0, 0.0, 0.0, 1.0);
let shape = Shape::new(ShapeId::Quat);

let mut batch = WriteBatch::new();
batch.push(WriteOp::new_with_shape(path.clone(), value.clone(), Some(shape.clone())));

for op in batch.iter() {
    println!("{} => {:?} ({:?})", op.path, op.value, op.shape.as_ref().map(|s| &s.id));
}
```

Parsing JSON using the normaliser:

```rust
use vizij_api_core::json;
use serde_json::json;

let raw = json!({ "vec3": [0.0, 1.0, 0.0] });
let canonical = json::normalize_value_json(raw);
let value: vizij_api_core::Value = serde_json::from_value(canonical)?;
```

---

## Development & Testing

```bash
cargo test -p vizij-api-core
```

Modules include unit tests for parsing, coercion, and normalisation. Run `pnpm run test:rust` from the repo root to exercise the entire workspace.

---

## Related Packages

- [`vizij-api-wasm`](../vizij-api-wasm/README.md) – wasm helpers that mirror the same normalisation logic for JavaScript.
- [`bevy_vizij_api`](../bevy_vizij_api/README.md) – Bevy utilities built on top of this crate.
- Engine stacks (`vizij-animation-core`, `vizij-graph-core`, `vizij-orchestrator-core`) all depend on these contracts.

Questions or improvements? Open an issue—shared contracts are the backbone of Vizij interoperability. 🔗
