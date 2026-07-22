# Changelog

All notable changes to `vizij-graph-core`. The format follows
[Keep a Changelog](https://keepachangelog.com/); versions follow
[Semantic Versioning](https://semver.org/).

## [Unreleased]

### Added

- `ExternalFunction` node plus the `NodeFunctions` call-bridge seam: a graph
  node can call out to host-provided functions.
- The path-less `Output` node applies a keyed record batch to its keys: an
  `output` without `path` takes `key_field`/`value_field` params (record field
  ids) and writes each record's value under the path its key field names.

### Changed

- Freshened workspace dependencies to current majors.

## [1.0.0] - 2026-07-10

### Breaking

- Built on the unified Value (vizij-api-core 1.0.0 = `arora_types` `Value`): the
  graph evaluates on PODs with `Value` only at node boundaries; node-registry
  defaults and evaluation output are in arora serde form. `List`/`Tuple`/`Array`
  collapse into one `ArrayValue`; enums ride arora's `Enumeration`.

### Added

- Record nodes; noise generator nodes; to/from vector nodes; a new blend-node
  default.

### Changed

- Graph plan cache and warm WASM fast paths for lower evaluation overhead;
  slot-based port layouts; delta staging tightened; defaults included in
  fingerprinting; caching policy documented.

### Fixed

- Transition nodes not receiving `dt` updates; piecewise-remap plateau handling;
  hardened `evaluate_all_cached` plan validation and selector projection errors;
  clippy `into_iter` lint.

## [0.3.0] - 2025-09-28

### Changed

- Cleaned up and extended graph support for IK; added comprehensive blend-node
  test coverage.

## [0.2.0] - 2025-09-23

### Added

- IK nodes and a demo IK node; an FK node; transition nodes; multi-slider node;
  external inputs to the graph with stronger shape selection by nodes; range-node
  params.

### Changed

- Refactored the node-eval monolith and the node schema; extracted the shared
  types and interfaces into a common API (vizij-api-core); typed paths with
  refined shape/value handling; refactor around vector data.

### Fixed

- Oscillator and join node fixes; new-node evaluation fixes.

## [0.1.0] - 2025-09-05

### Added

- Initial release: core node-graph engine for Vizij.
