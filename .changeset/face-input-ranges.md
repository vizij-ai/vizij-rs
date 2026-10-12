---
"@vizij/runtime": minor
---

A face's free input outside the shipped profiles takes its range, as `listKeys` reports it, from: its input node's `params.min` and `params.max`, when both are there and `max` exceeds `min` (both optional); else the `range` the rig declares for it in `metadata.vizij.inputs`, as the authoring app exports for every rig input; else `[0, 1]`. A key a shipped profile declares keeps that profile's range, and types are unchanged. A face's rig inputs that rested outside `[0, 1]` (an eye's scale or translation, say) now report the range their rig declares around their rest.
