---
"@vizij/node-graph": patch
"@vizij/runtime": patch
---

A `Spring` node moves by the exact solution of a damped spring over each step, so one long step lands where as many short ones would, whatever its stiffness, damping and mass. A face whose page was hidden — its steps throttled to one a second, then one a minute — shows its springs where that much time leaves them when it shows again. Before, a step longer than a bound its parameters set (0.15 s for the mouth and brow springs of Quori's program, 0.23 s for its eye spring) made a spring diverge: under steps of a second Quori's springs grew to infinity and then NaN, and its mouth froze at its clamps' floors until the face was reloaded. A spring's state also stays finite: at a target that is not a number it holds, and a state seeded from one starts again at rest on the next target.
