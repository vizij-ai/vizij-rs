---
"@vizij/runtime": minor
---

`channelKeys(gltf, channels)` names the store key each animation track channel drives on the Vizij whose glTF JSON document is `gltf`, without loading it: one key per channel, in order, resolved through the bundle's rig exactly as `Runtime.moduleAnimation` keys a track once the Vizij is loaded. A page that samples an animation on a `startRuntime` device over `composeVizij`'s graph writes these keys to move it as the Vizij's own playback does. It rejects a document that carries no `VIZIJ_bundle`.
