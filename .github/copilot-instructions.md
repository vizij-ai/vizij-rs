# Reviewing this repository

The `vizij` crate's view is built on `arora-visuals` (semio-ai/arora-visuals), pinned as a git dependency in `crates/vizij/Cargo.toml`. The shared controls, ghosts, outlines, picking, the instance index and the frame stages (`InstanceSystems`) all live there. vizij adds its faces, rigs and runtime on top.

When a review needs that shared code, it is checked out at the exact commit `Cargo.lock` pins, in `.copilot-deps/arora-visuals`. Read it there rather than reporting the pinned implementation as unavailable.
