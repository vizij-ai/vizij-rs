---
"@vizij/runtime": patch
"@vizij/node-graph": patch
---

A graph edge that selects from a constant is resolved when the graph is planned, so evaluating the graph neither reads nor copies the constant: a face's device step takes 12 % fewer instructions. In `@vizij/node-graph`, `set_param` on the `value` of a constant read through a selector replans the graph.
