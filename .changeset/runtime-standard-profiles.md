---
"@vizij/runtime": minor
---

Expose the standard-profile registry to JS: `standardProfiles()` lists the shipped profiles (`{ id, title, description }` — currently `ros4hri`) and `standardProfile(id, rigPrefix)` returns a profile's graph with the face's rig prefix applied, ready to compose or to embed into a GLB as a `standard-profile` bundle graph (`standard::<id>`). The API an authoring app's opt-in picker consumes (VIZ-92).
