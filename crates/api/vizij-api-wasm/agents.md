# vizij-api-wasm — Agent Notes

- **Purpose**: wasm-bindgen wrapper exposing Value/WriteBatch validation and normalisation to JS tooling.
- **Key files**: `src/lib.rs`, generated `pkg/` artefacts.
- **Commands**: Build via `wasm-pack build` (or `pnpm run build:shared` downstream), test with `cargo test -p vizij-api-wasm`.
- **Integration**: Animation/graph/orchestrator wasm crates depend on these helpers—keep function signatures stable.
- **Docs**: Maintain README notes on consumer guidance and error messaging whenever exports change.
- **Follow-ups**: wasm_bindgen tests and string-returning helpers are tracked in `ROADMAP.md`.
