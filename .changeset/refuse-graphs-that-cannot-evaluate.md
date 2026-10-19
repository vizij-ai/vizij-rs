---
"@vizij/runtime": minor
"@vizij/node-graph": patch
---

`loadGraph` and `applyGraphEdits` refuse a graph that would fail every evaluation, and the promise rejects with an error naming the node at fault. The graph is refused if it has a node that leaves a required input with neither an edge nor an inline default (a `remap` with no `in`, say), an edge to a node it lacks, two edges into one input, two nodes sharing an id, or a cycle. A refused edit is refused whole: the running graph, its runs and their state go on as they were. Such a graph used to be accepted, then failed the device's every behavior tick, base graph and every run with it, until another load or edit replaced it. A graph that evaluates is accepted as before, including one whose input path has no value staged yet.

In `@vizij/node-graph`'s registry, an input port a node cannot evaluate without carries `required: true`: the inputs of `clamp`, `remap`, `centeredremap` and `piecewiseremap`, `urdffk`'s `joints`, `urdfikposition`'s `target_pos`, and `urdfikpose`'s `target_pos` and `target_rot`. A port that is neither `optional` nor `required` reads a default (most often zero) when unwired.
