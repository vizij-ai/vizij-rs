---
"@vizij/runtime": patch
---

The module runs its constructors once, when it is instantiated, so a call into it runs only its own code. `invoke`, which builds its answer as JavaScript values and calls into the module for each one, answers in about the time `call` takes for the same value: its answer used to take 10 to 30 times as long as `call`'s, the module's constructors running on each of those calls. Every other operation sheds the same cost per call into the module.
