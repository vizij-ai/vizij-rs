# Test Fixtures — Agent Notes

- **Scope**: `vizij-test-fixtures` Rust crate plus supporting assets in `fixtures/`.
- **Manifest**: `fixtures/manifest.json` enumerates animations, node graphs, orchestrations—keep it authoritative.
- **Commands**: `cargo test -p vizij-test-fixtures`; rebuild npm bundle via `pnpm run build:shared` after adding fixtures.
- **Usage**: Rust crates use `vizij_test_fixtures::*`; npm packages consume the mirrored bundle (`@vizij/test-fixtures`).
- **Docs**: Coordinate README updates with fixture additions; capture manifest or schema changes in the roadmap.
