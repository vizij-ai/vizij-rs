---
"@vizij/runtime": minor
---

A device's animation module (1.1.0) adds `set_window`, `play_at`, `set_start_offset` and `set_time_scale`, callable with `Runtime.call`, plays backwards at a negative `set_speed`, and reports `ended`, the play window and each instance's timing in the player states at `vizij/animations/players` (see `@vizij/animation-module`). The device's animation source steps the module without `time_ns`, so there `play_at` counts from the module's first step.
