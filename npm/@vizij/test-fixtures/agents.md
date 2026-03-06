# @vizij/test-fixtures — Agent Notes

- **Purpose**: Ships the shared fixture manifest/JSON for browser and Node consumers (mirrors `vizij-test-fixtures`).
- **Key files**: `src/animations.ts`, `src/nodeGraphs.ts`, `src/orchestrations.ts`, `src/shared.ts`, `dist/`.
- **Commands**: `pnpm run build:shared`, `pnpm --filter @vizij/test-fixtures build`.
- **Sync**: After editing `fixtures/manifest.json`, update both this package and the Rust crate; keep versions aligned.
- **Docs**: Update the README when altering dist layout, versioning policy, or partial-bundle guidance.
- **QA**: Ensure new fixtures round-trip via the manifest helpers and validate downstream packages that consume the generated bundle.
- **Release**: This package is currently private. If that changes, capture the release through Changesets and update this note to match the publish workflow.
