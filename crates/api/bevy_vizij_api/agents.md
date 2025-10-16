# bevy_vizij_api — Agent Notes

- **Purpose**: Applies `WriteBatch` updates to Bevy worlds via a registry of setter callbacks.
- **Key modules**: `src/registry.rs`, `src/setters/transform.rs`, `src/apply.rs`.
- **Commands**: `cargo test -p bevy_vizij_api`; rerun dependent Bevy plugin tests after modifying setters.
- **Usage**: Higher-level plugins clone `WriterRegistry`; preserve thread-safety (Arc<Mutex>) when extending.
- **Docs**: Update the README when changing setter behaviour, error handling, or feature flags.
- **Watch for**: Coordinate new component bindings with `bevy_vizij_animation`/`bevy_vizij_graph` to avoid duplication.
