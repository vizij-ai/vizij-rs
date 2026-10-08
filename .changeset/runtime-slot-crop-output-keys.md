---
"@vizij/runtime": minor
---

A Vizij's rectangle is any rectangle of the canvas. `placeVizij` frames the Vizij on the whole rectangle and draws the part on the canvas, so a slot scrolled past the canvas's edge shows its Vizij cut off there, at the position and scale the whole slot gives it, where it used to be moved (past the top or left edge) or refitted into what was left (past the bottom or right edge). An empty rectangle (a width or height of 0 or less) or one wholly off the canvas draws nothing and takes no picks; a Vizij never placed still draws over the whole canvas. `safeArea` follows the whole rectangle, and reads `null` while it is empty. The canvas shows `mount`'s background wherever no Vizij draws, also when none does.

`outputKeys(graph)` lists the store keys a run of `graph` writes and leaves holding when halted — its output nodes' paths, read as the device reads them, without the `task/…` outputs that write the run's own keys — what `invoke("reset_keys", { keys: { strs } })` takes to return a halted program's outputs to rest.

`describe(glb).graphs` carries each graph's `id` beside its `kind` (`null` for an entry without one).
