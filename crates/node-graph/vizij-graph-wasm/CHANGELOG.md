# Changelog

All notable changes to `vizij-graph-wasm`. The format follows
[Keep a Changelog](https://keepachangelog.com/); versions follow
[Semantic Versioning](https://semver.org/).

## [Unreleased]

### Changed

- Freshened workspace dependencies to current majors.

## [1.0.0] - 2026-07-10

### Breaking

- Built on vizij-graph-core 1.0.0 / vizij-api-core 1.0.0: values from graph
  evaluation and the node-registry defaults are now in arora serde form. Read
  them through the `@vizij/value-json` accessors; code that pattern-matched the
  raw JSON shape must switch to the accessors. Legacy input forms are normalized
  on ingress.

### Added

- Record nodes and to/from vector nodes exposed; noise params supported in the
  runtime `set_param` paths.

### Changed

- Graph plan cache and WASM fast paths; delta staging tightened in the wasm
  wrapper; caching policy documented.

### Fixed

- Casing on wasm conversion; borrow/self-aliasing in the wasm delta-snapshot
  helper.

## [0.3.0] - 2025-09-28

### Changed

- Cleaned up graph support for IK; added blend-node test coverage.

## [0.2.0] - 2025-09-23

### Added

- IK nodes and a demo IK node; transition nodes; multi-slider node; range-node
  params; graph input/output wiring exposed through the WASM/NPM packages;
  examples and tests; console panic hook.

### Changed

- Refactored the node schema; extracted the shared API (vizij-api-core); typed
  paths with refined shape/value handling; refactor around vector data.

## [0.1.0] - 2025-09-05

### Added

- Initial release: wasm-bindgen interface for the Vizij node graph.
