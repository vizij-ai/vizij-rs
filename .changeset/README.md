# Changesets

This repository uses [Changesets](https://github.com/changesets/changesets) to manage npm package versions and changelog entries.

## Workflow

1. After merging feature work, run `pnpm changeset` and select the affected npm packages under `npm/@vizij/*`. Describe the change succinctly.
2. Commit the generated markdown file under `.changeset/`.
3. When you are ready to publish:
   ```bash
   pnpm version:packages
   pnpm release
   ```
4. Commit the generated changelog/version bumps and push the relevant `npm-*-vX.Y.Z` tag so the GitHub workflows can publish the packages.

The `release` script reinstalls dependencies and rebuilds the wasm/shared bundles to validate artefacts before tagging; publishing still happens via the tag-triggered GitHub workflows.
