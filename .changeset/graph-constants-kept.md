---
"@vizij/runtime": patch
"@vizij/node-graph": patch
---

A graph's constants keep their value from one evaluation to the next instead of being copied each time: a face's device step takes 42 % fewer instructions.
