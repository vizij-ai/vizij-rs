---
"@vizij/runtime": minor
---

`runtime.reset()` returns a face to rest at any time: every key the device describes goes back to the value it rests at — each free input of the composed graph to its authored default, each input the bundle's neutral pose names to its neutral — through the device's `rest` module, so every bridge sees the writes. A device from `startRuntime` rests its graph's free inputs at their defaults.

`setView(vizijId, { bounds, fit, zoom, toneMapping })` frames and tone-maps one face on a canvas several faces share: `bounds` (`{ center, size }`, the shape `describe` reports `rootBounds` in) replaces the GLB's authored bounds, `fit` and `zoom` override `mount`'s for that face, and `toneMapping` is one of the authoring app's curves (`none`, `agx`, `aces`, `neutral`, as three.js computes them). Each call replaces the face's previous view, and the view survives the face's reloads. `safeArea(vizijId)` gives the rectangle the face's framed bounds cover on the canvas, in CSS pixels from its top-left corner, for a DOM overlay. `Bounds`, `Fit`, `ToneMapping` and `VizijView` are exported types.
