---
"@vizij/runtime": patch
---

A Vizij's device reads its graph's inputs from the store in one read per step, under keys prepared when the graph loads: its step takes 16 % fewer instructions.
