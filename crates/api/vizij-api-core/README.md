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
`serde` support is always compiled in so `Value`, `Shape`, and `TypedPath` can be serialised/deserialised across Rust and wasm hosts. Disabling `serde` or targeting `no_std` is currently unsupported; downstream engines rely on these derives.

---

## Key Concepts

### Value & Shape

- `Value` is a tagged enum serialised with `{ "type": "...", "data": ... }`. Helper constructors (`Value::vec3`, `Value::quat`, etc.) simplify native code.
- `ShapeId` mirrors the possible structural forms (`Scalar`, `Vec3`, `Transform`, `Record`, …). `Shape` wraps the ID plus optional metadata (`HashMap<String, String>`).
- Consuming crates rely on shape metadata to catch schema drift early and to build “null-of-shape” placeholders (e.g., NaN vectors).

| Variant | Canonical JSON | Legacy shorthand input |
|---------|----------------|------------------------|
| `Value::Float(1.0)` | `{"type":"float","data":1.0}` | `{"float":1}` |
| `Value::Bool(true)` | `{"type":"bool","data":true}` | `{"bool":true}` |
| `Value::Vec3([0,1,0])` | `{"type":"vec3","data":[0,1,0]}` | `{"vec3":[0,1,0]}` |
| `Value::Quat([0,0,0,1])` | `{"type":"quat","data":[0,0,0,1]}` | `{"quat":[0,0,0,1]}` |
| `Value::ColorRgba([1,0,0,1])` | `{"type":"colorrgba","data":[1,0,0,1]}` | `{"color":[1,0,0,1]}` |
| `Value::Transform { .. }` | `{"type":"transform","data":{"translation":[0,0,0],"rotation":[0,0,0,1],"scale":[1,1,1]}}` | `{"transform":{"translation":[0,0,0],"rotation":[0,0,0,1],"scale":[1,1,1]}}` |
| `Value::Enum("State", box Value::Bool(true))` | `{"type":"enum","data":["State",{"type":"bool","data":true}]}` | `{"enum":{"tag":"State","value":{"bool":true}}}` |
| `Value::Record({ "joint": Value::Float(2.0) })` | `{"type":"record","data":{"joint":{"type":"float","data":2.0}}}` | `{"record":{"joint":{"float":2}}}` |
| `Value::Array([Value::Float(0.0)])` | `{"type":"array","data":[{"type":"float","data":0.0}]}` | `{"array":[{"float":0}]}` |
| `Value::List([Value::Vec3([1,1,1])])` | `{"type":"list","data":[{"type":"vec3","data":[1,1,1]}]}` | `{"list":[{"vec3":[1,1,1]}]}` |
| `Value::Tuple([Value::Float(1.0), Value::Bool(false)])` | `{"type":"tuple","data":[{"type":"float","data":1.0},{"type":"bool","data":false}]}` | `{"tuple":[{"float":1},{"bool":false}]}` |
| `Value::Text("hello")` | `{"type":"text","data":"hello"}` | `{"text":"hello"}` |

Use the canonical JSON form when serialising fixtures or interoperating with other runtimes; the legacy column remains accepted on input for backwards compatibility.

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
- Graph specs are normalised so node shorthands stay ergonomic while the runtime always sees the canonical schema:
  - Node `type` strings are lowercased and legacy `kind` aliases are rewritten.
  - Inline `inputs` maps are expanded into the top-level `edges` array so wiring is explicit.
  - Scalar/boolean/vector literals placed on an input (for example `"rhs": 2`) are lifted into `node.input_defaults` and survive the `inputs → edges` rewrite.
  - Connection objects can provide both wiring and fallbacks (`{ "node_id": "config", "default": 0.5 }`), which normalise into a link plus an `input_defaults` entry.
  - Optional `default_shape` or `shape` keys are accepted (string IDs become `{ "id": "Scalar" }`) so downstream coercion can infer the intended layout.

  ```jsonc
  // authoring shorthand
  {
    "id": "scale",
    "type": "multiply",
    "inputs": {
      "lhs": "sensor_gain",
      "rhs": 2
    }
  }

  // normalised representation consumed by vizij-graph-core
  {
    "id": "scale",
    "type": "multiply",
    "input_defaults": {
      "rhs": { "value": { "type": "float", "data": 2.0 } }
    }
  }
  ```

  This means authors can skip boilerplate constant nodes, but hosts still receive a deterministic graph definition with explicit edges and defaults.

  ```jsonc
  // enum/record defaults survive normalisation
  {
    "id": "mode-selector",
    "type": "switch",
    "inputs": {
      "mode": { "enum": { "tag": "On", "value": { "bool": true } } }
    },
    "input_defaults": {
      "payload": { "record": { "intensity": { "float": 0.75 } } }
    }
  }

  // becomes
  {
    "id": "mode-selector",
    "type": "switch",
    "input_defaults": {
      "mode": {
        "value": {
          "type": "enum",
          "data": ["On", { "type": "bool", "data": true }]
        }
      },
      "payload": {
        "value": {
          "type": "record",
          "data": {
            "intensity": { "type": "float", "data": 0.75 }
          }
        }
      }
    }
  }
  ```

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
