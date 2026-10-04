---
"@vizij/runtime": minor
---

A Vizij's animations load into its device with the face, each track writing the rig input its channel names, and `Runtime` plays them: `animations()` lists them (`{ id, name, duration }`); `playAnimation(id, { reset, speed })`, `pauseAnimation`, `stopAnimation(id, { clearOutputs })`, `seekAnimation(id, seconds)`, `setAnimationLoop` and `setAnimationSpeed` drive the animation module's transport through `call`; `animationState(id)` reads `{ time, duration, playing, loop, speed, completed }` from the player states at `vizij/animations/players`; `setAnimation(animation)` adds or replaces an animation live (in `describe()`'s `animations` shape) and `removeAnimation(id)` unloads one. An animation loads silent, stopped at its start, looping at speed 1. `LoadedAnimation`, `AnimationState`, `PlayAnimationOptions` and `StopAnimationOptions` are exported types.
