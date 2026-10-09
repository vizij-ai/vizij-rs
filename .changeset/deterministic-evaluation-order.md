---
"@vizij/node-graph": patch
"@vizij/runtime": patch
---

The evaluation order is the graph's alone: where two outputs write one path, the later-listed one wins every time, instead of a winner that changed between loads. In a device, a task run writes after the main behavior and a later run after an earlier one, and grafting or pruning a run no longer re-plans the whole graph.
