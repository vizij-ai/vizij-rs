# Changelog

All notable changes to `vizij-api-core`. The format follows
[Keep a Changelog](https://keepachangelog.com/); versions follow
[Semantic Versioning](https://semver.org/).

## [1.1.0] - 2026-09-10

### Changed

- Integers are scalars to the vizij vocabulary: `kind` classifies every
  integer `Value` as `Float`, and `as_float` widens one. The store carries
  integers too — the runtime's `dt` in nanoseconds, a frame's dimensions —
  and arithmetic on them is arithmetic, so a graph reads them without a
  conversion node.

## [1.0.0] - 2026-07-31

First published release. `Value` is `arora_types::value::Value` — the one
runtime value type Vizij shares with Arora's store, modules, behaviors and
Studio — with the vizij vocabulary declared over it: the composite type ids
(`vec2`, `vec3`, `vec4`, `quat`, `color-rgba`, `transform`) with their
constructors and accessors, `VizijKind`, blending and coercion, `Shape`,
`TypedPath`, `WriteOp` / `WriteBatch`, the `Blackboard`, and the JSON
normalisation that accepts every historical payload form and produces arora
serde.
