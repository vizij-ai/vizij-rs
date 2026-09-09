---
"@vizij/runtime": minor
---

Profiles and mappings are two things, and the API now says so.

A **profile** is an interface: the set of store paths one party exposes to another, each with its type, range, and default. `profiles()` lists the shipped ones (`{ id, version, title, description, scope, keys }` — currently `vizij-face`, 81 paths, and `ros4hri`, 40) and `profile(id, rigPrefix)` returns one in full, every path with its arora type, range, default, and standard metadata (FACS action unit, ARKit blendshape, tier). `scope` says where the paths live: a `face` profile is addressed to one face with its rig prefix, a `device` profile is absolute and ignores the prefix.

A **mapping** is a graph that implements one profile in terms of another. `mappings()` and `mapping(id, rigPrefix)` are the renamed `standardProfiles()` / `standardProfile(id, rigPrefix)`, which stay as deprecated aliases (with `StandardProfile` aliasing `Mapping`); nothing that consumes them moves.

This is the API an authoring app's profile import consumes: pick a profile, get its paths already addressed to the open face, and declare it on the GLB (`bundle.profiles`) so the interface a face is authored against travels with the asset.
