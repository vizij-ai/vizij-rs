# vizij-test-fixtures — Agent Notes

- **Purpose**: Loads fixtures defined in `fixtures/manifest.json` for Rust tests (animations, node graphs, orchestrations).
- **Key files**: `src/lib.rs` (manifest loader, module API), `fixtures/`.
- **Commands**: `cargo test -p vizij-test-fixtures`; ensure new fixtures deserialize and update manifest.
- **Coordination**: Sync changes with `npm/@vizij/test-fixtures` (`pnpm run build:shared`) and update the README accordingly.
- **Common tasks**: Add manifest entries, provide stage payloads, and update tests in dependent crates that rely on the fixtures.
