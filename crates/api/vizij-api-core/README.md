# vizij-api-core

> **The vizij vocabulary over `arora_types::value::Value`, plus the Shape/TypedPath/WriteBatch contracts used by every Vizij engine, adapter, and tooling surface.**

Vizij runs on Arora: the store, the modules, the behaviors, and Studio all speak `arora_types::value::Value`. This crate re-exports that `Value` and declares vizij's vocabulary on top of it — the composite type ids, the constructors and accessors, and the helpers engines need to blend, coerce, and serialize values consistently.

---

## Table of Contents

1. [Overview](#overview)
2. [Installation](#installation)
3. [Key Concepts](#key-concepts)
4. [Examples](#examples)
5. [Development & Testing](#development--testing)
6. [Related Packages](#related-packages)

---

## Overview

- Rust 2021 library over `arora-types` (plus `serde`, `serde_json`, `hashbrown`, `thiserror`, `uuid`).
- Re-exports `arora_types::value::Value` and declares the vizij composite type ids (`vec2`/`vec3`/`vec4`/`quat`/`color-rgba`/`transform`) with constructors and accessors.
- Defines `Shape` metadata shared by animation and graph stacks.
- Implements canonical `TypedPath` parsing/formatting for target identifiers.
- Supplies `WriteOp` / `WriteBatch` helpers so engines can communicate deterministic side effects.
- Ships JSON normalisation utilities that accept every historical payload form and produce Arora `Value` serde.

---

## Installation

```bash
cargo add vizij-api-core
```

No feature flags are required; the crate is always built with the full surface enabled.
`serde` support is always compiled in so values, `Shape`, and `TypedPath` can be serialised/deserialised across Rust and wasm hosts. Disabling `serde` or targeting `no_std` is currently unsupported; downstream engines rely on these derives.

---

## Key Concepts

### Value: the vizij vocabulary

`Value` is Arora's enum; the `value` module gives it vizij semantics:

- **Type ids** (`value::VEC3_TYPE`, `value::TRANSFORM_TYPE`, ...) are UUIDs namespaced under the ASCII bytes of "vizij". Composites are `Value::Structure` with these ids; the ids are shared by module codegen and Studio introspection.
- **Constructors** build the canonical encoding: `float`, `bool_`, `text`, `vector`, `vec2`, `vec3`, `vec4`, `quat`, `color_rgba`, `transform`, `record`, `array`, `enumeration`.
- **Accessors** read values back into PODs: `as_float`, `as_bool`, `as_text`, `as_vector`, `as_vec2/3/4`, `as_quat`, `as_color_rgba`, `as_transform`, `as_record`, `as_array`, `as_enumeration`. Kernels decode a value once through these, do their math on plain Rust types, and re-encode at the store boundary.
- **`kind(&Value) -> VizijKind`** classifies a value for dispatch; anything outside the vocabulary is `VizijKind::Other` and flows through untouched.

Mapping: `f32` -> `F32`, `bool` -> `Boolean`, text -> `String`, numeric vector -> `ArrayF32`, composites -> `Structure`, records -> `KeyValue` (field ids derived from key names), sequences -> `ArrayValue`, enums -> native `Enumeration` (variant ids derived from variant names via `value::variant_id`).

### Shape

- `ShapeId` mirrors the declared structural forms (`Scalar`, `Vec3`, `Transform`, `Record`, …). `Shape` wraps the ID plus optional metadata (`HashMap<String, String>`).
- Shapes are declared metadata about a path, not part of the wire value. Where the wire value cannot express a declared distinction — arora has a single sequence kind, so array/list/tuple all travel as `ArrayValue` — the path's `Shape` preserves it.

### TypedPath

- Canonical identifiers follow `namespace/.../target.field.subfield`.
- `TypedPath::parse` validates grammar; the type implements `Display`, `FromStr`, `Serialize`, and `Deserialize`.
- Used everywhere a value needs to be identified consistently across engines (animation targets, graph sinks, blackboard keys).

### Write Operations

- `WriteOp` captures a single `{ path, value, shape? }` produced by an engine; it serialises with the path as a string and the value in Arora serde form.
- `WriteBatch` is a thin wrapper around `Vec<WriteOp>` with append helpers and serde support.
- Engines use `WriteBatch` to communicate external side effects to hosts.

### JSON Normalisation

The canonical JSON form of a value is Arora `Value`'s own serde: `{"f32": 1.0}`, `{"str": "hi"}`, `{"f32s": [...]}`, `{"struct": {"id": ..., "fields": [...]}}`, `{"keyvalue": ...}`, `{"enum": {"id": ..., "variant_id": ..., "value": ...}}`. `json::parse_value` and `json::normalize_value_json` additionally accept every payload form vizij hosts have emitted, and produce the canonical form:

| Accepted input | Reading |
|----------------|---------|
| `1.0`, `true`, `"hi"` | float / bool / text |
| `[1, 2]`, `[1, 2, 3]`, `[1, 2, 3, 4]` | vec2 / vec3 / vec4 (`AutoVectorKinds` policy; `AlwaysVector` reads all numeric arrays as generic vectors) |
| `{"float": 1}`, `{"bool": true}`, `{"text": "hi"}` | float / bool / text |
| `{"vec3": [0, 1, 0]}` (also `vec2`/`vec4`/`quat`/`color`/`vector`) | the corresponding composite |
| `{"transform": {"translation": ..., "rotation": ..., "scale": ...}}` | transform (components as arrays or `{x, y, z[, w]}` objects) |
| `{"enum": {"tag": "On", "value": ...}}` | native `Enumeration` with `variant_id("On")` |
| `{"record": {...}}` | `KeyValue` record |
| `{"array"\|"list"\|"tuple": [...]}` | `ArrayValue` sequence |
| `{"type": "vec3", "data": [0, 1, 0]}` (any type tag) | the corresponding value |
| `{"x": ..., "y": ...}` / `{x, y, z}` / `{x, y, z, w}` | vec2 / vec3 / quat |
| canonical Arora serde | passed through unchanged |

This normaliser is the single entry point for migrating persisted documents (e.g. Value-bearing JSON embedded in `.glb` face bundles) to the canonical form. Values serialise back to JSON with plain `serde_json`; there is no producer of the legacy forms. `json::writebatch_from_json` reads write batches whose values use any accepted form.

Graph specs are normalised so node shorthands stay ergonomic while the runtime always sees the canonical schema:

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
    "rhs": { "value": { "f32": 2.0 } }
  }
}
```

This means authors can skip boilerplate constant nodes, but hosts still receive a deterministic graph definition with explicit edges and defaults.

### Blending and coercion

- `blend::blend_values` decodes both operands into PODs, blends (lerp for floats/vectors/colors, slerp for quaternions, TRS-wise for transforms, field-wise for records, index-wise for sequences), and re-encodes. `blend::step_blend` picks an operand whole for step-only kinds.
- `coercion::to_float` / `to_vector` / `to_vec3` give every value a lossy numeric reading so mixed-kind blends and adapters always have something sensible to work with.

---

## Examples

```rust
use vizij_api_core::{json, value, Shape, ShapeId, TypedPath, WriteBatch, WriteOp};

let path = TypedPath::parse("robot/Arm/Joint3.rotation")?;
let rotation = value::quat([0.0, 0.0, 0.0, 1.0]);
let shape = Shape::new(ShapeId::Quat);

let mut batch = WriteBatch::new();
batch.push(WriteOp::new_with_shape(path.clone(), rotation.clone(), Some(shape)));

// Kernels decode once into PODs:
let pod: [f32; 4] = value::as_quat(&rotation).expect("quat");

// The normaliser reads any historical payload form:
let parsed = json::parse_value(serde_json::json!({ "vec3": [0.0, 1.0, 0.0] }))?;
assert_eq!(value::as_vec3(&parsed), Some([0.0, 1.0, 0.0]));
```

---

## Development & Testing

```bash
cargo test -p vizij-api-core
```

Modules include unit tests for the constructor/accessor round-trips, blending, coercion, and normalisation. Run `pnpm run test:rust` from the repo root to exercise the entire workspace.

---

## Related Packages

- [`vizij-api-wasm`](../vizij-api-wasm/README.md) – wasm helpers that mirror the same normalisation logic for JavaScript.
- Engine stacks (`vizij-animation-core`, `vizij-graph-core`) all depend on these contracts.

Questions or improvements? Open an issue—shared contracts are the backbone of Vizij interoperability. 🔗
