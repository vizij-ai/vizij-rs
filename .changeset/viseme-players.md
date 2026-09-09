---
"@vizij/runtime": minor
"@vizij/node-graph": minor
---

Visemes leave the ROS4HRI profile and become the viseme players' business. `skills()` lists `play_viseme` (one shape through a lipsync envelope; a new call takes the lips over) and `say` (text-to-speech with the lips driven from the streamed visemes) next to `look_at`, with `skillSource(id)` serving their fragments; the `ros4hri` profile no longer declares `standard/ros4hri/viseme/*`, and the mapping no longer writes the lipsync surface. The face standard's `standard/vizij/viseme/<shape>` weights stay raw, and the current viseme is state at `standard/vizij/viseme`. In the node graph, the `taskrun` node gains `mutated` (the call's out parameters as a record keyed by parameter id) and `done` (terminality) outputs, and integer values count as scalars to the arithmetic nodes.
