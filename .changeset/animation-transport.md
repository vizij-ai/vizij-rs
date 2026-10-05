---
"@vizij/runtime": minor
---

A Vizij's animations load into its device's animation module with the face (unless `loadVizij(id, glb, { animations: false })`): each on a player named after the animation's id, its instance at weight 0, stopped, each track writing the rig input its channel names. Every client plays, loads and unloads animations through the module's declared functions, by name (`invoke("play", { player })`, `set_weight`, `load_animation`, `reload_animation`, …). `moduleAnimation(animation)` turns an animation in `describe()`'s shape into the module's `AnimationClip` on the face's rig keys, for `load_animation`; `decodePlayerStates` reads the player states the animation source writes to `ANIMATION_PLAYERS_PATH` each step. `ModuleAnimation`, `PlayerState` and `InstanceState` are exported types.
