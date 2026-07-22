# Changelog

All notable changes to `vizij-orchestrator-wasm`. The format follows
[Keep a Changelog](https://keepachangelog.com/); versions follow
[Semantic Versioning](https://semver.org/).

## [Unreleased]

### Changed

- Freshened workspace dependencies to current majors.

## [1.0.0] - 2026-07-10

### Breaking

- Built on vizij-orchestrator-core 1.0.0 / vizij-api-core 1.0.0: values crossing
  the orchestrator boundary are now in arora serde form. Read them through the
  `@vizij/value-json` accessors; code that pattern-matched the raw JSON shape
  must switch to the accessors. Legacy input forms are normalized on ingress.

### Added

- `replaceGraph` for structural edits, which invalidates the plan cache.

### Changed

- WASM bindings, typing, and docs aligned with the core; lowered the
  orchestrator log level.

### Fixed

- Transition nodes not receiving `dt` updates; casing on wasm conversion.

## [0.1.0] - 2025-09-29

### Added

- Initial release: WASM bindings for the vizij orchestrator core (JS-friendly
  wrapper), with publishing setup.
