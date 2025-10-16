# Vizij-RS Roadmap

> Engineering-oriented backlog compiled from cross-stack discussions. Update this document whenever priorities shift or work lands.

---

## Current Focus (Next 1–2 Sprints)

- Harden wasm distribution tooling:
  - Add wasm-bindgen integration tests for the animation, graph, and orchestrator packages.
  - Script ABI/version bump checks so release candidates fail fast when glue and binaries drift.
- Establish performance baselines:
  - Capture benchmark data for representative node graphs (including robotics workloads).
  - Record orchestrator pass timings to feed future observability dashboards.
- Improve developer ergonomics:
  - Prototype a shared `vizij` CLI for fixture sync, release prep, and local smoke tests.

---

## Stack Roadmaps

### Animation Stack
- Add native↔wasm parity tests to catch divergence before publishing.
- Ship a minimal Bevy example app demonstrating animation playback.
- Investigate profiling hooks (feature-flagged metrics) and faster watcher iterations.

### API Stack
- Explore generating JSON/TypeScript schemas directly from `vizij-api-core`.
- Provide derive macros or builders to reduce setter boilerplate in Bevy integrations.
- Investigate `no_std` support for constrained targets.

### Node Graph Stack
- Add optional tracing/diagnostic events for Bevy consumers.
- Prototype incremental/streaming evaluation APIs in the wasm bridge.
- Evaluate selector-heavy graphs and record benchmark results over time.

### Orchestrator Stack
- Add tracing/metrics hooks around passes and controller evaluation.
- Explore configurable animation path parsing and richer CLI smoke tests.
- Expose incremental tick APIs or event streams for advanced hosts.

- Provide automated snapshot tests that compare orchestrator frames against Rust outputs for a set of fixtures.
- Explore exposing an event stream API that emits controller lifecycle events for UI visualisations.
- Consider shipping TypeScript builder helpers for assembling `MergedGraphRegistrationConfig` programmatically.

---

## Support Packages & Fixtures

- Automate manifest linting and TypeScript definition generation for fixtures.
- Provide codemods or builders that keep Value/Shape unions in sync with Rust definitions.
- Introduce loader helper builders and telemetry around ABI mismatches.

---

## Cross-Cutting Initiatives

- Maintain `AGENTS.md` and per-crate `agents.md` alongside major workflow or tooling changes.
- Track wasm binary sizes across releases and alert on regressions.
- Publish shared benchmark dashboards once performance baselines exist.

---

## Backlog & Ideas

- Ship optional Web Worker helpers for heavy wasm workloads (animation baking, graph evaluation).
- Offer CLI utilities for schema diffing (`vizij-graph-core`) and orchestration smoke tests.
- Publish fixture metadata (durations, node counts) to help demos surface summary information quickly.
