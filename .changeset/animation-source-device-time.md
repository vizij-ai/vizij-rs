---
"@vizij/runtime": minor
---

The animation source steps the animation module with the device's `arora/time` as well as `arora/dt`, so `play_at(player, time_ns)` starts a player at that device time, where it counted from the module's first step.
