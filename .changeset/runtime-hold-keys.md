---
"@vizij/runtime": minor
---

`Runtime.holdKeys(keys)` holds store keys at the values clients write: from the next step until `releaseKeys(keys?)` (every held key when omitted), the Vizij's own behavior — its graphs, its program and skill runs, its animations — does not write them, while `setValue`, `writeValues` and the rest module's `reset` still do. What the graphs compute from a held key follows the client's value, so an author's edit of a rig input, a pose weight or a program's input stands while it is made. Holding a key the behavior does not write changes nothing; a reloaded Vizij starts with no holds.
