---
"@vizij/runtime": minor
---

A `Runtime` controls its face's programs while the device runs: `startProgram(id)`, `pauseProgram(id)` and `stopProgram(id, { resetOutputs })` play, pause and stop any of the bundle's motiongraphs, several at once; `programState(id)` reads `"playing"`, `"paused"` or `"stopped"`; `setProgram(id, graph)` defines a program under a new id or replaces a program's graph, in place when it plays. A playing program's nodes are part of the device's one running graph, so none of these reloads the face or touches the store's other values. `resetOutputs` returns each key the program writes to the neutral pose the face staged for it, or clears it. `ProgramState` and `StopProgramOptions` are exported types.
