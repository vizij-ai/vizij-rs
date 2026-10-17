---
"@vizij/runtime": major
"@vizij/node-graph": minor
---

A step with no time in it moves nothing that time drives. In `@vizij/runtime`, `step(0)` carries changes through the Vizij's graphs and runs and moves nothing that time drives: the springs, dampers and slews of its graphs, programs and skills keep their value (and a spring its velocity), as the clocks, oscillators, noise and animations already did. They used to jump to their targets, so a paused face snapped each smoothed value on every edit. In `@vizij/node-graph`, the same holds for an `evalAll` with no `step` before it (or after a `setTime` back in time). An IK node answers a request equal to its last one with its last solution: solving a redundant chain again from its own solution moved its joints with nothing changed.
