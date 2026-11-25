# Reviewing this repository

The `vizij` crate's view may be built on `arora-visuals` (semio-ai/arora-visuals). When a revision depends on it, it is a git dependency in `crates/vizij/Cargo.toml`, and the shared controls, ghosts, outlines, picking, the instance index and the frame stages (`InstanceSystems`) all live there. vizij adds its faces, rigs and runtime on top.

When the revision under review has `arora-visuals` in its `Cargo.lock`, that source is checked out at the exact commit the lock pins, in `.copilot-deps/arora-visuals`. Read it there rather than reporting the pinned implementation as unavailable. When the lock has no `arora-visuals`, nothing is checked out there; review against the revision's own sources and dependency declarations.
