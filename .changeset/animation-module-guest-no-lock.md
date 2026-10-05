---
"@vizij/animation-module": patch
---

The module (1.1.1) reaches its state without a lock, so the calls after a trap are answered, where a lock the trapped call held stayed held and failed every later call of the instance. A trap still leaves the instance unsound to keep — what the trapped call had changed stays changed, and its stack frames and argument buffer are not reclaimed — so a host that sees a trap should retire the instance. `set_speed` rejects a speed that is not finite, and `set_weight` and `add_instance_with_weight` a weight that is not finite and non-negative, with `u32::MAX`: a NaN or infinite value used to reach the engine and leave the playhead or the blend at NaN, and a negative weight, which the engine ignored, is now refused.
