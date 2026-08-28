---
"@vizij/runtime": minor
---

Expose the profile registry to JS: `profiles()` lists the shipped profiles (`{ id, version, title, description, keys }` — currently `vizij-face` with 81 paths and `ros4hri` with 40) and `profile(id, rigPrefix)` returns one in full, every path with its type, range, default, and standard metadata (FACS action unit, ARKit blendshape, tier), with the face's rig prefix applied.

A *profile* is a set of paths and their types — the vocabulary half of a standard. It is distinct from `standardProfile(id, rigPrefix)`, which returns a *mapping*: the graph that carries one profile's values onto another's. The two are separate exports rather than a rename, so nothing that consumes `standardProfile` moves.

The API an authoring app's profile import consumes: pick a profile, get its paths already addressed to the open face, and declare it on the GLB (`bundle.profiles`) so the vocabulary a face is authored against travels with the asset.
