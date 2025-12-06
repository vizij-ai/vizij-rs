# Orchestrator Stack — Agent Notes

- **Scope**: `vizij-orchestrator-core` and `vizij-orchestrator-wasm`.
- **Purpose**: Multi-pass coordination of graphs/animations backed by a shared blackboard.
- **Key commands**: `cargo test -p vizij-orchestrator-core`, `cargo test -p vizij-orchestrator-wasm`, `pnpm run build:wasm:orchestrator`, `pnpm --filter @vizij/orchestrator-wasm test`.
- **Integration**: Relies on animation + graph stacks; keep API changes aligned with wasm and npm wrappers.
- **Docs**: Study the README sections covering TwoPass examples, conflict logs, and wasm JSON docs when behaviour changes.
- **Fixtures**: Use orchestration bundles from `vizij_test_fixtures::orchestrations` / `@vizij/test-fixtures`.
