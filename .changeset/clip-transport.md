---
"@vizij/runtime": minor
---

A Vizij's animation clips load into its device with the face, each track writing the rig input its channel names, and `Runtime` plays them: `clips()` lists them (`{ id, name, duration }`); `playClip(id, { reset, speed })`, `pauseClip`, `stopClip(id, { clearOutputs })`, `seekClip(id, seconds)`, `setClipLoop` and `setClipSpeed` drive the animation module's transport through `call`; `clipState(id)` reads `{ time, duration, playing, loop, speed, completed }` from the player states at `vizij/animations/players`; `setClip(clip)` adds or replaces a clip live (in `describe()`'s `clips` shape) and `removeClip(id)` unloads one. A clip loads silent, stopped at its start, looping at speed 1.
