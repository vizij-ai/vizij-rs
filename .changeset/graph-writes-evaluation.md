---
"@vizij/runtime": patch
---

A Vizij's device evaluates its graph without copying every port by name each step, a copy nothing read: its step takes 30 % fewer instructions.
