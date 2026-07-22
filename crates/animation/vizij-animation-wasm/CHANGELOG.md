# Changelog

All notable changes to `vizij-animation-wasm`. The format follows
[Keep a Changelog](https://keepachangelog.com/); versions follow
[Semantic Versioning](https://semver.org/).

## [Unreleased]

### Changed

- Freshened workspace dependencies to current majors.

## [1.0.0] - 2026-07-10

### Breaking

- Built on vizij-animation-core 1.0.0 / vizij-api-core 1.0.0: values the engine
  emits (frame changes, baked tracks) are now in arora serde form. Read them
  through the `@vizij/value-json` accessors; code that pattern-matched the raw
  JSON shape must switch to the accessors. Legacy input forms are normalized on
  ingress.

### Changed

- Standardized `Value` usage and casing; fixtures and value-json packages
  refactored.

### Fixed

- Accumulation and pre-binding bugs surfaced through the wasm bridge.

## [0.3.0] - 2025-09-28

### Added

- Baked animation exposed through the wasm/npm API; derivative update API.

## [0.2.0] - 2025-09-23

### Added

- Full animation-player port; multi-slider node.

### Changed

- Extracted the shared API (vizij-api-core); refactor around vector data; crate
  metadata prepared for publishing.

### Fixed

- Offset and scale calculations on instances.

## [0.1.0] - 2025-09-05

### Added

- Initial release: wasm-bindgen interface for the Vizij animation engine.
