---
"@vizij/runtime": minor
---

The calls an authoring viewport makes. `drainPicks()` reports a press on no element as a miss (`elementId: null`), and finds the element over its mesh as drawn, morphs applied. `setSelection(vizijId, elementIds)` outlines the chosen elements with the selection glow. `Runtime.hold(paths)` / `release(paths?)` keep rig outputs, whole or per component, at what the view shows while the device runs. `setStaticFeature(vizijId, elementId, feature, value)` sets a static feature in place, with no reload. `loadVizij` over a Vizij already shown keeps its previous scene on screen until the new one is ready, so a structural edit reloads from exported GLB bytes without a blank frame; `ready` reads false from the call until then. A GLB carrying static features in its RobotData loads.
