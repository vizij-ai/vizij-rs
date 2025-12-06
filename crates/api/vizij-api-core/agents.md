# vizij-api-core — Agent Notes

- **Purpose**: Canonical `Value`, `Shape`, and `TypedPath` types plus write-batch helpers used by every engine.
- **Key files**: `src/value.rs`, `src/shape.rs`, `src/typed_path.rs`, `src/json/`.
- **Commands**: `cargo test -p vizij-api-core`; run `pnpm --filter @vizij/value-json test` when changing JSON normalisation.
- **Policy**: Treat changes as breaking unless proven otherwise—coordinate with all dependent crates and npm packages.
- **Documentation**: Keep README coverage (JSON mapping table, serde features) up to date when altering serialization.
- **Future work**: Schema/code generation and `no_std` feasibility tracked in `ROADMAP.md`.
