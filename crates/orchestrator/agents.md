# Orchestrator Stack — Agent Notes

- **Scope**: `vizij-orchestrator-core` (a workspace-private crate).
- **Purpose**: Multi-pass coordination of graphs/animations backed by a shared blackboard.
- **Key commands**: `cargo test -p vizij-orchestrator-core`.
- **Integration**: Relies on animation + graph stacks; consumed by `vizij-arora-behavior` in the interop stack.
- **Docs**: Study the README sections covering TwoPass examples and conflict logs when behaviour changes.
- **Fixtures**: Use orchestration bundles from `vizij_test_fixtures::orchestrations` / `@vizij/test-fixtures`.
