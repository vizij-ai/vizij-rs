---
"@vizij/runtime": minor
---

`describe(glb)` reads the face's versions from its bundle: `hardwareVersion`, its `metadata.hardwareVersion` — the version of the face itself, its shapes, what can move in them and the layout giving those movements their meaning, such as `quori-1.3` — and `softwareVersion`, its `metadata.softwareVersion` — the version of what it runs, its profiles, exposed inputs, programs, extra rigs and animations, such as `1.2`. Each is `null` when the bundle states none or a blank one. A Vizij's device describes itself as the `vizij` model family, of the face's hardware version, else its `faceId`, and of its software version when it has one.
