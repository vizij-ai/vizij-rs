# Docs Refresh Brief

This branch is a fresh docs pass starting from current `main`. It is informed by the older `docs-update` work, but it is not a direct port of that branch.

## Goal

Improve the quality of the repo's documentation in a way that is:

- accurate against current `main`
- easy to review
- low-risk to merge
- useful for maintainers, downstream users, and future contributors

## Non-Goals

Do not use this branch to:

- re-import the old `docs-update` branch wholesale
- carry over local artifacts, generated files, or workflow detritus
- sneak in behavior changes under the cover of docs work
- rewrite stable docs just for tone or style

## What Good Looks Like

A strong docs PR from this branch should:

- make the root README and crate/package READMEs more reliable
- tighten public API docs where they materially help users or reviewers
- align doc claims with actual scripts, tests, fixtures, and file layout
- add only examples that are clearly correct and maintainable
- stay scoped enough that reviewers can understand it without archaeology

## Hard Rules

### 1) Start from current truth

Every doc change should be checked against current `main`, not against memory and not against the stale branch.

Use the repo as the source of truth:

- `README.md`
- crate and package `README.md` files
- `Cargo.toml`
- `package.json`
- `.github/workflows/ci.yml`
- `scripts/`
- actual test directories, examples, fixtures, and exported APIs

### 2) Do not carry over stale-branch junk

Do not port any of the following from the old docs branch:

- `.ralph/`
- `.git.bak`
- tracked `node_modules` content
- local path references
- generated or machine-local helper files
- broad ignore-rule changes unless they are clearly still needed on `main`

### 3) Keep docs changes reviewable

Prefer one or more small, coherent commits over a giant sweep.

Good slices:

- root README correctness fixes
- one crate family at a time
- one npm package family at a time
- doctest/import fixes as a separate commit

Bad slices:

- 80+ file mixed passes with no thematic boundary
- mixed docs plus tooling cleanup plus generated-file churn

### 4) Prefer precision over volume

A shorter accurate doc is better than a longer generic one.

Add documentation where it reduces confusion about:

- contracts
- invariants
- input/output shapes
- testing and validation paths
- release and wasm workflows

Avoid filler such as:

- repeating obvious type names
- narrating trivial code
- generic “this function sets the value” style comments

## Priority Order

### Priority 1: Root and workflow correctness

First, verify and improve:

- top-level `README.md`
- build/test/publish instructions
- wasm/perf references
- fixture/test guidance

This has the highest leverage and the lowest review cost.

### Priority 2: Public crate/package docs that affect adoption

Focus next on surfaces that downstream users or maintainers actually touch:

- `vizij-api-core`
- `vizij-animation-core`
- `vizij-graph-core`
- `vizij-orchestrator-core`
- npm wrapper READMEs

### Priority 3: Public API rustdoc that removes real ambiguity

Only add rustdoc where it answers questions a reviewer or user would reasonably have:

- what this type represents
- what input shape is expected
- what errors mean
- what stability assumptions exist
- whether an example is illustrative, runnable, or `no_run`

## Reviewability Standards

Before considering a docs commit “ready,” check:

1. Would a reviewer understand why these files changed together?
2. Are the claims verified against current code and scripts?
3. Did we avoid dragging in generated or local-only content?
4. Is there any hidden behavior change?
5. Is there a smaller slice if this still feels broad?

If the answer to `5` is yes, split it.

## Validation Expectations

Run the cheapest relevant validation for the slice you changed.

Typical checks:

- `cargo fmt --all`
- `cargo test --workspace`
- `cargo test -p <crate> --doc` when adding or changing doctests
- targeted package tests when touching npm wrapper docs that describe tested flows

If a claimed command or path changed, verify that it exists on current `main`.

## Practical Salvage Strategy From The Old Branch

The old `docs-update` branch can still be useful as reference material, but only as input.

Recommended approach:

1. Read the old branch for ideas, not for direct merge intent.
2. Re-derive each worthy docs change from current `main`.
3. Re-apply only the parts that are still correct.
4. Drop anything noisy, stale, or hard to verify.

## Deliverable Shape

The best outcome from this branch is probably not one giant PR.

Prefer:

1. a small root/docs correctness PR
2. one or more focused follow-up PRs for crate/package documentation

That makes review faster and keeps the branch from turning into another stale docs pile.
