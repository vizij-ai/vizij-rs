# Changesets

This repository uses [Changesets](https://github.com/changesets/changesets) to
version the npm packages under `npm/@vizij/*` and write their changelog entries.

1. In your PR, run `pnpm changeset`, select the packages the change affects, and
   describe it for a reader of the changelog. Commit the generated markdown file.
2. Once the PR merges, the `release` workflow opens or updates a **Version
   Packages** PR that applies every pending changeset.
3. Merging the Version Packages PR lands the new versions on `main`, and the
   `publish-npm` workflow publishes every version npm does not have yet.

A changeset never names a package on a prerelease version (`3.0.0-alpha.0`):
semver increments a prerelease to its release whatever the bump, so the Version
Packages PR would turn the alpha into `3.0.0`. Bump a prerelease by hand, in its
`package.json` and `CHANGELOG.md`; merging it publishes it under its dist-tag.

The Rust crates are not versioned by changesets: a crate's version is bumped in
its `Cargo.toml`, with a `CHANGELOG.md` entry, in the PR that changes it.
