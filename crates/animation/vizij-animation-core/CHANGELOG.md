# Changelog

All notable changes to `vizij-animation-core`. The format follows
[Keep a Changelog](https://keepachangelog.com/); versions follow
[Semantic Versioning](https://semver.org/).

## [Unreleased]

### Changed

- Freshened workspace dependencies to current majors.

## [1.0.0] - 2026-07-10

### Breaking

- Built on the unified Value: `vizij_api_core::Value` is now
  `arora_types::value::Value` (vizij-api-core 1.0.0). Keypoints decode once at
  ingestion into POD `TrackValue` (`[f32; N]`); sampling, interpolation, and
  accumulation run on the PODs (same math, byte-identical blending), with the
  dynamic `Value` appearing only at boundaries. Values the engine emits (frame
  changes, baked tracks) are in arora serde form; consumers must read them
  through the arora accessors instead of pattern-matching the old JSON shape.

### Fixed

- Accumulation and pre-binding bugs in layered playback.

### Changed

- Standardized `Value` usage and type casing across the engine; test fixtures
  reorganized around the shared value-json helpers.

## [0.3.0] - 2025-09-28

### Added

- Derivative update and baking APIs: per-frame derivative calculations and
  baked-track output exposed through the core (and surfaced in the wasm/npm
  layers).

## [0.2.0] - 2025-09-23

### Added

- Full animation-player port; multi-slider node.

### Changed

- Extracted the shared types and interfaces into a common API
  (vizij-api-core); refactor around vector data; crate metadata prepared for
  publishing.

### Fixed

- Timing bugs; offset and scale calculations on instances.

## [0.1.0] - 2025-09-05

### Added

- Initial release: engine-agnostic core animation logic for Vizij.
