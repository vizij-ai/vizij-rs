---
"@vizij/runtime": major
---

The device runs on arora 18, through arora-web 15: its behavior interpreter reads arora-behavior 10's graphs, a node holding its own bindings, and its behavior trees are `behavior-tree` 2.0 graphs. A graph `invoke`, `spawn` or the interpreter module's `LOAD` and `EDIT` carry in arora-behavior 9's form is refused. `loadGraph`, `applyGraphEdits` and `runEdits` take and give Vizij specs and spec diffs as before; `behaviorValue` gives the graph in the form the device's `run_behavior` now takes.
