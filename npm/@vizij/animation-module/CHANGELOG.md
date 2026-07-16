# Changelog

All notable changes to `@vizij/animation-module`. The format follows
[Keep a Changelog](https://keepachangelog.com/); versions follow
[Semantic Versioning](https://semver.org/).

## [0.1.0] - 2026-07-16

### Added

- Initial release: the vizij animation engine packaged as an Arora wasm
  module, shipped as importable assets (`wasm32-wasip1` executable + Arora
  header JSON). `loadAnimationModule()` returns the `{ headerJson, wasmBytes }`
  pair `@vizij/arora-web-wasm`'s `startDevice` loads into the browser device;
  `headerUrl`/`wasmUrl` expose the raw assets.
