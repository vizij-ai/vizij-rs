# @vizij/node-graph — Agent Notes

- **Purpose**: JavaScript/TypeScript wrapper around `vizij-graph-wasm` with Graph class, schema helpers, and fixtures.
- **Key files**: `src/index.ts`, `src/graph.ts`, `src/schema.ts`, `src/types.ts`, `pkg/`.
- **Commands**: `pnpm run build:wasm:graph`, `pnpm --filter @vizij/node-graph test`.
- **Docs**: Keep the README current on custom wasm URLs, troubleshooting, and schema registry docs.
- **Dependencies**: Relies on `@vizij/value-json`, `@vizij/test-fixtures`, `@vizij/wasm-loader`.
- **Follow-ups**: Streaming evaluation ideas and parity tests track in `ROADMAP.md`; update when progress is made.
- **Release**: Add a changeset (`pnpm changeset`) in your PR. Once it merges, CI opens or updates the Version Packages PR; merging that publishes the package.
