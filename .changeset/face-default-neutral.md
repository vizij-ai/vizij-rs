---
"@vizij/runtime": patch
---

A face whose pose config is in `face-default` neutral mode rests at its rig inputs' authored defaults, as vizij-authoring shows it. The runtime staged the bundle's `neutralInputs` at load whatever the mode, so a default changed after the face's poses were made, such as the import orientation, was reverted in every app; `describe()`'s `neutralInputs` and the rest module's `reset` now give those defaults too. An `explicit` neutral is unchanged.
