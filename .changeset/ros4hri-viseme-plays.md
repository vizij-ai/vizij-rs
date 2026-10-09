---
"@vizij/runtime": minor
---

The `ros4hri` mapping plays the streamed viseme: each new `standard/ros4hri/viseme` code spawns a `play_viseme` run of its shape, which takes over from the run before it, and `sil` lets the lips settle at rest. The mapping writes no lip key itself, so a stream and the device's own `say` share the lips the way players do.
