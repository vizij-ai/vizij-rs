# vizij-orchestrator-core — Agent Notes

- **Purpose**: Schedules graph/animation controllers against a shared blackboard with deterministic passes.
- **Key files**: `src/orchestrator.rs`, `src/controllers/`, `src/blackboard.rs`, `examples/`.
- **Commands**: `cargo test -p vizij-orchestrator-core`; try examples via `cargo run -p vizij-orchestrator-core --example <name>`.
- **Docs**: Update the README when adjusting TwoPass behaviour, conflict logging, or merge strategies.
- **Dependencies**: Consumes `vizij-graph-core`, `vizij-animation-core`, `vizij-api-core`; coordinate API changes across stacks.
- **Roadmap**: Metrics hooks, configurable animation path parsing, and CLI smoke tests live in `ROADMAP.md`.
