# @vizij/value-json — Agent Notes

- **Purpose**: TypeScript definitions and helpers for Vizij Value/Shape payloads shared across npm packages.
- **Key files**: `src/value.ts`, `src/shape.ts`, `src/utils.ts`.
- **Commands**: `pnpm --filter @vizij/value-json test`; rebuild via `pnpm run build:shared`.
- **Docs**: Keep the README current (conversion matrix, tree-shaking tips, shape metadata guidance).
- **Dependencies**: Mirrors `vizij-api-core`; coordinate structural changes with the Rust crate.
- **Common work**: Add coercion helpers, update union types, ensure TypeScript definitions stay aligned with Rust schema.
- **Release**: Queue a changeset, get it on `main`, then push an `npm-pub-*` tag to trigger the CI release (runs `pnpm ci:version` + `pnpm ci:publish`).
