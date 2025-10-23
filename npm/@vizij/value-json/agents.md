# @vizij/value-json — Agent Notes

- **Purpose**: TypeScript definitions and helpers for Vizij Value/Shape payloads shared across npm packages.
- **Key files**: `src/value.ts`, `src/shape.ts`, `src/utils.ts`.
- **Commands**: `pnpm --filter @vizij/value-json test`; rebuild via `pnpm run build:shared`.
- **Docs**: Keep the README current (conversion matrix, tree-shaking tips, shape metadata guidance).
- **Dependencies**: Mirrors `vizij-api-core`; coordinate structural changes with the Rust crate.
- **Common work**: Add coercion helpers, update union types, ensure TypeScript definitions stay aligned with Rust schema.
- **Release**: Queue a changeset (`pnpm changeset`), version with `pnpm version:packages`, and publish via `pnpm release` from the repo root.
