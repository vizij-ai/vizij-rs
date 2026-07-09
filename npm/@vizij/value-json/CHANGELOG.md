# @vizij/value-json

## 0.2.0

### Minor Changes

- Decode arora serde values: every `valueAs*` accessor now reads the JSON the
  Rust side emits (`{"f32": …}`, typed structures by vizij type id, enums,
  keyvalue records, arrays, options). New exports: `fromAroraValueJSON`, the
  `AroraValueJSON` types, and the `VIZIJ_*_TYPE` id constants. Values sent
  into the engines may stay in the legacy forms — the Rust side normalizes
  them.

## 0.1.2

### Patch Changes

- 9bd0189: Fix publishing to include dist again

## 0.1.1

### Patch Changes

- f6cba9e: Release process testing patch bump

Release notes are handled by [Changesets](../../../.changeset/README.md). Capture updates with `pnpm changeset` before publishing.
